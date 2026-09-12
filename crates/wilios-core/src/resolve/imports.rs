//! Import-graph walking for `validate`. Deliberately does **not** reuse the
//! interpreter's recursive/merging `Parser::parse_import` path: that path
//! flattens every imported file's statements into one `Program` with no
//! provenance (a merged `Stmt` is indistinguishable from a native one), and
//! its `loaded: HashSet<PathBuf>` is cloned down the recursion — it guards
//! against ancestor cycles only, not a global "every file visited"
//! registry — neither of which is enough to attribute a diagnostic to the
//! file it came from or to populate `files_validated`.
//!
//! Instead, each file is lexed and parsed *independently* via
//! `Parser::new_shallow` (which records `import "..."` as an inert
//! `Stmt::Import` rather than resolving/recursing into it — see
//! `crate::parser::parser`), and this module drives its own explicit,
//! depth- and cycle-aware walk over those `Stmt::Import` markers. The one
//! piece of security-sensitive logic (extension/relative-path/canonicalize/
//! sandbox-containment) is never duplicated: both the normal recursive
//! parser and this walker call the same `resolve_import_path`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::diagnostics::{self, Diagnostic, Severity, Span};
use crate::lexer::Lexer;
use crate::lexer::lex::LexError;
use crate::parser::ast::Stmt;
use crate::parser::parser::{ParseError, Parser, Program, resolve_import_path};

/// Depth is measured in edges from the entry file (spec §9).
const MAX_IMPORT_DEPTH: usize = 32;
/// Distinct files touched across the whole call, entry included (spec §9).
const MAX_FILES: usize = 128;

/// One file's lex/parse result plus enough context to check it and to
/// resolve any imports it names.
pub struct FileUnit {
    pub display_name: String,
    pub source: String,
    /// `None` if this file itself failed to lex/parse — its own failure is
    /// recorded in `ImportGraph::diagnostics`, and (unlike the entry file)
    /// this simply contributes no bindings and is not walked further.
    pub program: Option<Program>,
    /// The directory this file's own *relative* `import "..."` paths
    /// resolve against — the file's own parent directory for an on-disk
    /// file, or whatever the caller passed for the entry (e.g. the sandbox
    /// root, for inline `source` with no file of its own). Stored per-unit
    /// rather than re-derived from the ancestor stack, since the entry
    /// file may have a `base_dir` while having no canonical path at all
    /// (inline `source`).
    pub base_dir: Option<PathBuf>,
}

pub struct ImportGraph {
    /// Insertion order: entry file first, then every successfully-reached
    /// import, deduplicated by canonical path (a diamond import appears
    /// once). This order becomes `files_validated`.
    pub units: Vec<FileUnit>,
    pub diagnostics: Vec<Diagnostic>,
    /// False only when the *entry* file itself failed to lex/parse — in
    /// that case `units` has exactly one (failed) entry and the walk never
    /// started, matching the "first error stops everything" rule for the
    /// file actually being validated (see `resolve::validate_source`).
    pub entry_ok: bool,
}

#[derive(Debug)]
pub enum ImportGraphError {
    TooManyFiles,
}

enum FrontendError {
    Lex(LexError),
    Parse(ParseError),
}

fn lex_and_parse(source: &str, base_dir: Option<PathBuf>) -> Result<Program, FrontendError> {
    let tokens = Lexer::new(source).lex().map_err(FrontendError::Lex)?;
    Parser::new_shallow(tokens, base_dir)
        .parse()
        .map_err(FrontendError::Parse)
}

fn frontend_error_diagnostic(err: FrontendError, file: &str, source: &str) -> Diagnostic {
    let (code, line, col, message) = match err {
        FrontendError::Lex(e) => ("WIL-E1000", e.line, e.col, e.message),
        FrontendError::Parse(e) => ("WIL-E2000", e.line, e.col, e.message),
    };
    let start = diagnostics::position_at(source, line, col);
    Diagnostic {
        severity: Severity::Error,
        code,
        message,
        span: Span {
            file: file.to_string(),
            start,
            end: start,
        },
        excerpt: None,
        suggestions: Vec::new(),
    }
}

fn display_name_for(canonical: &Path, project_root: &Path) -> String {
    canonical
        .strip_prefix(project_root)
        .unwrap_or(canonical)
        .to_string_lossy()
        .replace('\\', "/")
}

