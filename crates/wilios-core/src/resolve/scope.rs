//! Static name resolution — the pass the interpreter never needed, since it
//! resolves every identifier dynamically at eval time against a live
//! `HashMap<Ident, Value>` (see `crate::interpreter::interpreter`). This
//! module reimplements just the *shape* of that scoping, statically:
//!
//! - **Flat, non-block scoping.** `let`/`assign` both bind into whatever
//!   scope is "current"; `loop`/`if` bodies are NOT separate scopes (see
//!   `Frame::Block`/`Frame::Loop`, which carry no env of their own) — a name
//!   bound anywhere inside a loop or if-branch is visible throughout the
//!   enclosing track/file. We deliberately ignore *order* (whether a use
//!   comes before or after its binding): the tool's overriding design goal
//!   is never a false positive, and modeling control-flow-sensitive order
//!   would require exactly the evaluation this tool is not allowed to do.
//!
//! - **A function body inherits the enclosing scope plus its parameters.**
//!   At a call the interpreter clones the caller's live `env_vars` and layers
//!   the parameters on top (`Stmt::Call`/`Expr::Call` in `interpreter.rs`), so
//!   a body can reference other top-level `func`s, globals, and the builtins,
//!   and can recurse. This resolver mirrors that: `Expr::Func` walks its body
//!   against `enclosing_scope ∪ params` (see `walk_expr`). Names bound *inside*
//!   a body (`let`, params) still do not leak *out* — `collect_bound_names`
//!   does not descend into `Expr::Func`. Being permissive here also fits the
//!   tool's "never a false positive" charter: a body defined in a track sees
//!   that track's bindings too, which the interpreter's dynamic scoping allows.
//!
//! Two distinct kinds of "known name" are tracked separately because they
//! get different treatment:
//! - `user_scope`: names a person actually wrote (`let`/`assign` targets,
//!   function parameters, and — when `follow_imports` is on — top-level
//!   bindings from every file in the import graph, matching the
//!   interpreter's own flattening of merged `global_stmts` into one env).
//!   A name found here is never arity-checked (see `resolve::checks`) even
//!   if it happens to collide with a stdlib name, since it may be the
//!   user's own (re)definition — the same reasoning that rules out a
//!   `duplicate-binding` check in this flat-scoping language.
//! - The 4 real builtins (`crate::interpreter::BUILTINS`): always resolved
//!   regardless of `user_scope` or imports, and the only names arity-checked
//!   (see `resolve::checks::builtin_arity_diagnostic`). FM presets are
//!   deliberately *not* treated as always-resolved here — unlike builtins
//!   they are ordinary `let name = func() {...}` bindings from
//!   `lib/lib.wilios` (confirmed in `wilios_core::stdlib`'s own doc
//!   comments) and only resolve if that file is actually imported, i.e.
//!   only via `user_scope`. `wilios_core::stdlib::all_symbols()` (which
//!   includes presets) is used only as a source of did-you-mean
//!   *candidates*, never as a resolution shortcut.

use std::collections::HashSet;

use crate::diagnostics::suggest::{self, Candidate};
use crate::diagnostics::{self, Diagnostic, Severity, Span};
use crate::interpreter::BUILTINS;
use crate::parser::ast::{Expr, Ident, Stmt};
use crate::stdlib;

use super::checks;

/// Collects every name bound by `let`/`assign` anywhere in `stmts`,
/// recursing into `Loop`/`If` bodies (which share the enclosing scope) but
/// NOT into `Expr::Func` bodies (an entirely separate scope — see module
/// docs). This is exactly the set of names that MAY be bound by the time
/// any given statement in `stmts` executes, ignoring order (see module
/// docs for why order is deliberately not modeled).
pub fn collect_bound_names(stmts: &[Stmt]) -> HashSet<Ident> {
    let mut names = HashSet::new();
    collect_into(stmts, &mut names);
    names
}

fn collect_into(stmts: &[Stmt], names: &mut HashSet<Ident>) {
    for stmt in stmts {
        match stmt {
            Stmt::Let { name, .. } | Stmt::Assign { name, .. } => {
                names.insert(name.clone());
            }
            Stmt::Loop { body, .. } => collect_into(body, names),
            Stmt::If {
                then_body,
                else_body,
                ..
            } => {
                collect_into(then_body, names);
                if let Some(eb) = else_body {
                    collect_into(eb, names);
                }
            }
            _ => {}
        }
    }
}

