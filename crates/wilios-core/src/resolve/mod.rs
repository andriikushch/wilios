//! The static-analysis pass backing `wilios-mcp`'s `validate` tool (and
//! reusable by anything else that wants to check a `.wilios` source without
//! rendering or evaluating it — `describe_symbol`, a future cookbook CI
//! check, editor tooling). See `scope`'s module docs for the name
//! resolution model, `imports`'s for the import-graph walk, and `checks`'s
//! for the static semantic checks and lints.
//!
//! Orchestration (`validate_source`) mirrors the four-stage pipeline from
//! the tool's spec: lex → parse → (import graph +) name resolution →
//! static checks/lints, each stage only meaningful if the previous one
//! didn't produce an error that makes it moot. Concretely:
//!
//! 1. Lex + parse the entry file. Either failing is a single diagnostic and
//!    stops everything else (no recovery in v1 — see `imports::build`).
//! 2. Walk the import graph (only if the entry parsed). A lex/parse failure
//!    in an *imported* file is that file's own diagnostic; it doesn't stop
//!    the rest of the graph or the entry file's own analysis.
//! 3. Name resolution over every successfully-parsed file, using a scope
//!    that matches the interpreter's real flattening of imported
//!    `global_stmts` into one shared env when `follow_imports` is on.
//! 4. Static checks (builtin arity, non-positive duration literals) and,
//!    if requested, lints (track-produces-no-events) — gated per-subtree,
//!    not globally: an arity check simply has nothing to check when its
//!    callee didn't resolve, but everything else still runs.

pub mod checks;
pub mod imports;
pub mod scope;

use std::collections::HashSet;
use std::path::PathBuf;

use crate::diagnostics::{Diagnostic, Severity};
use crate::parser::ast::Ident;

use imports::ImportGraphError;
use scope::WalkCtx;