/// The `import "path"` statement a diagnostic is about, bundled so the
/// builder functions below don't each need 5+ separate parameters.
struct ImportSite<'a> {
    path_str: &'a str,
    line: usize,
    col: usize,
    file: &'a str,
    source: &'a str,
}

impl ImportSite<'_> {
    fn span(&self) -> Span {
        let start = diagnostics::position_at(self.source, self.line, self.col);
        // The token this span covers is the `"path.wilios"` string literal
        // (quotes included), matching what a reader sees at that position.
        let end = diagnostics::end_position(start, &format!("\"{}\"", self.path_str));
        Span {
            file: self.file.to_string(),
            start,
            end,
        }
    }
}

fn unresolved_import_diagnostic(site: &ImportSite, detail: &dyn std::fmt::Display) -> Diagnostic {
    Diagnostic {
        severity: Severity::Error,
        code: "WIL-E3002",
        message: format!("Cannot resolve import '{}': {detail}", site.path_str),
        span: site.span(),
        excerpt: None,
        suggestions: Vec::new(),
    }
}

fn depth_exceeded_diagnostic(site: &ImportSite) -> Diagnostic {
    Diagnostic {
        severity: Severity::Error,
        code: "WIL-E5001",
        message: format!("Import depth exceeds the limit of {MAX_IMPORT_DEPTH}."),
        span: site.span(),
        excerpt: None,
        suggestions: Vec::new(),
    }
}

fn cycle_diagnostic(
    site: &ImportSite,
    stack: &[PathBuf],
    cycle_start: usize,
    project_root: &Path,
) -> Diagnostic {
    let mut names: Vec<String> = stack[cycle_start..]
        .iter()
        .map(|p| display_name_for(p, project_root))
        .collect();
    names.push(display_name_for(&stack[cycle_start], project_root));
    Diagnostic {
        severity: Severity::Error,
        code: "WIL-E5001",
        message: format!("Import cycle detected: {}", names.join(" -> ")),
        span: site.span(),
        excerpt: None,
        suggestions: Vec::new(),
    }
}

fn collect_import_stmts(program: &Program) -> Vec<(String, usize, usize)> {
    let mut out = Vec::new();
    for stmt in &program.global_stmts {
        if let Stmt::Import { path, line, col } = stmt {
            out.push((path.clone(), *line, *col));
        }
    }
    for track in &program.tracks {
        for stmt in &track.statements {
            if let Stmt::Import { path, line, col } = stmt {
                out.push((path.clone(), *line, *col));
            }
        }
    }
    out
}

/// Builds the import graph starting from an already-lexed-and-parsed-or-not
/// entry file. `entry_canonical` is `Some` (and must already be
/// canonicalized) when the entry came from an on-disk `path`, `None` for
/// inline `source` — an inline entry has no path of its own, so nothing can
/// name it in an `import` statement and it can't participate in a cycle.
#[allow(clippy::too_many_arguments)]
pub fn build(
    entry_text: &str,
    entry_display: &str,
    entry_canonical: Option<PathBuf>,
    entry_base_dir: Option<PathBuf>,
    follow_imports: bool,
    project_root: &Path,
) -> Result<ImportGraph, ImportGraphError> {
    let mut graph = ImportGraph {
        units: Vec::new(),
        diagnostics: Vec::new(),
        entry_ok: true,
    };

    let entry_program = match lex_and_parse(entry_text, entry_base_dir.clone()) {
        Ok(p) => Some(p),
        Err(e) => {
            graph
                .diagnostics
                .push(frontend_error_diagnostic(e, entry_display, entry_text));
            graph.entry_ok = false;
            None
        }
    };

    graph.units.push(FileUnit {
        display_name: entry_display.to_string(),
        source: entry_text.to_string(),
        program: entry_program,
        base_dir: entry_base_dir.clone(),
    });

    if !graph.entry_ok {
        return Ok(graph);
    }

    if !follow_imports {
        // Still validate each import statement's own resolvability (spec
        // §3/§8), but never read/recurse into the target, and its bindings
        // never enter scope.
        let imports = collect_import_stmts(graph.units[0].program.as_ref().unwrap());
        for (path_str, line, col) in imports {
            if let Err(e) = resolve_import_path(&path_str, entry_base_dir.as_deref(), project_root)
            {
                let site = ImportSite {
                    path_str: &path_str,
                    line,
                    col,
                    file: entry_display,
                    source: entry_text,
                };
                graph
                    .diagnostics
                    .push(unresolved_import_diagnostic(&site, &e));
            }
        }
        return Ok(graph);
    }

    let mut visited: HashMap<PathBuf, usize> = HashMap::new();
    let mut stack: Vec<PathBuf> = Vec::new();
    if let Some(p) = entry_canonical {
        visited.insert(p.clone(), 0);
        stack.push(p);
    }

    visit(0, &mut graph, &mut visited, &mut stack, project_root)?;

    Ok(graph)
}