/// Per-file context threaded through the walk: which file diagnostics
/// should be attributed to, that file's source text (for span/excerpt
/// computation), and the request's `suggestions`/`excerpt` flags.
pub struct WalkCtx<'a> {
    pub file: &'a str,
    pub source: &'a str,
    pub suggestions: bool,
    pub excerpt: bool,
}

impl WalkCtx<'_> {
    fn span(&self, line: usize, col: usize, token_text: &str) -> Span {
        let start = diagnostics::position_at(self.source, line, col);
        let end = diagnostics::end_position(start, token_text);
        Span {
            file: self.file.to_string(),
            start,
            end,
        }
    }

    fn maybe_excerpt(&self, span: &Span) -> Option<String> {
        self.excerpt
            .then(|| diagnostics::render_excerpt(self.source, span))
    }
}

/// Walks `stmts` (a file's global statements, or one track's statements)
/// checking every `Expr::Var` reference against `user_scope` (see module
/// docs), pushing `WIL-E3001` (unknown identifier) and — via
/// `checks::builtin_arity_diagnostic` — `WIL-E4001` (bad builtin arity)
/// diagnostics into `out`.
pub fn walk_stmts(
    stmts: &[Stmt],
    user_scope: &HashSet<Ident>,
    ctx: &WalkCtx,
    out: &mut Vec<Diagnostic>,
) {
    for stmt in stmts {
        walk_stmt(stmt, user_scope, ctx, out);
    }
}

fn walk_stmt(stmt: &Stmt, scope: &HashSet<Ident>, ctx: &WalkCtx, out: &mut Vec<Diagnostic>) {
    match stmt {
        Stmt::Chord { pitches, duration } => {
            for p in pitches {
                walk_expr(p, scope, ctx, out);
            }
            walk_expr(&duration.beats, scope, ctx, out);
            walk_expr(&duration.division, scope, ctx, out);
        }
        Stmt::Rest { duration } => {
            walk_expr(&duration.beats, scope, ctx, out);
            walk_expr(&duration.division, scope, ctx, out);
        }
        Stmt::Attack(e)
        | Stmt::Decay(e)
        | Stmt::Sustain(e)
        | Stmt::Release(e)
        | Stmt::FmRatio(e)
        | Stmt::FmDepth(e)
        | Stmt::Swing(e)
        | Stmt::Cutoff(e)
        | Stmt::Resonance(e) => walk_expr(e, scope, ctx, out),
        Stmt::Vibrato { depth, rate } => {
            walk_expr(depth, scope, ctx, out);
            walk_expr(rate, scope, ctx, out);
        }
        Stmt::FmBlock { ops, .. } => {
            for op in ops {
                walk_expr(&op.ratio, scope, ctx, out);
                walk_expr(&op.level, scope, ctx, out);
                for e in [
                    &op.attack_ms,
                    &op.decay_ms,
                    &op.sustain_level,
                    &op.release_ms,
                ]
                .into_iter()
                .flatten()
                {
                    walk_expr(e, scope, ctx, out);
                }
            }
        }
        Stmt::Loop { condition, body } => {
            walk_expr(condition, scope, ctx, out);
            walk_stmts(body, scope, ctx, out);
        }
        Stmt::If {
            condition,
            then_body,
            else_body,
        } => {
            walk_expr(condition, scope, ctx, out);
            walk_stmts(then_body, scope, ctx, out);
            if let Some(eb) = else_body {
                walk_stmts(eb, scope, ctx, out);
            }
        }
        Stmt::Let { value, .. } | Stmt::Assign { value, .. } => walk_expr(value, scope, ctx, out),
        Stmt::Call { callee, args } => walk_call(callee, args, scope, ctx, out),
        Stmt::Return { value } => walk_expr(value, scope, ctx, out),
        Stmt::IndexAssign { index, value, .. } => {
            // `name` (the array being written to) has no position captured
            // in the AST — `Stmt::IndexAssign` was out of scope for the v1
            // span additions (see `wilios_core::parser::ast`) — so it's
            // deliberately not checked here rather than emitting a
            // diagnostic with a fabricated span. `index`/`value` are still
            // fully checked.
            walk_expr(index, scope, ctx, out);
            walk_expr(value, scope, ctx, out);
        }
        Stmt::Import { .. } => {
            // Handled separately by `resolve::imports` (which needs the
            // whole import graph, not just one file's scope).
        }
        Stmt::Tempo(_)
        | Stmt::Track { .. }
        | Stmt::Global
        | Stmt::Pan(_)
        | Stmt::Volume(_)
        | Stmt::TimeSignature(_)
        | Stmt::Wave(_) => {}
    }
}

