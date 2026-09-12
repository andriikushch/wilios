//! Offline MIDI export: run the pipeline and write a Standard MIDI File
//! (format 1) — no audio device, no synthesis.
//!
//! Timing is musical: ticks come straight from each note's exact `at_beats`
//! (480 PPQ), and every `tempo` change becomes a `Tempo` meta event, so a DAW's
//! bar grid lines up. wilios tempo is per-track but a MIDI file has one global
//! tempo map, so the lowest-numbered track's tempo history is used as the global
//! map and a warning is logged if another track disagrees.

use std::path::PathBuf;

use midly::num::{u4, u7, u15, u24, u28};
use midly::{Format, Header, MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind};
use wilios_core::dump::build_dump;
use wilios_core::interpreter::interpreter::Interpreter;
use wilios_core::time::Beats;

use crate::render::{DEFAULT_MAX_RENDER_SECS, partial_path};

/// Pulses (ticks) per quarter note.
const PPQ: u16 = 480;
/// `Tempo` meta carries microseconds per quarter note in a `u24`.
const MAX_US_PER_QN: u32 = (1 << 24) - 1;

pub struct MidiOpts {
    pub out: PathBuf,
    /// `Some(secs)` exports exactly that much composition time and never errors
    /// on a non-terminating piece; `None` exports until every track finishes,
    /// bounded by `max_secs`.
    pub duration: Option<f32>,
    pub max_secs: f32,
}

impl MidiOpts {
    pub fn new(out: PathBuf) -> Self {
        Self {
            out,
            duration: None,
            max_secs: DEFAULT_MAX_RENDER_SECS,
        }
    }
}

/// Whole-note beats → MIDI ticks (a quarter note = `1/4` whole note = `PPQ` ticks).
fn ticks(beats: f64) -> u32 {
    (beats * 4.0 * PPQ as f64).round().max(0.0) as u32
}

fn beats_to_f64(b: Beats) -> f64 {
    *b.numer() as f64 / *b.denom() as f64
}

/// Run `interp` to completion (or to the configured bound) and write a `.mid`.
/// Writes to a `<out>.partial` sibling and renames it into place only on success.
pub fn export_midi(mut interp: Interpreter, opts: MidiOpts) -> Result<(), String> {
    let bound_ms: u64 = match opts.duration {
        Some(secs) => (secs as f64 * 1000.0).round() as u64,
        None => (opts.max_secs as f64 * 1000.0).round() as u64,
    };

    let (events, finished) = interp.schedule_to_end(bound_ms).map_err(|e| e.0)?;

    if opts.duration.is_none() && !finished {
        return Err(format!(
            "piece did not finish within {:.0}s and no --duration was given — \
             it likely contains an endless loop (`loop (true) {{ … }}`) or a \
             note shorter than its attack that never releases. \
             Re-run with --duration <seconds>.",
            opts.max_secs
        ));
    }

    let dump = build_dump(&events, finished);
    if dump.tracks.is_empty() {
        return Err("nothing to export: the piece scheduled no notes".into());
    }

    // Global tempo map: lowest track id wins; warn if another track disagrees
    // (a single-tempo-map format cannot represent per-track tempo).
    let mut tempo_tracks: Vec<(usize, Vec<(Beats, u32)>)> = interp
        .tracks
        .iter()
        .map(|t| {
            (
                t.ctx.track_id,
                t.ctx.tempo_history.breakpoints().collect::<Vec<_>>(),
            )
        })
        .collect();
    tempo_tracks.sort_by_key(|(id, _)| *id);
    let (ref_id, tempo_map) = tempo_tracks
        .first()
        .cloned()
        .unwrap_or((0, vec![(Beats::new(0, 1), 120)]));
    for (id, bps) in tempo_tracks.get(1..).unwrap_or(&[]) {
        if *bps != tempo_map {
            tracing::warn!(
                "track {id} has a different tempo map than track {ref_id}; MIDI is \
                 single-tempo, so track {id} may play back at the wrong speed"
            );
        }
    }

    let mut smf = Smf::new(Header::new(
        Format::Parallel,
        Timing::Metrical(u15::new(PPQ)),
    ));

    // Track 0: the tempo map.
    smf.tracks.push(build_tempo_track(&tempo_map));

    // One MIDI track per wilios track.
    let names: Vec<String> = dump
        .tracks
        .iter()
        .map(|t| format!("track {}", t.id))
        .collect();
    for (idx, td) in dump.tracks.iter().enumerate() {
        let channel = u4::new((idx % 16) as u8);
        // (tick, order, message); order 0 sorts note-off before note-on at the
        // same tick, so a repeated pitch retriggers cleanly.
        let mut msgs: Vec<(u32, u8, MidiMessage)> = Vec::with_capacity(td.notes.len() * 2);
        for n in &td.notes {
            let on = ticks(n.at_beats_f64);
            let off = ticks(n.at_beats_f64 + n.dur_beats_f64).max(on + 1);
            let key = u7::new(n.midi_note.min(127));
            let vel = u7::new(n.velocity.clamp(1, 127) as u8);
            msgs.push((on, 1, MidiMessage::NoteOn { key, vel }));
            msgs.push((
                off,
                0,
                MidiMessage::NoteOff {
                    key,
                    vel: u7::new(0),
                },
            ));
        }
        msgs.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

        let mut track: Vec<TrackEvent> = vec![TrackEvent {
            delta: u28::new(0),
            kind: TrackEventKind::Meta(MetaMessage::TrackName(names[idx].as_bytes())),
        }];
        let mut last = 0u32;
        for (tick, _, message) in msgs {
            track.push(TrackEvent {
                delta: u28::new(tick - last),
                kind: TrackEventKind::Midi { channel, message },
            });
            last = tick;
        }
        track.push(TrackEvent {
            delta: u28::new(0),
            kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
        });
        smf.tracks.push(track);
    }

    let tmp = partial_path(&opts.out);
    smf.save(&tmp)
        .map_err(|e| format!("Error writing '{}': {e}", tmp.display()))?;
    if let Err(e) = std::fs::rename(&tmp, &opts.out) {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("Error moving output into place: {e}"));
    }

    tracing::info!(
        "Exported {} ({} track{}, {} notes, {} PPQ)",
        opts.out.display(),
        dump.tracks.len(),
        if dump.tracks.len() == 1 { "" } else { "s" },
        dump.note_count,
        PPQ,
    );
    Ok(())
}

fn build_tempo_track(tempo_map: &[(Beats, u32)]) -> Vec<TrackEvent<'static>> {
    let mut sorted = tempo_map.to_vec();
    sorted.sort_by_key(|&(pos, _)| pos);

    let mut track: Vec<TrackEvent<'static>> = Vec::new();
    let mut last_tick = 0u32;
    let mut prev_bpm: Option<u32> = None;
    for (pos, bpm) in sorted {
        if prev_bpm == Some(bpm) {
            continue;
        }
        prev_bpm = Some(bpm);
        let us_per_qn = (60_000_000u32 / bpm.max(1)).min(MAX_US_PER_QN);
        let tick = ticks(beats_to_f64(pos));
        track.push(TrackEvent {
            delta: u28::new(tick.saturating_sub(last_tick)),
            kind: TrackEventKind::Meta(MetaMessage::Tempo(u24::new(us_per_qn))),
        });
        last_tick = tick;
    }
    track.push(TrackEvent {
        delta: u28::new(0),
        kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
    });
    track
}
