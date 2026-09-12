//! The `validate` MCP tool: statically checks a wilios source (inline or by
//! path) without rendering or evaluating it, returning structured
//! diagnostics. All the actual analysis lives in `wilios_core::resolve` —
//! this module is transport plumbing only: request/response DTOs, sandbox
//! path handling, and the request-level limits from the tool's spec (§9).

use std::time::Duration;

use rmcp::model::{CallToolResult, ContentBlock};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use wilios_core::diagnostics::{Confidence, Diagnostic, Severity, Suggestion};
use wilios_core::parser::parser::resolve_import_path;
use wilios_core::resolve::{self, ToolError, ValidateOptions, ValidateOutcome};

const MAX_SOURCE_BYTES: usize = 1_000_000;
const DEFAULT_MAX_DIAGNOSTICS: u32 = 50;
const WALL_CLOCK_TIMEOUT: Duration = Duration::from_secs(5);

fn default_true() -> bool {
    true
}

fn default_max_diagnostics() -> u32 {
    DEFAULT_MAX_DIAGNOSTICS
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ValidateRequest {
    /// Inline wilios source to check. Mutually exclusive with `path`.
    #[serde(default)]
    source: Option<String>,
    /// Path to a `.wilios` file, relative to the sandbox root. Mutually
    /// exclusive with `source`.
    #[serde(default)]
    path: Option<String>,
    /// Whether to resolve and check `import "..."` targets too (default
    /// true). When false, each import statement is still checked for
    /// resolvability, but its target is never read, and names it would
    /// have defined are not considered in scope.
    #[serde(default = "default_true")]
    follow_imports: bool,
    /// Whether to include "did you mean" suggestions on unresolved
    /// identifiers (default true).
    #[serde(default = "default_true")]
    suggestions: bool,
    /// Whether to include warning-severity lint diagnostics (default true).
    #[serde(default = "default_true")]
    lints: bool,
    /// Maximum diagnostics to return (default 50, clamped to [1, 500]).
    #[serde(default = "default_max_diagnostics")]
    max_diagnostics: u32,
    /// Whether to include a source excerpt with a caret underline on each
    /// diagnostic (default true).
    #[serde(default = "default_true")]
    excerpt: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
struct PositionDto {
    line: usize,
    column: usize,
    offset: usize,
}

#[derive(Debug, Serialize, JsonSchema)]
struct SpanDto {
    file: String,
    start: PositionDto,
    end: PositionDto,
}

#[derive(Debug, Serialize, JsonSchema)]
struct SuggestionDto {
    replacement: String,
    confidence: &'static str,
    reason: String,
}

impl From<&Suggestion> for SuggestionDto {
    fn from(s: &Suggestion) -> Self {
        SuggestionDto {
            replacement: s.replacement.clone(),
            confidence: match s.confidence {
                Confidence::High => "high",
                Confidence::Medium => "medium",
                Confidence::Low => "low",
            },
            reason: s.reason.clone(),
        }
    }
}

#[derive(Debug, Serialize, JsonSchema)]
struct DiagnosticDto {
    severity: &'static str,
    code: &'static str,
    message: String,
    span: SpanDto,
    excerpt: Option<String>,
    suggestions: Vec<SuggestionDto>,
}

impl From<&Diagnostic> for DiagnosticDto {
    fn from(d: &Diagnostic) -> Self {
        DiagnosticDto {
            severity: match d.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
                Severity::Info => "info",
            },
            code: d.code,
            message: d.message.clone(),
            span: SpanDto {
                file: d.span.file.clone(),
                start: PositionDto {
                    line: d.span.start.line,
                    column: d.span.start.col,
                    offset: d.span.start.offset,
                },
                end: PositionDto {
                    line: d.span.end.line,
                    column: d.span.end.col,
                    offset: d.span.end.offset,
                },
            },
            excerpt: d.excerpt.clone(),
            suggestions: d.suggestions.iter().map(SuggestionDto::from).collect(),
        }
    }
}

#[derive(Debug, Serialize, JsonSchema)]
struct SummaryDto {
    errors: usize,
    warnings: usize,
    truncated: bool,
    recovered: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
struct ValidateResponse {
    ok: bool,
    summary: SummaryDto,
    diagnostics: Vec<DiagnosticDto>,
    files_validated: Vec<String>,
}

impl From<ValidateOutcome> for ValidateResponse {
    fn from(o: ValidateOutcome) -> Self {
        ValidateResponse {
            ok: o.ok,
            summary: SummaryDto {
                errors: o.summary.errors,
                warnings: o.summary.warnings,
                truncated: o.summary.truncated,
                recovered: o.summary.recovered,
            },
            diagnostics: o.diagnostics.iter().map(DiagnosticDto::from).collect(),
            files_validated: o.files_validated,
        }
    }
}

fn tool_error(message: impl Into<String>) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(message.into())])
}

/// Display name for a canonicalized on-disk path, relative to
/// `project_root` when possible — mirrors
/// `wilios_core::resolve::imports`'s own (private) helper, since the two
/// are independent transport-vs-analysis concerns rather than logic that
/// needs to be shared.
fn display_name_for(canonical: &std::path::Path, project_root: &std::path::Path) -> String {
    canonical
        .strip_prefix(project_root)
        .unwrap_or(canonical)
        .to_string_lossy()
        .replace('\\', "/")
}

pub async fn handle(req: ValidateRequest) -> Result<CallToolResult, rmcp::ErrorData> {
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

    let (text, entry_display, entry_canonical, base_dir) = if let Some(source) = source {
        (
            source,
            "<source>".to_string(),
            None,
            Some(project_root.clone()),
        )
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
        let display = display_name_for(&canonical, &project_root);
        let base_dir = canonical.parent().map(|p| p.to_path_buf());
        (text, display, Some(canonical), base_dir)
    };

    if text.len() > MAX_SOURCE_BYTES {
        return Ok(tool_error(format!(
            "source exceeds the {MAX_SOURCE_BYTES}-byte limit ({} bytes)",
            text.len()
        )));
    }

    let opts = ValidateOptions {
        follow_imports: req.follow_imports,
        suggestions: req.suggestions,
        lints: req.lints,
        max_diagnostics: req.max_diagnostics.clamp(1, 500) as usize,
        excerpt: req.excerpt,
        project_root,
    };

    let task = tokio::task::spawn_blocking(move || {
        resolve::validate_source(&text, &entry_display, entry_canonical, base_dir, &opts)
    });

    let outcome = match tokio::time::timeout(WALL_CLOCK_TIMEOUT, task).await {
        Ok(Ok(Ok(outcome))) => outcome,
        Ok(Ok(Err(ToolError::TooManyFiles))) => {
            return Ok(tool_error("import graph touches more than 128 files"));
        }
        Ok(Err(join_err)) => {
            return Ok(tool_error(format!("validate task failed: {join_err}")));
        }
        Err(_) => {
            return Ok(tool_error("validate timed out after 5s (timeout)"));
        }
    };

    // Invalid source is a successful tool call — `isError` is reserved for
    // tool-level failure (bad args, sandbox escape, timeout, size/file
    // limits), all handled above. `ok:false` in the payload carries an
    // invalid-source result, unlike `describe_symbol`'s existing
    // "unknown name -> CallToolResult::error" precedent, which does not
    // apply here (see the tool's spec §4).
    let response = ValidateResponse::from(outcome);
    Ok(CallToolResult::success(vec![ContentBlock::json(response)?]))
}