fn walk_expr(expr: &Expr, scope: &HashSet<Ident>, ctx: &WalkCtx, out: &mut Vec<Diagnostic>) {
    match expr {
        Expr::Int(_) | Expr::Float(_) | Expr::Bool(_) | Expr::Pitch(_) => {}
        Expr::Var { name, line, col } => check_var(name, *line, *col, scope, ctx, out),
        Expr::Unary { expr, .. } => walk_expr(expr, scope, ctx, out),
        Expr::Binary { left, right, .. } => {
            walk_expr(left, scope, ctx, out);
            walk_expr(right, scope, ctx, out);
        }
        Expr::Func { params, body } => {
            // The body inherits the enclosing scope (globals, and — for a
            // func defined in a track — that track's bindings) plus its own
            // parameters (which shadow) and everything it `let`-binds inside
            // itself. The interpreter runs the body against a clone of the
            // caller's env with params layered on, and a `let` in the body
            // is visible to the rest of that body (a bounded `loop`'s
            // counter, say) — so pre-collect the body's bound names here the
            // same way `resolve::mod` does for a file's globals / a track.
            // See module docs.
            let mut inner_scope = scope.clone();
            inner_scope.extend(params.iter().cloned());
            inner_scope.extend(collect_bound_names(body));
            walk_stmts(body, &inner_scope, ctx, out);
        }
        Expr::Call { callee, args } => walk_call(callee, args, scope, ctx, out),
        Expr::Chord(items) | Expr::Array(items) => {
            for item in items {
                walk_expr(item, scope, ctx, out);
            }
        }
        Expr::Index { array, index } => {
            walk_expr(array, scope, ctx, out);
            walk_expr(index, scope, ctx, out);
        }
    }
}

fn walk_call(
    callee: &Expr,
    args: &[Expr],
    scope: &HashSet<Ident>,
    ctx: &WalkCtx,
    out: &mut Vec<Diagnostic>,
) {
    for a in args {
        walk_expr(a, scope, ctx, out);
    }
    match callee {
        Expr::Var { name, line, col } => {
            if scope.contains(name) {
                // Resolved via a user binding — never arity-checked, since
                // it may be the user's own (re)definition (see module docs).
                return;
            }
            if let Some(builtin) = BUILTINS.iter().find(|b| b.name == name.0) {
                if let Some(d) =
                    checks::builtin_arity_diagnostic(builtin, args.len(), *line, *col, ctx)
                {
                    out.push(d);
                }
                return;
            }
            emit_unknown_identifier(name, *line, *col, scope, ctx, out);
        }
        other => walk_expr(other, scope, ctx, out),
    }
}

fn check_var(
    name: &Ident,
    line: usize,
    col: usize,
    scope: &HashSet<Ident>,
    ctx: &WalkCtx,
    out: &mut Vec<Diagnostic>,
) {
    if scope.contains(name) {
        return;
    }
    if BUILTINS.iter().any(|b| b.name == name.0) {
        return;
    }
    emit_unknown_identifier(name, line, col, scope, ctx, out);
}