fn visit(
    file_index: usize,
    graph: &mut ImportGraph,
    visited: &mut HashMap<PathBuf, usize>,
    stack: &mut Vec<PathBuf>,
    project_root: &Path,
) -> Result<(), ImportGraphError> {
    let (display_name, source, base_dir, imports) = {
        let unit = &graph.units[file_index];
        let imports = unit
            .program
            .as_ref()
            .map(collect_import_stmts)
            .unwrap_or_default();
        (
            unit.display_name.clone(),
            unit.source.clone(),
            unit.base_dir.clone(),
            imports,
        )
    };

    for (path_str, line, col) in imports {
        let site = ImportSite {
            path_str: &path_str,
            line,
            col,
            file: &display_name,
            source: &source,
        };
        match resolve_import_path(&path_str, base_dir.as_deref(), project_root) {
            Err(e) => {
                graph
                    .diagnostics
                    .push(unresolved_import_diagnostic(&site, &e));
            }
            Ok(canonical) => {
                if let Some(pos) = stack.iter().position(|p| p == &canonical) {
                    graph
                        .diagnostics
                        .push(cycle_diagnostic(&site, stack, pos, project_root));
                    continue;
                }
                if visited.contains_key(&canonical) {
                    continue; // diamond import — already fully processed
                }
                if stack.len() >= MAX_IMPORT_DEPTH {
                    graph.diagnostics.push(depth_exceeded_diagnostic(&site));
                    continue;
                }

                let child_source = match std::fs::read_to_string(&canonical) {
                    Ok(s) => s,
                    Err(e) => {
                        graph
                            .diagnostics
                            .push(unresolved_import_diagnostic(&site, &e));
                        continue;
                    }
                };
                let child_base_dir = canonical.parent().map(|p| p.to_path_buf());
                let child_display = display_name_for(&canonical, project_root);

                let child_program = match lex_and_parse(&child_source, child_base_dir.clone()) {
                    Ok(p) => Some(p),
                    Err(e) => {
                        graph.diagnostics.push(frontend_error_diagnostic(
                            e,
                            &child_display,
                            &child_source,
                        ));
                        None
                    }
                };
                let has_program = child_program.is_some();

                graph.units.push(FileUnit {
                    display_name: child_display,
                    source: child_source,
                    program: child_program,
                    base_dir: child_base_dir,
                });
                if graph.units.len() > MAX_FILES {
                    return Err(ImportGraphError::TooManyFiles);
                }
                let child_index = graph.units.len() - 1;
                visited.insert(canonical.clone(), child_index);

                if has_program {
                    stack.push(canonical);
                    visit(child_index, graph, visited, stack, project_root)?;
                    stack.pop();
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "wilios_resolve_imports_test_{name}_{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self { path }
        }

        fn write(&self, rel: &str, contents: &str) -> PathBuf {
            let p = self.path.join(rel);
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            let mut f = std::fs::File::create(&p).unwrap();
            f.write_all(contents.as_bytes()).unwrap();
            p
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn single_file_no_imports() {
        let dir = TempDir::new("single");
        let graph = build(
            "track 0\n<C4> 1/4\n",
            "<source>",
            None,
            None,
            true,
            &dir.path,
        )
        .unwrap();
        assert!(graph.entry_ok);
        assert_eq!(graph.units.len(), 1);
        assert!(graph.diagnostics.is_empty());
    }

    #[test]
    fn entry_lex_error_stops_everything() {
        let dir = TempDir::new("lexerr");
        let graph = build(
            "let x = \"unterminated\n",
            "<source>",
            None,
            None,
            true,
            &dir.path,
        )
        .unwrap();
        assert!(!graph.entry_ok);
        assert_eq!(graph.units.len(), 1);
        assert_eq!(graph.diagnostics.len(), 1);
        assert_eq!(graph.diagnostics[0].code, "WIL-E1000");
    }

    #[test]
    fn follows_a_real_import() {
        let dir = TempDir::new("follow");
        dir.write("lib.wilios", "let helper = 1\n");
        let entry_path = dir.write("entry.wilios", "import \"lib.wilios\"\ntrack 0\n<C4> 1/4\n");
        let canonical = entry_path.canonicalize().unwrap();
        let project_root = dir.path.canonicalize().unwrap();
        let text = std::fs::read_to_string(&canonical).unwrap();
        let base_dir = canonical.parent().map(|p| p.to_path_buf());
        let graph = build(
            &text,
            "entry.wilios",
            Some(canonical),
            base_dir,
            true,
            &project_root,
        )
        .unwrap();
        assert!(graph.entry_ok);
        assert_eq!(graph.units.len(), 2);
        assert!(graph.diagnostics.is_empty());
        assert_eq!(graph.units[1].display_name, "lib.wilios");
    }

    #[test]
    fn unresolvable_import_is_a_diagnostic_not_a_panic() {
        let dir = TempDir::new("missing");
        let entry_path = dir.write("entry.wilios", "import \"does_not_exist.wilios\"\n");
        let canonical = entry_path.canonicalize().unwrap();
        let project_root = dir.path.canonicalize().unwrap();
        let text = std::fs::read_to_string(&canonical).unwrap();
        let base_dir = canonical.parent().map(|p| p.to_path_buf());
        let graph = build(
            &text,
            "entry.wilios",
            Some(canonical),
            base_dir,
            true,
            &project_root,
        )
        .unwrap();
        assert!(graph.entry_ok);
        assert_eq!(graph.units.len(), 1);
        assert_eq!(graph.diagnostics.len(), 1);
        assert_eq!(graph.diagnostics[0].code, "WIL-E3002");
    }

    #[test]
    fn import_cycle_is_detected() {
        let dir = TempDir::new("cycle");
        dir.write("a.wilios", "import \"b.wilios\"\n");
        dir.write("b.wilios", "import \"a.wilios\"\n");
        let entry_path = dir.path.join("a.wilios");
        let canonical = entry_path.canonicalize().unwrap();
        let project_root = dir.path.canonicalize().unwrap();
        let text = std::fs::read_to_string(&canonical).unwrap();
        let base_dir = canonical.parent().map(|p| p.to_path_buf());
        let graph = build(
            &text,
            "a.wilios",
            Some(canonical),
            base_dir,
            true,
            &project_root,
        )
        .unwrap();
        assert!(graph.entry_ok);
        assert!(graph.diagnostics.iter().any(|d| d.code == "WIL-E5001"));
    }

    #[test]
    fn diamond_import_is_deduped() {
        let dir = TempDir::new("diamond");
        dir.write("shared.wilios", "let x = 1\n");
        dir.write("a.wilios", "import \"shared.wilios\"\n");
        dir.write("b.wilios", "import \"shared.wilios\"\n");
        let entry_path = dir.write("entry.wilios", "import \"a.wilios\"\nimport \"b.wilios\"\n");
        let canonical = entry_path.canonicalize().unwrap();
        let project_root = dir.path.canonicalize().unwrap();
        let text = std::fs::read_to_string(&canonical).unwrap();
        let base_dir = canonical.parent().map(|p| p.to_path_buf());
        let graph = build(
            &text,
            "entry.wilios",
            Some(canonical),
            base_dir,
            true,
            &project_root,
        )
        .unwrap();
        assert!(graph.entry_ok);
        assert!(graph.diagnostics.is_empty());
        // entry + a + b + shared (once, not twice)
        assert_eq!(graph.units.len(), 4);
        let shared_count = graph
            .units
            .iter()
            .filter(|u| u.display_name == "shared.wilios")
            .count();
        assert_eq!(shared_count, 1);
    }

    #[test]
    fn follow_imports_false_checks_resolvability_but_does_not_recurse() {
        let dir = TempDir::new("nofollow");
        dir.write("lib.wilios", "let helper = 1\n");
        let entry_path = dir.write("entry.wilios", "import \"lib.wilios\"\n");
        let canonical = entry_path.canonicalize().unwrap();
        let project_root = dir.path.canonicalize().unwrap();
        let text = std::fs::read_to_string(&canonical).unwrap();
        let base_dir = canonical.parent().map(|p| p.to_path_buf());
        let graph = build(
            &text,
            "entry.wilios",
            Some(canonical),
            base_dir,
            false,
            &project_root,
        )
        .unwrap();
        assert!(graph.entry_ok);
        assert_eq!(
            graph.units.len(),
            1,
            "must not recurse when follow_imports is false"
        );
        assert!(graph.diagnostics.is_empty());
    }
}
