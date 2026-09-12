//! Cheap, scalar-only analysis of a rendered buffer — the "did it work" summary
//! an agent reads before (or instead of) listening: level, clipping, silence,
//! and per-track note counts / pitch range.

use serde::Serialize;

use wilios_core::dump::build_dump;
use wilios_core::interpreter::event::Event;

use crate::render::RenderSamples;

const SILENCE_FLOOR: f32 = 1e-4;
/// The mixer's soft-clip saturator asymptotes to ±1.0 and never truly clips, so
/// "clipped" here means a sample driven deep into the tanh region (heavy,
/// audible squashing) rather than a hard digital clip.
const CLIP_LEVEL: f32 = 0.9;
/// The saturator's linear-region knee — above this the mix is being shaped.
const LIMITER_LEVEL: f32 = 0.8;
const MIN_DBFS: f32 = -160.0;

#[derive(Debug, Clone, Serialize)]
pub struct RenderAnalysis {
    /// Loudest sample, in dBFS. `MIN_DBFS` (≈ -inf) for pure silence.
    pub peak_dbfs: f32,
    /// RMS level over the whole buffer, in dBFS.
    pub rms_dbfs: f32,
    /// Samples driven deep into the soft-clip saturator (`|s| >= 0.9`) — the
    /// mixer never hard-clips, so a non-zero count means audible squashing.
    pub clipped_samples: u64,
    /// Fraction of samples past the saturator knee (`|s| > 0.8`) — a proxy for
    /// how hard the mixer's (internal, non-reporting) limiter is working.
    pub limiter_engaged_ratio: f32,
    pub leading_silence_ms: u64,
    pub trailing_silence_ms: u64,
    pub tracks: Vec<TrackAnalysis>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrackAnalysis {
    pub id: usize,
    pub note_count: usize,
    /// Lowest / highest sounding pitch, scientific spelling (e.g. `"C4"`).
    pub lowest_pitch: Option<String>,
    pub highest_pitch: Option<String>,
}

fn dbfs(amp: f32) -> f32 {
    if amp <= 0.0 {
        MIN_DBFS
    } else {
        (20.0 * amp.log10()).max(MIN_DBFS)
    }
}

pub fn analyze(s: &RenderSamples, events: &[Event], finished: bool) -> RenderAnalysis {
    let n = s.interleaved.len();
    let mut peak = 0f32;
    let mut sum_sq = 0f64;
    let mut clipped = 0u64;
    let mut limiter_hits = 0u64;
    for &v in &s.interleaved {
        let a = v.abs();
        if a > peak {
            peak = a;
        }
        sum_sq += (v as f64) * (v as f64);
        if a >= CLIP_LEVEL {
            clipped += 1;
        }
        if a > LIMITER_LEVEL {
            limiter_hits += 1;
        }
    }
    let rms = if n == 0 {
        0.0
    } else {
        (sum_sq / n as f64).sqrt() as f32
    };
    let limiter_engaged_ratio = if n == 0 {
        0.0
    } else {
        limiter_hits as f32 / n as f32
    };

    let ch = s.channels.max(1) as usize;
    let frames = s.frames();
    let frame_loud = |f: usize| -> bool {
        let base = f * ch;
        s.interleaved[base..base + ch]
            .iter()
            .any(|v| v.abs() > SILENCE_FLOOR)
    };
    let lead_frames = (0..frames).take_while(|&f| !frame_loud(f)).count();
    let frames_to_ms = |fr: usize| -> u64 {
        if s.sample_rate == 0 {
            0
        } else {
            (fr as f64 / s.sample_rate as f64 * 1000.0).round() as u64
        }
    };
    let (leading_silence_ms, trailing_silence_ms) = if lead_frames >= frames {
        // Entirely silent: report the whole length as leading silence.
        (frames_to_ms(frames), 0)
    } else {
        let trail_frames = (0..frames).rev().take_while(|&f| !frame_loud(f)).count();
        (frames_to_ms(lead_frames), frames_to_ms(trail_frames))
    };

    let dump = build_dump(events, finished);
    let tracks = dump
        .tracks
        .iter()
        .map(|t| {
            let mut lowest: Option<(&str, u8)> = None;
            let mut highest: Option<(&str, u8)> = None;
            for note in &t.notes {
                if lowest.is_none_or(|(_, m)| note.midi_note < m) {
                    lowest = Some((note.pitch.as_str(), note.midi_note));
                }
                if highest.is_none_or(|(_, m)| note.midi_note > m) {
                    highest = Some((note.pitch.as_str(), note.midi_note));
                }
            }
            TrackAnalysis {
                id: t.id,
                note_count: t.notes.len(),
                lowest_pitch: lowest.map(|(p, _)| p.to_string()),
                highest_pitch: highest.map(|(p, _)| p.to_string()),
            }
        })
        .collect();

    RenderAnalysis {
        peak_dbfs: dbfs(peak),
        rms_dbfs: dbfs(rms),
        clipped_samples: clipped,
        limiter_engaged_ratio,
        leading_silence_ms,
        trailing_silence_ms,
        tracks,
    }
}