fn emit_unknown_identifier(
    name: &Ident,
    line: usize,
    col: usize,
    scope: &HashSet<Ident>,
    ctx: &WalkCtx,
    out: &mut Vec<Diagnostic>,
) {
    let span = ctx.span(line, col, &name.0);
    let suggestions = if ctx.suggestions {
        let mut candidates: Vec<Candidate> = scope
            .iter()
            .map(|i| Candidate {
                text: i.0.clone(),
                priority: 0,
            })
            .collect();
        candidates.extend(stdlib::all_symbols().map(|s| Candidate {
            text: s.name.to_string(),
            priority: 2,
        }));
        suggest::suggest(&name.0, &candidates)
    } else {
        Vec::new()
    };
    let excerpt = ctx.maybe_excerpt(&span);
    out.push(Diagnostic {
        severity: Severity::Error,
        code: "WIL-E3001",
        message: format!("Unknown identifier `{}`.", name.0),
        span,
        excerpt,
        suggestions,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::parser::Parser;

    fn parse(src: &str) -> crate::parser::parser::Program {
        let tokens = Lexer::new(src).lex().unwrap();
        Parser::new(tokens).parse().unwrap()
    }

    fn ctx<'a>(source: &'a str) -> WalkCtx<'a> {
        WalkCtx {
            file: "<source>",
            source,
            suggestions: true,
            excerpt: false,
        }
    }

    #[test]
    fn collect_bound_names_recurses_into_loop_and_if_not_func() {
        let src = "let a = 1\nloop (a < 2) { let b = 2 }\nif (a == 1) { let c = 3 } else { let d = 4 }\nlet e = func() { let f = 5 }";
        let program = parse(src);
        let names = collect_bound_names(&program.global_stmts);
        for n in ["a", "b", "c", "d", "e"] {
            assert!(
                names.contains(&Ident(n.to_string())),
                "expected {n} in bound names"
            );
        }
        assert!(
            !names.contains(&Ident("f".to_string())),
            "func body scope must not leak out"
        );
    }

    #[test]
    fn unknown_identifier_is_flagged() {
        let src = "track 1\nlet x = arpegio\n";
        let program = parse(src);
        let track = &program.tracks[0];
        let scope = collect_bound_names(&track.statements);
        let mut out = Vec::new();
        walk_stmts(&track.statements, &scope, &ctx(src), &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].code, "WIL-E3001");
        assert!(out[0].message.contains("arpegio"));
    }

    #[test]
    fn known_local_binding_is_not_flagged() {
        let src = "track 1\nlet x = 1\nlet y = x\n";
        let program = parse(src);
        let track = &program.tracks[0];
        let scope = collect_bound_names(&track.statements);
        let mut out = Vec::new();
        walk_stmts(&track.statements, &scope, &ctx(src), &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn builtin_reference_is_not_flagged() {
        let src = "track 1\nprint(1)\n";
        let program = parse(src);
        let track = &program.tracks[0];
        let scope = collect_bound_names(&track.statements);
        let mut out = Vec::new();
        walk_stmts(&track.statements, &scope, &ctx(src), &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn function_body_can_see_globals_and_builtins() {
        // A func body inherits the enclosing scope plus its params, and the
        // builtins are always resolved — so nothing here is unresolved.
        let src = "let g = 7\nlet f = func() { print(g) }\n";
        let program = parse(src);
        let scope = collect_bound_names(&program.global_stmts);
        let mut out = Vec::new();
        walk_stmts(&program.global_stmts, &scope, &ctx(src), &mut out);
        assert!(
            out.is_empty(),
            "func body should see the global `g` and the builtin `print`, got {out:?}"
        );
    }

    #[test]
    fn function_body_can_call_a_sibling_function() {
        // Mutually-recursive top-level funcs: neither call is unresolved.
        let src = "let a = func() { b() }\nlet b = func() { a() }\n";
        let program = parse(src);
        let scope = collect_bound_names(&program.global_stmts);
        let mut out = Vec::new();
        walk_stmts(&program.global_stmts, &scope, &ctx(src), &mut out);
        assert!(
            out.is_empty(),
            "sibling func calls should resolve, got {out:?}"
        );
    }

    #[test]
    fn function_param_is_visible_in_its_own_body() {
        let src = "let f = func(x) { let y = x }\n";
        let program = parse(src);
        let scope = collect_bound_names(&program.global_stmts);
        let mut out = Vec::new();
        walk_stmts(&program.global_stmts, &scope, &ctx(src), &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn function_body_local_let_is_visible_in_the_rest_of_the_body() {
        // A bounded-loop counter declared inside a func body: `i` must
        // resolve in the loop condition and the increment, exactly as the
        // interpreter runs it. (Regression: `Expr::Func` used to add only
        // params to the inner scope, not the body's own `let`s.)
        let src = "let count = func(n) {\n  let i = 0\n  loop (i < n) {\n    i = i + 1\n    <C4> 1/8\n  }\n}\n";
        let program = parse(src);
        let scope = collect_bound_names(&program.global_stmts);
        let mut out = Vec::new();
        walk_stmts(&program.global_stmts, &scope, &ctx(src), &mut out);
        assert!(
            out.is_empty(),
            "a func body's own `let` should resolve in the rest of the body, got {out:?}"
        );
    }

    #[test]
    fn user_binding_shadowing_a_builtin_skips_arity_check() {
        // `len` is redefined by the user with a different arity; calling it
        // with 0 args must not trigger a builtin arity diagnostic, since
        // the flat, non-block scoping model can't know which definition
        // "wins" without evaluating (see module docs).
        let src = "let len = func() { return 1 }\nlen()\n";
        let program = parse(src);
        let scope = collect_bound_names(&program.global_stmts);
        let mut out = Vec::new();
        walk_stmts(&program.global_stmts, &scope, &ctx(src), &mut out);
        assert!(out.is_empty());
    }
}
