//! Device-independent core of the `wilios` CLI.
//!
//! `main.rs` is only argument parsing and dispatch; everything below the CLI —
//! loading a `.wilios` file into an [`Interpreter`], turning its events into
//! synth [`Voice`]s, and consuming them — lives here so it can be reused and
//! tested without opening an audio device.
//!
//! [`Interpreter`]: wilios_core::interpreter::interpreter::Interpreter
//! [`Voice`]: wilios_synth::Voice

pub mod dump;
pub mod midi;
pub mod play;
pub mod progress;
pub mod smoke;

// The device-independent render path now lives in `wilios-render` so hosts that
// must not link `cpal` can reuse it. Re-exported here so `main.rs` and the
// `tests/` crates keep their `wilios_cli::{pipeline,render,voices}` paths.
pub use wilios_render::{pipeline, render, voices};
