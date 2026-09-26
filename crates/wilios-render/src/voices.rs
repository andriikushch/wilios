//! The single mapping from interpreter [`Event`]s to synth [`Voice`]s.
//!
//! Both the live and offline consumers pull events one buffer at a time and turn
//! every `Note` into a freshly spawned voice; keeping that here means the two
//! paths can't drift apart.

use wilios_core::interpreter::RuntimeError;
use wilios_core::interpreter::event::{Event, EventKind};
use wilios_core::interpreter::interpreter::Interpreter;
use wilios_core::interpreter::pitch::{Accidental, Pitch, PitchName, note_frequency};
use wilios_synth::Voice;

/// Turns one note event into a voice, at its own volume/tone settings.
fn voice_for(ev: Event, sample_rate: f32) -> Voice {
    let EventKind::Note {
        pitch,
        duration,
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
        ..
    } = ev.kind;
    let freq = note_frequency(
        Pitch {
            name: PitchName::from_string(pitch.letter),
            accidental: Accidental::from_int(pitch.accidental),
        },
        pitch.octave as u8,
    );
    Voice::new(
        freq,
        sample_rate,
        volume as f32 / 127.0,
        duration,
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
    )
}

/// Spawns voices at their written time, to the sample.
///
/// A note's `Event::at` is where it is meant to sound, which is not necessarily
/// the start of the buffer it was scheduled in: `offset`, `swing` and a piece's
/// own subdivisions all put onsets inside a buffer, and an event can be produced
/// one buffer before it is due. So events wait here until their buffer arrives
/// and then spawn with a sub-buffer delay.
#[derive(Default)]
pub struct VoiceScheduler {
    /// Scheduled, not yet due — sorted by time.
    pending: Vec<(u64, Event)>,
}

impl VoiceScheduler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Advance `interp` to `buffer_end_ms` and push onto `out` a [`Voice`] for
    /// every note due before the end of this buffer, each delayed to its exact
    /// onset within it.
    ///
    /// `buffer_start_ms` / `buffer_end_ms` are running cumulative timestamps
    /// (same contract as [`Interpreter::schedule_until`]) bounding the buffer the
    /// caller is about to render.
    pub fn drain_new_voices(
        &mut self,
        interp: &mut Interpreter,
        buffer_start_ms: u64,
        buffer_end_ms: u64,
        sample_rate: f32,
        out: &mut Vec<Voice>,
    ) -> Result<(), RuntimeError> {
        for ev in interp.schedule_until(0, buffer_end_ms)? {
            self.pending.push((ev.at, ev));
        }
        // A changed `offset` can emit onsets out of order, so sort rather than
        // assume the interpreter's emission order is the sounding order.
        self.pending.sort_by_key(|(at, _)| *at);

        let due = self.pending.partition_point(|(at, _)| *at < buffer_end_ms);
        for (at, ev) in self.pending.drain(..due) {
            // An onset already past (a note pulled earlier by a negative offset)
            // sounds now rather than never.
            let delay = (at.saturating_sub(buffer_start_ms)) as f64 * sample_rate as f64 / 1000.0;
            out.push(voice_for(ev, sample_rate).delayed(delay as u32));
        }
        Ok(())
    }
}
