//! Serializable snapshot of a scheduled composition — the data behind
//! `wilios dump` (CLI) and the `dump_events` MCP tool.
//!
//! `wilios-core` is otherwise serde-free by design (see `diagnostics/mod.rs`);
//! this module is the single exception, gated behind the off-by-default `serde`
//! feature. Two separate transports need byte-for-byte the same event JSON —
//! the CLI, and the MCP server, which cannot depend on the CLI — and the
//! full-fidelity mapping (nested FM-operator config included) is not worth
//! maintaining in two places.

use serde::Serialize;

use std::fmt::Write as _;

use crate::interpreter::event::{Event, EventKind, FmBlockConfig};
use crate::interpreter::pitch::{Accidental, PitchName, midi_note_number, note_frequency};
use crate::parser::ast::{Pitch, TimeSignature, Waveform};
use crate::time::Beats;

/// Every scheduled note, grouped by track.
#[derive(Debug, Clone, Serialize)]
pub struct EventDump {
    /// `false` if scheduling stopped at a time bound rather than the piece
    /// ending on its own (endless loop, or a piece longer than the bound).
    pub finished: bool,
    /// End of the last note (`max(at_ms + dur_ms)`); `0` when there are no notes.
    pub duration_ms: u64,
    /// Total note count across all tracks.
    pub note_count: usize,
    pub tracks: Vec<TrackDump>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrackDump {
    pub id: usize,
    pub notes: Vec<NoteDump>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NoteDump {
    pub at_ms: u64,
    /// Exact nominal onset in whole-note units, e.g. `"1/4"`, `"2"`.
    pub at_beats: String,
    pub at_beats_f64: f64,
    /// Scientific pitch spelling, e.g. `"C4"`, `"F#5"`, `"Eb3"`.
    pub pitch: String,
    /// MIDI note number (0–127), C4 = 60.
    pub midi_note: u8,
    pub freq_hz: f32,
    pub dur_ms: u64,
    pub dur_beats: String,
    pub dur_beats_f64: f64,
    pub velocity: usize,
    pub pan: isize,
    pub waveform: &'static str,
    pub attack_ms: f32,
    pub decay_ms: f32,
    pub sustain_level: f32,
    pub release_ms: f32,
    pub fm_ratio: f32,
    pub fm_depth: f32,
    pub fm_block: Option<FmBlockDump>,
    /// Per-track resonant low-pass: cutoff in Hz (20000 = open), resonance 0..1.
    pub cutoff_hz: f32,
    pub resonance: f32,
    /// Per-track vibrato: LFO depth in cents, rate in Hz (0 = off).
    pub vibrato_depth_cents: f32,
    pub vibrato_rate_hz: f32,
    /// Time signature in force at this note, e.g. `"4/4"`.
    pub time_signature: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FmBlockDump {
    pub ops: Vec<FmOpDump>,
    /// Routing edges as `[modulator_id, target_id]` pairs.
    pub algorithm: Vec<[usize; 2]>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FmOpDump {
    pub id: usize,
    pub ratio: f32,
    pub level: f32,
    pub wave: &'static str,
    pub attack_ms: f32,
    pub decay_ms: f32,
    pub sustain_level: f32,
    pub release_ms: f32,
}

/// Canonical short name for a waveform, matching the DSL keyword.
pub fn waveform_name(w: &Waveform) -> &'static str {
    match w {
        Waveform::Sine => "sine",
        Waveform::Square => "square",
        Waveform::Saw => "saw",
        Waveform::Triangle => "tri",
    }
}

/// `"C4"`, `"F#5"`, `"Eb3"` — mirrors the interpreter's own `Value::Pitch`
/// formatting.
pub fn pitch_name(p: &Pitch) -> String {
    let acc = match p.accidental {
        1 => "#",
        -1 => "b",
        _ => "",
    };
    format!("{}{}{}", p.letter, acc, p.octave)
}

fn pitch_shim(p: &Pitch) -> crate::interpreter::pitch::Pitch {
    crate::interpreter::pitch::Pitch {
        name: PitchName::from_string(p.letter),
        accidental: Accidental::from_int(p.accidental),
    }
}

fn pitch_freq(p: &Pitch) -> f32 {
    note_frequency(pitch_shim(p), p.octave as u8)
}

fn pitch_midi(p: &Pitch) -> u8 {
    midi_note_number(pitch_shim(p), p.octave as u8)
}

fn beats_f64(b: &Beats) -> f64 {
    *b.numer() as f64 / *b.denom() as f64
}

fn time_sig_str(ts: &TimeSignature) -> String {
    format!("{}/{}", ts.numerator, ts.denominator)
}

impl FmBlockDump {
    fn from_config(c: &FmBlockConfig) -> Self {
        FmBlockDump {
            ops: c
                .ops
                .iter()
                .map(|o| FmOpDump {
                    id: o.id,
                    ratio: o.ratio,
                    level: o.level,
                    wave: waveform_name(&o.wave),
                    attack_ms: o.attack_ms,
                    decay_ms: o.decay_ms,
                    sustain_level: o.sustain_level,
                    release_ms: o.release_ms,
                })
                .collect(),
            algorithm: c.algorithm.iter().map(|&(s, d)| [s, d]).collect(),
        }
    }
}

impl NoteDump {
    fn from_event(ev: &Event) -> Self {
        let EventKind::Note {
            pitch,
            duration,
            duration_beats,
            pan,
            volume,
            waveform,
            attack_ms,
            decay_ms,
            sustain_level,
            release_ms,
            fm_ratio,
            fm_depth,
            fm_block,
            cutoff_hz,
            resonance,
            vibrato_depth_cents,
            vibrato_rate_hz,
            time_signature,
        } = &ev.kind;
        NoteDump {
            at_ms: ev.at,
            at_beats: ev.at_beats.to_string(),
            at_beats_f64: beats_f64(&ev.at_beats),
            pitch: pitch_name(pitch),
            midi_note: pitch_midi(pitch),
            freq_hz: pitch_freq(pitch),
            dur_ms: *duration,
            dur_beats: duration_beats.to_string(),
            dur_beats_f64: beats_f64(duration_beats),
            velocity: *volume,
            pan: *pan,
            waveform: waveform_name(waveform),
            attack_ms: *attack_ms,
            decay_ms: *decay_ms,
            sustain_level: *sustain_level,
            release_ms: *release_ms,
            fm_ratio: *fm_ratio,
            fm_depth: *fm_depth,
            fm_block: fm_block.as_ref().map(FmBlockDump::from_config),
            cutoff_hz: *cutoff_hz,
            resonance: *resonance,
            vibrato_depth_cents: *vibrato_depth_cents,
            vibrato_rate_hz: *vibrato_rate_hz,
            time_signature: time_sig_str(time_signature),
        }
    }
}

/// Group `events` by track (ascending track id), each track's notes sorted by
/// onset. `finished` is threaded straight through to [`EventDump::finished`].
pub fn build_dump(events: &[Event], finished: bool) -> EventDump {
    let mut track_ids: Vec<usize> = events.iter().map(|e| e.track).collect();
    track_ids.sort_unstable();
    track_ids.dedup();

    let tracks = track_ids
        .into_iter()
        .map(|id| {
            let mut notes: Vec<NoteDump> = events
                .iter()
                .filter(|e| e.track == id)
                .map(NoteDump::from_event)
                .collect();
            notes.sort_by_key(|n| n.at_ms);
            TrackDump { id, notes }
        })
        .collect();

    let duration_ms = events
        .iter()
        .map(|e| {
            let EventKind::Note { duration, .. } = &e.kind;
            e.at + *duration
        })
        .max()
        .unwrap_or(0);

    EventDump {
        finished,
        duration_ms,
        note_count: events.len(),
        tracks,
    }
}

/// 16th-note cells per whole note — the piano-roll grid resolution.
const CELLS_PER_WHOLE: usize = 16;

/// Sharp-spelled scientific name for a MIDI note number, e.g. `60 -> "C4"`.
fn midi_note_name(n: u8) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    format!("{}{}", NAMES[(n % 12) as usize], n as i32 / 12 - 1)
}

/// Bar length in grid cells from a `"num/den"` time-signature string; falls back
/// to a 4/4 bar (16 cells) on anything unexpected.
fn bar_cells(time_signature: &str) -> usize {
    time_signature
        .split_once('/')
        .and_then(|(num, den)| {
            let num: usize = num.trim().parse().ok()?;
            let den: usize = den.trim().parse().ok()?;
            (num > 0 && den > 0).then(|| CELLS_PER_WHOLE * num / den)
        })
        .filter(|&c| c > 0)
        .unwrap_or(CELLS_PER_WHOLE)
}

/// ASCII piano roll of a scheduled composition — one 16th-note-resolution grid
/// per track, for eyeballing rhythm and voice-leading without listening.
///
/// `#` = note onset, `=` = sustained, `.` = empty; `|` marks bar boundaries.
/// Rows run high pitch (top) to low, spanning every chromatic step the track
/// actually uses. Wide pieces wrap into stacked panels of 8 bars. Meter is taken
/// from the track's first note (a mid-track change is not reflected in the grid).
pub fn render_piano_roll(d: &EventDump) -> String {
    const PANEL_BARS: usize = 8;
    let cell_of = |beats: f64| (beats * CELLS_PER_WHOLE as f64).round() as i64;

    let plural = |n: usize| if n == 1 { "" } else { "s" };
    let mut s = String::new();

    let _ = writeln!(
        s,
        "# piano roll — {} note{} across {} track{}, {:.3}s{}",
        d.note_count,
        plural(d.note_count),
        d.tracks.len(),
        plural(d.tracks.len()),
        d.duration_ms as f64 / 1000.0,
        if d.finished {
            ""
        } else {
            "  (truncated — piece did not finish)"
        },
    );

    for track in &d.tracks {
        if track.notes.is_empty() {
            let _ = writeln!(s, "\ntrack {}  (no notes)", track.id);
            continue;
        }

        let ts = track.notes[0].time_signature.as_str();
        let bar = bar_cells(ts);
        let min_midi = track.notes.iter().map(|n| n.midi_note).min().unwrap();
        let max_midi = track.notes.iter().map(|n| n.midi_note).max().unwrap();
        let rows = (max_midi - min_midi) as usize + 1;

        let total_cells = track
            .notes
            .iter()
            .map(|n| (cell_of(n.at_beats_f64) + cell_of(n.dur_beats_f64).max(1)).max(0) as usize)
            .max()
            .unwrap_or(0);
        let total_bars = total_cells.div_ceil(bar).max(1);
        let padded = total_bars * bar;

        // grid[row][cell]: b'.', b'#', or b'='   (row 0 = highest pitch)
        let mut grid = vec![vec![b'.'; padded]; rows];
        for n in &track.notes {
            let start = cell_of(n.at_beats_f64);
            if !(0..padded as i64).contains(&start) {
                continue;
            }
            let start = start as usize;
            let len = cell_of(n.dur_beats_f64).max(1) as usize;
            let row = &mut grid[(max_midi - n.midi_note) as usize];
            row[start] = b'#';
            for cell in &mut row[start + 1..(start + len).min(padded)] {
                if *cell == b'.' {
                    *cell = b'=';
                }
            }
        }

        let _ = writeln!(
            s,
            "\ntrack {}  ({}, {} note{}, {} bar{})",
            track.id,
            ts,
            track.notes.len(),
            plural(track.notes.len()),
            total_bars,
            plural(total_bars),
        );

        let mut first_bar = 0;
        while first_bar < total_bars {
            let last_bar = (first_bar + PANEL_BARS).min(total_bars);

            let mut ruler = String::from("     ");
            for b in first_bar..last_bar {
                let _ = write!(ruler, " |{:<width$}", b + 1, width = bar);
            }
            ruler.push_str(" |");
            let _ = writeln!(s, "{ruler}");

            for (row, cells) in grid.iter().enumerate() {
                let _ = write!(s, "{:>4} ", midi_note_name(max_midi - row as u8));
                for (i, &cell) in cells[first_bar * bar..last_bar * bar].iter().enumerate() {
                    if i % bar == 0 {
                        s.push_str(" |");
                    }
                    s.push(cell as char);
                }
                s.push_str(" |\n");
            }
            first_bar = last_bar;
        }
    }

    s
}

#[cfg(test)]
mod tests {
    use crate::interpreter::interpreter::Interpreter;
    use crate::lexer::Lexer;
    use crate::parser::parser::Parser;

    fn dump_src(src: &str, max_ms: u64) -> super::EventDump {
        let tokens = Lexer::new(src).lex().unwrap();
        let program = Parser::new(tokens).parse().unwrap();
        let mut interp = Interpreter::new(program).unwrap();
        let (events, finished) = interp.schedule_to_end(max_ms).unwrap();
        super::build_dump(&events, finished)
    }

    #[test]
    fn groups_by_track_and_sorts_by_onset() {
        let d = dump_src(
            "tempo 120\ntrack 1\n<C4> 1/4\n<E4> 1/4\ntrack 2\n<G3> 1/2\n",
            60_000,
        );
        assert!(d.finished);
        assert_eq!(d.note_count, 3);
        assert_eq!(d.tracks.len(), 2);
        assert_eq!(d.tracks[0].id, 1);
        assert_eq!(d.tracks[1].id, 2);

        let t1 = &d.tracks[0].notes;
        assert_eq!(t1[0].at_ms, 0);
        assert_eq!(t1[0].pitch, "C4");
        assert_eq!(t1[0].at_beats, "0");
        assert_eq!(t1[0].dur_ms, 500); // 1/4 note @ 120 BPM
        assert_eq!(t1[0].dur_beats, "1/4");
        assert_eq!(t1[1].at_ms, 500);
        assert_eq!(t1[1].pitch, "E4");
        assert_eq!(d.duration_ms, 1000);
    }

    #[test]
    fn accidentals_render_in_scientific_notation() {
        let d = dump_src("track 1\n<F#4> 1/8\n<Eb3> 1/8\n", 60_000);
        let notes = &d.tracks[0].notes;
        assert_eq!(notes[0].pitch, "F#4");
        assert_eq!(notes[1].pitch, "Eb3");
    }

    #[test]
    fn endless_loop_is_marked_unfinished_and_bounded() {
        let d = dump_src("track 1\nloop (true) {\n<C4> 1/8\n}\n", 2_000);
        assert!(!d.finished);
        assert!(!d.tracks[0].notes.is_empty());
        assert!(d.tracks[0].notes.iter().all(|n| n.at_ms < 2_000));
    }

    #[test]
    fn midi_note_is_stamped_on_notes() {
        let d = dump_src("track 1\n<C4> 1/4\n<A4> 1/4\n", 60_000);
        assert_eq!(d.tracks[0].notes[0].midi_note, 60);
        assert_eq!(d.tracks[0].notes[1].midi_note, 69);
    }

    #[test]
    fn piano_roll_places_onsets_on_the_grid() {
        let d = dump_src("track 1\n<C4> 1/4\n<E4> 1/4\n", 60_000);
        let roll = super::render_piano_roll(&d);

        let e4 = roll.lines().find(|l| l.starts_with("  E4 ")).unwrap();
        let c4 = roll.lines().find(|l| l.starts_with("  C4 ")).unwrap();
        // 5-col label + " |" bar separator, then the 16-cell bar.
        assert_eq!(c4.as_bytes()[7], b'#'); // C4 onset at cell 0
        assert_eq!(e4.as_bytes()[11], b'#'); // E4 onset at cell 4 (1/4 note in)
        assert_eq!(&c4[7..11], "#===");
        assert!(roll.contains("track 1  (4/4, 2 notes, 1 bar)"));
        // rows span C4..E4 inclusive (5 chromatic steps); the ruler line has a
        // blank label, grid rows carry a note name.
        let grid_rows = roll
            .lines()
            .filter(|l| l.len() > 7 && &l[5..7] == " |" && !l[..5].trim().is_empty())
            .count();
        assert_eq!(grid_rows, 5);
    }

    #[test]
    fn piano_roll_renders_each_track_as_its_own_block() {
        let d = dump_src("track 1\n<C4> 1/4\ntrack 2\n<G3> 1/4\n", 60_000);
        let roll = super::render_piano_roll(&d);
        assert!(roll.contains("track 1  (4/4, 1 note, 1 bar)"));
        assert!(roll.contains("track 2  (4/4, 1 note, 1 bar)"));
    }
}
