//! Headless CI smoke check: run the pipeline and assert the piece
//! **(a)** schedules to completion with no runtime error and **(b)** every
//! track reaches the same nominal end position. No audio device, no output
//! file — the process exit code is the contract.
//!
//! (b) is only meaningful for a run that ended on its own: a `--duration`-bounded
//! (or endless) piece stops each track at an arbitrary position, so the
//! equal-end-time check is skipped there and only (a) is enforced.

use std::fmt::Write as _;

use wilios_core::interpreter::interpreter::Interpreter;
use wilios_core::time::Beats;

use crate::render::DEFAULT_MAX_RENDER_SECS;

/// Safety cap when no `--duration` is given: a piece that has not ended after
/// this many seconds of composition time is treated as non-terminating.
pub const DEFAULT_MAX_SECS: f32 = DEFAULT_MAX_RENDER_SECS;

pub struct SmokeOpts {
    /// `Some(secs)` schedules exactly that much and does not require natural
    /// termination; the equal-end-time check is skipped (the run is truncated).
    /// `None` requires the piece to finish within `max_secs`.
    pub duration: Option<f32>,
    pub max_secs: f32,
}

impl SmokeOpts {
    pub fn new() -> Self {
        Self {
            duration: None,
            max_secs: DEFAULT_MAX_SECS,
        }
    }
}

impl Default for SmokeOpts {
    fn default() -> Self {
        Self::new()
    }
}

/// Run `interp` and check it schedules cleanly and its tracks stay aligned.
/// Returns a one-line human summary on success; an error string on failure.
pub fn smoke(mut interp: Interpreter, opts: SmokeOpts) -> Result<String, String> {
    let bound_ms: u64 = match opts.duration {
        Some(secs) => (secs as f64 * 1000.0).round() as u64,
        None => (opts.max_secs as f64 * 1000.0).round() as u64,
    };

    // (a) schedules with no runtime error.
    let (_events, finished) = interp.schedule_to_end(bound_ms).map_err(|e| e.0)?;

    if opts.duration.is_none() && !finished {
        return Err(format!(
            "piece did not finish within {:.0}s and no --duration was given — \
             it likely contains an endless loop (`loop (true) {{ … }}`) or a \
             note shorter than its attack that never releases. \
             Re-run with --duration <seconds>.",
            opts.max_secs
        ));
    }

    // (track_id, nominal end position, derived ms end position).
    let track_ends: Vec<(usize, Beats, u64)> = interp
        .tracks
        .iter()
        .map(|t| (t.ctx.track_id, t.ctx.nominal_position, t.ctx.time))
        .collect();

    if track_ends.is_empty() {
        return Ok("ok: 0 tracks — nothing scheduled".to_string());
    }

    // A --duration-bounded run truncates each track at an arbitrary position,
    // so (b) can't be checked — only (a) applies.
    if let Some(secs) = opts.duration {
        return Ok(format!(
            "ok (bounded to {secs:.0}s, finished={finished}): {} track(s) scheduled with no \
             runtime error; equal-end-time check skipped for a --duration-bounded run",
            track_ends.len()
        ));
    }

    // (b) all tracks reach the same end time.
    let (_, first_pos, _) = track_ends[0];
    if track_ends.iter().any(|&(_, pos, _)| pos != first_pos) {
        let mut detail = String::from("tracks do not all reach the same end time:\n");
        for (id, pos, ms) in &track_ends {
            let _ = writeln!(detail, "  track {id}: {pos} whole notes ({ms} ms)");
        }
        return Err(detail);
    }

    let (_, pos, ms) = track_ends[0];
    Ok(format!(
        "ok: {} track(s) scheduled with no runtime error; all end at {pos} whole notes ({ms} ms)",
        track_ends.len()
    ))
}