pub struct ValidateOptions {
    pub follow_imports: bool,
    pub suggestions: bool,
    pub lints: bool,
    /// Clamped to `[1, 500]` by the caller (see `wilios-mcp`'s `validate`
    /// tool) before reaching here.
    pub max_diagnostics: usize,
    pub excerpt: bool,
    /// The sandbox root every import must stay within — see
    /// `crate::parser::parser::resolve_import_path`.
    pub project_root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summary {
    pub errors: usize,
    pub warnings: usize,
    pub truncated: bool,
    /// Always `true` unless a `WIL-E1000`/`WIL-E2000` diagnostic is present
    /// — v1 has no error-recovery machinery for lex/parse errors (see
    /// module docs), so whenever one of those fires, there may be more
    /// that were never reached.
    pub recovered: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ValidateOutcome {
    pub ok: bool,
    pub summary: Summary,
    pub diagnostics: Vec<Diagnostic>,
    /// Display names, entry first, in first-touched order — a diamond
    /// import appears once.
    pub files_validated: Vec<String>,
}

#[derive(Debug)]
pub enum ToolError {
    /// More than 128 distinct files were reached (spec §9) — a tool-level
    /// failure, not a diagnostic, since it means the request itself is out
    /// of bounds rather than the source being invalid.
    TooManyFiles,
}

/// Validates `entry_text` (already read from disk or supplied inline).
///
/// `entry_canonical` must already be canonicalized, and `Some` only when
/// the entry has an on-disk identity (a `path`-based request) — `None` for
/// inline `source`, which can't be named by any `import` statement and so
/// can't participate in an import cycle.
pub fn validate_source(
    entry_text: &str,
    entry_display: &str,
    entry_canonical: Option<PathBuf>,
    entry_base_dir: Option<PathBuf>,
    opts: &ValidateOptions,
) -> Result<ValidateOutcome, ToolError> {
    let graph = imports::build(
        entry_text,
        entry_display,
        entry_canonical,
        entry_base_dir,
        opts.follow_imports,
        &opts.project_root,
    )
    .map_err(|e| match e {
        ImportGraphError::TooManyFiles => ToolError::TooManyFiles,
    })?;

    let mut diagnostics = graph.diagnostics;

    if graph.entry_ok {
        // The runtime flattens every transitively-imported file's top-level
        // bindings into one shared env before any track executes (see
        // `scope`'s module docs) — so the "global" scope every file's
        // top-level code and every track sees is the union across the
        // whole graph, not just the entry file's own.
        let mut global_names: HashSet<Ident> = HashSet::new();
        for unit in &graph.units {
            if let Some(program) = &unit.program {
                global_names.extend(scope::collect_bound_names(&program.global_stmts));
            }
        }

        for unit in &graph.units {
            let Some(program) = &unit.program else {
                continue;
            };
            let ctx = WalkCtx {
                file: &unit.display_name,
                source: &unit.source,
                suggestions: opts.suggestions,
                excerpt: opts.excerpt,
            };

            scope::walk_stmts(&program.global_stmts, &global_names, &ctx, &mut diagnostics);
            checks::duration_diagnostics(&program.global_stmts, &ctx, &mut diagnostics);

            for track in &program.tracks {
                let mut track_scope = global_names.clone();
                track_scope.extend(scope::collect_bound_names(&track.statements));

                scope::walk_stmts(&track.statements, &track_scope, &ctx, &mut diagnostics);
                checks::duration_diagnostics(&track.statements, &ctx, &mut diagnostics);

                if opts.lints
                    && let Some(d) = checks::track_no_events_lint(track, &ctx)
                {
                    diagnostics.push(d);
                }
            }
        }
    }

    diagnostics.sort_by(|a, b| {
        a.span
            .file
            .cmp(&b.span.file)
            .then(a.span.start.line.cmp(&b.span.start.line))
            .then(a.span.start.col.cmp(&b.span.start.col))
            .then(a.code.cmp(b.code))
            .then(a.message.cmp(&b.message))
    });

    let recovered = !diagnostics
        .iter()
        .any(|d| d.code == "WIL-E1000" || d.code == "WIL-E2000");

    let truncated = diagnostics.len() > opts.max_diagnostics;
    if truncated {
        diagnostics.truncate(opts.max_diagnostics);
    }

    let errors = diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .count();
    let warnings = diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Warning)
        .count();

    let files_validated = graph.units.iter().map(|u| u.display_name.clone()).collect();

    Ok(ValidateOutcome {
        ok: errors == 0,
        summary: Summary {
            errors,
            warnings,
            truncated,
            recovered,
        },
        diagnostics,
        files_validated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(project_root: PathBuf) -> ValidateOptions {
        ValidateOptions {
            follow_imports: true,
            suggestions: true,
            lints: true,
            max_diagnostics: 50,
            excerpt: true,
            project_root,
        }
    }

    #[test]
    fn valid_source_is_clean() {
        let cwd = std::env::current_dir().unwrap();
        let outcome =
            validate_source("track 0\n<C4> 1/4\n", "<source>", None, None, &opts(cwd)).unwrap();
        assert!(outcome.ok);
        assert!(outcome.diagnostics.is_empty());
        assert_eq!(outcome.files_validated, vec!["<source>".to_string()]);
        assert!(outcome.summary.recovered);
    }

    #[test]
    fn lex_error_produces_single_diagnostic_and_stops() {
        let cwd = std::env::current_dir().unwrap();
        let outcome =
            validate_source("let x = \"oops\n", "<source>", None, None, &opts(cwd)).unwrap();
        assert!(!outcome.ok);
        assert_eq!(outcome.diagnostics.len(), 1);
        assert_eq!(outcome.diagnostics[0].code, "WIL-E1000");
        assert!(!outcome.summary.recovered);
    }

    #[test]
    fn unknown_identifier_end_to_end() {
        let cwd = std::env::current_dir().unwrap();
        let src = "track 1\nlet x = arpegio(c4, 4)\n<C4> 1/4\n";
        let outcome = validate_source(src, "<source>", None, None, &opts(cwd)).unwrap();
        assert!(!outcome.ok);
        assert!(outcome.diagnostics.iter().any(|d| d.code == "WIL-E3001"));
        let d = outcome
            .diagnostics
            .iter()
            .find(|d| d.code == "WIL-E3001")
            .unwrap();
        assert!(d.message.contains("arpegio"));
        assert!(d.excerpt.is_some());
    }

    #[test]
    fn bad_builtin_arity_end_to_end() {
        let cwd = std::env::current_dir().unwrap();
        let src = "let n = len(1, 2)\n";
        let outcome = validate_source(src, "<source>", None, None, &opts(cwd)).unwrap();
        assert!(!outcome.ok);
        assert!(outcome.diagnostics.iter().any(|d| d.code == "WIL-E4001"));
    }

    #[test]
    fn computed_arguments_validate_clean() {
        let cwd = std::env::current_dir().unwrap();
        let src = "let a = [1, 2, 3]\nlet n = len(a)\ntrack 0\nrest n/4\n";
        let outcome = validate_source(src, "<source>", None, None, &opts(cwd)).unwrap();
        assert!(outcome.ok, "expected clean, got {:?}", outcome.diagnostics);
    }

    #[test]
    fn max_diagnostics_truncates() {
        let cwd = std::env::current_dir().unwrap();
        let src = "print(unknown_a)\nprint(unknown_b)\nprint(unknown_c)\n";
        let mut o = opts(cwd);
        o.max_diagnostics = 1;
        let outcome = validate_source(src, "<source>", None, None, &o).unwrap();
        assert_eq!(outcome.diagnostics.len(), 1);
        assert!(outcome.summary.truncated);
    }

    #[test]
    fn lints_flag_disables_lint_diagnostics() {
        let cwd = std::env::current_dir().unwrap();
        let src = "track 0\ntempo 120\n";
        let mut o = opts(cwd.clone());
        o.lints = false;
        let outcome = validate_source(src, "<source>", None, None, &o).unwrap();
        assert!(outcome.diagnostics.iter().all(|d| d.code != "WIL-W6001"));

        let outcome_with_lints = validate_source(src, "<source>", None, None, &opts(cwd)).unwrap();
        assert!(
            outcome_with_lints
                .diagnostics
                .iter()
                .any(|d| d.code == "WIL-W6001")
        );
    }

    #[test]
    fn deterministic_across_repeated_calls() {
        let cwd = std::env::current_dir().unwrap();
        let src = "track 1\nlet x = arpegio(c4, 4)\n<C4> 1/4\n";
        let a = validate_source(src, "<source>", None, None, &opts(cwd.clone())).unwrap();
        let b = validate_source(src, "<source>", None, None, &opts(cwd)).unwrap();
        assert_eq!(a, b);
    }
}
