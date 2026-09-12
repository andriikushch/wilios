//! A one-line stderr progress bar for `wilios render`.
//!
//! stdout is reserved (dump payloads here, JSON-RPC in `wilios-mcp`), so the bar
//! writes to stderr with a carriage return and no newline, redrawing in place.
//! It stays silent unless stderr is a real terminal, so piped / redirected / CI
//! output is never polluted with `\r` spam — the completion `tracing::info!`
//! line remains the machine-readable signal.

use std::io::{IsTerminal, Write};
use std::time::{Duration, Instant};

use wilios_render::render::RenderProgress;

const BAR_WIDTH: usize = 24;
const MIN_REDRAW: Duration = Duration::from_millis(100);

pub struct RenderBar {
    enabled: bool,
    last_draw: Instant,
    drawn: bool,
}

impl RenderBar {
    /// `enabled` reflects the `--no-progress` flag; the bar additionally requires
    /// stderr to be a TTY. When off, every method is a no-op.
    pub fn new(enabled: bool) -> Self {
        RenderBar {
            enabled: enabled && std::io::stderr().is_terminal(),
            last_draw: Instant::now(),
            drawn: false,
        }
    }

    /// Redraw the bar from a render progress ping. In-place draws are throttled
    /// to ~10 fps; the final ping (`p.done`) always draws and closes the line
    /// with a newline so whatever prints next — the `Rendered …` log line —
    /// starts clean.
    pub fn update(&mut self, p: &RenderProgress) {
        if !self.enabled {
            return;
        }
        if self.drawn && !p.done && self.last_draw.elapsed() < MIN_REDRAW {
            return;
        }
        self.last_draw = Instant::now();

        let mut err = std::io::stderr().lock();
        let _ = match (p.fraction(), p.seconds_total()) {
            (Some(frac), Some(total)) => {
                let filled = (frac * BAR_WIDTH as f64).round() as usize;
                write!(
                    err,
                    "\r\u{1b}[K  rendering  \u{2595}{}{}\u{258f}  {:>3.0}%  {:.1}s / {:.1}s",
                    "\u{2588}".repeat(filled),
                    "\u{2591}".repeat(BAR_WIDTH - filled),
                    frac * 100.0,
                    p.seconds_done(),
                    total,
                )
            }
            // Open-ended render: no known total, so show elapsed audio seconds.
            _ => write!(
                err,
                "\r\u{1b}[K  rendering  {:.1}s \u{2026}",
                p.seconds_done()
            ),
        };
        if p.done {
            let _ = err.write_all(b"\n");
            self.drawn = false;
        } else {
            self.drawn = true;
        }
        let _ = err.flush();
    }

    /// Safety net: clear a half-drawn bar line if `update` was never called with
    /// a `done` ping (e.g. the render errored out mid-loop). No-op otherwise.
    pub fn finish(&mut self) {
        if !self.enabled || !self.drawn {
            return;
        }
        let _ = write!(std::io::stderr(), "\r\u{1b}[K");
        let _ = std::io::stderr().flush();
        self.drawn = false;
    }
}
