//! Device-independent offline render path, lifted out of `wilios-cli` so hosts
//! that must not link an audio device (notably `wilios-mcp`) can still turn a
//! `.wilios` source into audio, images, and analysis.
//!
//! - [`pipeline`] — `lex → parse → interpret`, shared with live playback.
//! - [`voices`] — the one [`wilios_synth::Voice`] mapping, shared with live playback.
//! - [`render`] — drive the mixer to a WAV file or an in-memory sample buffer.
//! - [`analysis`] — peak / RMS / clipping / silence / per-track stats.
//! - [`image`] — waveform and log-frequency spectrogram PNGs.

pub mod analysis;
pub mod image;
pub mod pipeline;
pub mod render;
pub mod voices;
