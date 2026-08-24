use std::cmp::Ordering;

use crate::parser::ast::{Pitch, TimeSignature, Waveform};
use crate::time::Beats;

/// A single FM operator's evaluated configuration (snapshotted at note-emit time).
#[derive(Clone, Debug, PartialEq)]
pub struct FmOpConfig {
    pub id: usize,
    pub ratio: f32,
    pub level: f32,
    pub wave: Waveform,
    pub attack_ms: f32,
    pub decay_ms: f32,
    pub sustain_level: f32,
    pub release_ms: f32,
}

/// Multi-operator FM patch configuration carried by a Note event.
#[derive(Clone, Debug, PartialEq)]
pub struct FmBlockConfig {
    pub ops: Vec<FmOpConfig>,
    pub algorithm: Vec<(usize, usize)>, // (modulator_id, target_id)
}

pub type TrackId = usize;
pub type TimeMs = u64; // todo

#[derive(Clone)]
pub struct Event {
    pub at: TimeMs,
    /// Exact nominal position (whole-note units) this event's `at` was derived
    /// from. Two tracks reaching the same `at_beats` via different subdivision
    /// paths are guaranteed to share the same `at` — useful for analysis/tests,
    /// and a disagreement between the two is meaningful (e.g. swing), not noise.
    pub at_beats: Beats,
    pub track: TrackId,
    pub kind: EventKind,
}

impl Ord for Event {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse time ordering (min-heap behavior)
        other
            .at
            .cmp(&self.at)
            // Tie-breaker: track id (stable-ish ordering)
            .then_with(|| self.track.cmp(&other.track))
    }
}

impl PartialOrd for Event {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Event {
    fn eq(&self, other: &Self) -> bool {
        self.at == other.at && self.track == other.track && self.kind == other.kind
    }
}

// f32 fields in EventKind prevent deriving Eq; manually assert reflexivity for Ord's sake.
impl Eq for Event {}

#[derive(Clone, PartialEq)]
pub enum EventKind {
    Note {
        pitch: Pitch,
        duration: TimeMs,
        duration_beats: Beats,
        pan: isize,
        volume: usize,
        waveform: Waveform,
        attack_ms: f32,
        decay_ms: f32,
        sustain_level: f32,
        release_ms: f32,
        fm_ratio: f32,
        fm_depth: f32,
        fm_block: Option<FmBlockConfig>,
        time_signature: TimeSignature,
    },
}
