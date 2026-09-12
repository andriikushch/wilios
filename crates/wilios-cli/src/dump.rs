//! Offline event dump: run the pipeline and emit the per-track note timeline
//! as JSON or an aligned text table. No audio device, no synthesis — just the
//! interpreter's scheduled events.

use wilios_core::dump::{EventDump, build_dump, render_piano_roll};
use wilios_core::interpreter::interpreter::Interpreter;

use crate::render::DEFAULT_MAX_RENDER_SECS;

/// Safety cap when no `--duration` is given: a piece that has not ended after
/// this many seconds of composition time is treated as non-terminating.
pub const DEFAULT_MAX_SECS: f32 = DEFAULT_MAX_RENDER_SECS;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DumpFormat {
    Json,
    Text,
    /// ASCII piano roll — a 16th-note grid per track.
    Roll,
}

pub struct DumpOpts {
    pub format: DumpFormat,
    /// `Some(secs)` dumps exactly that much composition time and never errors
    /// on a non-terminating piece; `None` dumps until every track finishes,
    /// bounded by `max_secs`.
    pub duration: Option<f32>,
    pub max_secs: f32,
}

impl DumpOpts {
    pub fn new(format: DumpFormat) -> Self {
        Self {
            format,
            duration: None,
            max_secs: DEFAULT_MAX_SECS,
        }
    }
}

/// Run `interp` to completion (or to the configured bound) and format the
/// event timeline. Returns the payload for the caller to print to stdout.
pub fn dump(mut interp: Interpreter, opts: DumpOpts) -> Result<String, String> {
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

    let d = build_dump(&events, finished);
    match opts.format {
        DumpFormat::Json => {
            serde_json::to_string_pretty(&d).map_err(|e| format!("Error serializing dump: {e}"))
        }
        DumpFormat::Text => Ok(render_text(&d)),
        DumpFormat::Roll => Ok(render_piano_roll(&d)),
    }
}

fn render_text(d: &EventDump) -> String {
    use std::fmt::Write as _;

    let plural = |n: usize| if n == 1 { "" } else { "s" };

    let mut s = String::new();
    let _ = writeln!(
        s,
        "# {} note{} across {} track{}, {:.3}s{}",
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
        let _ = writeln!(
            s,
            "\ntrack {}  ({} note{})",
            track.id,
            track.notes.len(),
            plural(track.notes.len())
        );
        let _ = writeln!(
            s,
            "  {:>9}  {:>8}  {:<5}  {:>9}  {:>7}  {:>9}  {:>4}  {:>4}  wave",
            "at_ms", "at_beats", "pitch", "freq_hz", "dur_ms", "dur_beats", "vel", "pan",
        );
        for n in &track.notes {
            let _ = writeln!(
                s,
                "  {:>9}  {:>8}  {:<5}  {:>9.2}  {:>7}  {:>9}  {:>4}  {:>4}  {}",
                n.at_ms,
                n.at_beats,
                n.pitch,
                n.freq_hz,
                n.dur_ms,
                n.dur_beats,
                n.velocity,
                n.pan,
                n.waveform,
            );
        }
    }
    s
}
