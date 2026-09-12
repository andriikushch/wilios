//! The single mapping from interpreter [`Event`]s to synth [`Voice`]s.
//!
//! Both the live and offline consumers pull events one buffer at a time and turn
//! every `Note` into a freshly spawned voice; keeping that here means the two
//! paths can't drift apart.

use wilios_core::interpreter::RuntimeError;
use wilios_core::interpreter::event::EventKind;
use wilios_core::interpreter::interpreter::Interpreter;
use wilios_core::interpreter::pitch::{Accidental, Pitch, PitchName, note_frequency};
use wilios_synth::Voice;

/// Advance `interp` up to `until_ms` and push a new [`Voice`] onto `out` for
/// every note event produced in that window.
///
/// `until_ms` is a running, ever-increasing cumulative timestamp (same contract
/// as [`Interpreter::schedule_until`]); callers pass the end of the buffer they
/// are about to render.
pub fn drain_new_voices(
    interp: &mut Interpreter,
    until_ms: u64,
    sample_rate: f32,
    out: &mut Vec<Voice>,
) -> Result<(), RuntimeError> {
    let events = interp.schedule_until(0, until_ms)?;
    for ev in events {
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
        out.push(Voice::new(
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
        ));
    }
    Ok(())
}
