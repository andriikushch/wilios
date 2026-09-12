//! The `dump_events` MCP tool: runs a wilios source (inline or by path) and
//! returns its per-track note-event timeline. This is the only tool that
//! executes the interpreter — it is bounded by `max_ms` of composition time
//! and a 5 s wall-clock timeout, and never opens an audio device. The
//! event → JSON mapping lives in `wilios_core::dump`; this module is transport
//! plumbing only: request DTO, sandbox path handling, and the request limits.

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;

use rmcp::model::{CallToolResult, ContentBlock};
use schemars::JsonSchema;
use serde::Deserialize;

use wilios_core::dump::{EventDump, build_dump, render_piano_roll};
use wilios_core::interpreter::interpreter::Interpreter;
use wilios_core::lexer::Lexer;
use wilios_core::parser::parser::{Parser, resolve_import_path};

const MAX_SOURCE_BYTES: usize = 1_000_000;
const WALL_CLOCK_TIMEOUT: Duration = Duration::from_secs(5);
const DEFAULT_MAX_MS: u64 = 60_000;
const MIN_MAX_MS: u64 = 1_000;
const MAX_MAX_MS: u64 = 600_000;

fn default_max_ms() -> u64 {
    DEFAULT_MAX_MS
}

/// How to render the scheduled timeline back to the caller.
#[derive(Debug, Default, PartialEq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DumpFormat {
    /// The full structured `EventDump` as JSON (default).
    #[default]
    Json,
    /// An ASCII piano roll — a 16th-note grid per track, for eyeballing rhythm
    /// and voice-leading. Compact, but lossy (no ADSR / FM / exact timings).
    Roll,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DumpRequest {
    /// Inline wilios source to run. Mutually exclusive with `path`.
    #[serde(default)]
    source: Option<String>,
    /// Path to a `.wilios` file, relative to the sandbox root. Mutually
    /// exclusive with `source`.
    #[serde(default)]
    path: Option<String>,
    /// Composition-time budget in milliseconds (default 60000, clamped to
    /// [1000, 600000]). A piece that has not finished within this many ms of
    /// its own timeline comes back with `finished: false` and events truncated
    /// at the bound — a successful call, not an error.
    #[serde(default = "default_max_ms")]
    max_ms: u64,
    /// Output format: `json` (default, the full structured timeline) or `roll`
    /// (a compact ASCII piano roll as text).
    #[serde(default)]
    format: DumpFormat,
}

fn tool_error(message: impl Into<String>) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(message.into())])
}

pub async fn handle(req: DumpRequest) -> Result<CallToolResult, rmcp::ErrorData> {
    let format = req.format;
    let (source, path) = (req.source, req.path);
    match (&source, &path) {
        (Some(_), Some(_)) => {
            return Ok(tool_error(
                "exactly one of `source` or `path` must be given, not both",
            ));
        }
        (None, None) => {
            return Ok(tool_error(
                "exactly one of `source` or `path` must be given",
            ));
        }
        _ => {}
    }

    let project_root = match std::env::current_dir().and_then(|d| d.canonicalize()) {
        Ok(p) => p,
        Err(e) => return Ok(tool_error(format!("cannot resolve sandbox root: {e}"))),
    };

    let (text, base_dir, entry_canonical) = if let Some(source) = source {
        (source, project_root.clone(), None)
    } else {
        let path = path.expect("path is Some — checked above");
        let canonical = match resolve_import_path(&path, Some(&project_root), &project_root) {
            Ok(p) => p,
            Err(e) => return Ok(tool_error(format!("cannot resolve path '{path}': {e}"))),
        };
        let text = match std::fs::read_to_string(&canonical) {
            Ok(t) => t,
            Err(e) => return Ok(tool_error(format!("cannot read '{path}': {e}"))),
        };
        let base_dir = canonical
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| project_root.clone());
        (text, base_dir, Some(canonical))
    };

    if text.len() > MAX_SOURCE_BYTES {
        return Ok(tool_error(format!(
            "source exceeds the {MAX_SOURCE_BYTES}-byte limit ({} bytes)",
            text.len()
        )));
    }

    let max_ms = req.max_ms.clamp(MIN_MAX_MS, MAX_MAX_MS);

    let task =
        tokio::task::spawn_blocking(move || run_dump(&text, base_dir, entry_canonical, max_ms));

    let outcome = match tokio::time::timeout(WALL_CLOCK_TIMEOUT, task).await {
        Ok(Ok(res)) => res,
        Ok(Err(join_err)) => return Ok(tool_error(format!("dump task failed: {join_err}"))),
        Err(_) => return Ok(tool_error("dump timed out after 5s (timeout)")),
    };

    match outcome {
        Ok(dump) => {
            let block = match format {
                DumpFormat::Json => ContentBlock::json(dump)?,
                DumpFormat::Roll => ContentBlock::text(render_piano_roll(&dump)),
            };
            Ok(CallToolResult::success(vec![block]))
        }
        Err(msg) => Ok(tool_error(msg)),
    }
}

/// Lex → parse → interpret → schedule, all synchronous. Compile failures are
/// tool-level errors (there is no diagnostic channel here — `validate` is the
/// tool for that); a non-terminating piece is a normal result with
/// `finished: false`.
fn run_dump(
    text: &str,
    base_dir: PathBuf,
    entry_canonical: Option<PathBuf>,
    max_ms: u64,
) -> Result<EventDump, String> {
    let tokens = Lexer::new(text)
        .lex()
        .map_err(|e| format!("source did not compile: {e} — use `validate` for diagnostics"))?;

    let loaded: HashSet<PathBuf> = entry_canonical.into_iter().collect();
    let program = Parser::new_with_context(tokens, Some(base_dir), loaded)
        .parse()
        .map_err(|e| format!("source did not compile: {e} — use `validate` for diagnostics"))?;

    let mut interp = Interpreter::new(program).map_err(|e| {
        format!(
            "source did not compile: {} — use `validate` for diagnostics",
            e.0
        )
    })?;

    let (events, finished) = interp
        .schedule_to_end(max_ms)
        .map_err(|e| format!("runtime error while scheduling: {}", e.0))?;

    Ok(build_dump(&events, finished))
}
