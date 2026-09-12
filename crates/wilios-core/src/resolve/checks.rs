//! Static semantic checks and lints that don't need scope information:
//! builtin call arity (`WIL-E4001`), non-positive duration literals
//! (`WIL-E5002`), and the track-produces-no-events lint (`WIL-W6001`).
//!
//! Each check only fires on what's decidable without evaluation — see each
//! function's doc comment for exactly what it does and doesn't look at.

use crate::diagnostics::{self, Diagnostic, Severity, Span};
use crate::interpreter::BuiltinSpec;
use crate::parser::ast::{Expr, Stmt};
use crate::parser::parser::TrackAst;

use super::scope::WalkCtx;

/// `WIL-E4001`: a call to a builtin (one of the 4 real Rust functions in
/// `crate::interpreter::BUILTINS` — never a preset, and never a
/// user-defined function; see `resolve::scope`'s module docs for why) with
/// an argument count outside `[min_args, max_args]`. Returns `None` when
/// the call is within range.
pub fn builtin_arity_diagnostic(
    builtin: &BuiltinSpec,
    arg_count: usize,
    line: usize,
    col: usize,
    ctx: &WalkCtx,
) -> Option<Diagnostic> {
    let in_range =
        arg_count >= builtin.min_args && builtin.max_args.is_none_or(|max| arg_count <= max);
    if in_range {
        return None;
    }

    let expected = match builtin.max_args {
        Some(max) if max == builtin.min_args => {
            format!("{max} argument{}", if max == 1 { "" } else { "s" })
        }
        Some(max) => format!("{}-{} arguments", builtin.min_args, max),
        None => format!(
            "at least {} argument{}",
            builtin.min_args,
            if builtin.min_args == 1 { "" } else { "s" }
        ),
    };

    let start = diagnostics::position_at(ctx.source, line, col);
    let end = diagnostics::end_position(start, builtin.name);
    let span = Span {
        file: ctx.file.to_string(),
        start,
        end,
    };
    let excerpt = if ctx.excerpt {
        Some(diagnostics::render_excerpt(ctx.source, &span))
    } else {
        None
    };

    Some(Diagnostic {
        severity: Severity::Error,
        code: "WIL-E4001",
        message: format!("`{}` expects {expected}, found {arg_count}.", builtin.name),
        span,
        excerpt,
        suggestions: Vec::new(),
    })
}

/// `WIL-E5002`: a `Duration`'s `beats`/`division` is a non-positive integer
/// *literal* (`Expr::Int(n)` with `n <= 0`). Per `CLAUDE.md`, `Duration`'s
/// fields are only ever populated as a numeric literal or a bare
/// identifier/int (never a compound expression), so `Expr::Var` (a
/// runtime-computed value, e.g. `n/4` where `n` came from `len(...)` or
/// `rand(...)`) is deliberately left unchecked — not decidable without
/// evaluation.
///
/// Recurses into `Loop`/`If` bodies and into any `Expr::Func` body reached
/// through a `let`/`assign`/`return` value, a call argument, or a chord's
/// pitch list — a bad literal is a bug wherever it's written, independent
/// of whether that code path ever actually runs, so (unlike name
/// resolution) there's no reason to limit this to specific scopes.
pub fn duration_diagnostics(stmts: &[Stmt], ctx: &WalkCtx, out: &mut Vec<Diagnostic>) {
    for stmt in stmts {
        match stmt {
            Stmt::Chord { duration, pitches } => {
                check_duration_field(&duration.beats, duration.line, ctx, out);
                check_duration_field(&duration.division, duration.line, ctx, out);
                for p in pitches {
                    duration_in_expr(p, ctx, out);
                }
            }
            Stmt::Rest { duration } => {
                check_duration_field(&duration.beats, duration.line, ctx, out);
                check_duration_field(&duration.division, duration.line, ctx, out);
            }
            Stmt::Loop { body, .. } => duration_diagnostics(body, ctx, out),
            Stmt::If {
                then_body,
                else_body,
                ..
            } => {
                duration_diagnostics(then_body, ctx, out);
                if let Some(eb) = else_body {
                    duration_diagnostics(eb, ctx, out);
                }
            }
            Stmt::Let { value, .. } | Stmt::Assign { value, .. } | Stmt::Return { value } => {
                duration_in_expr(value, ctx, out);
            }
            Stmt::Call { callee, args } => {
                duration_in_expr(callee, ctx, out);
                for a in args {
                    duration_in_expr(a, ctx, out);
                }
            }
            Stmt::IndexAssign { index, value, .. } => {
                duration_in_expr(index, ctx, out);
                duration_in_expr(value, ctx, out);
            }
            _ => {}
        }
    }
}

fn duration_in_expr(expr: &Expr, ctx: &WalkCtx, out: &mut Vec<Diagnostic>) {
    match expr {
        Expr::Func { body, .. } => duration_diagnostics(body, ctx, out),
        Expr::Unary { expr, .. } => duration_in_expr(expr, ctx, out),
        Expr::Binary { left, right, .. } => {
            duration_in_expr(left, ctx, out);
            duration_in_expr(right, ctx, out);
        }
        Expr::Call { callee, args } => {
            duration_in_expr(callee, ctx, out);
            for a in args {
                duration_in_expr(a, ctx, out);
            }
        }
        Expr::Chord(items) | Expr::Array(items) => {
            for item in items {
                duration_in_expr(item, ctx, out);
            }
        }
        Expr::Index { array, index } => {
            duration_in_expr(array, ctx, out);
            duration_in_expr(index, ctx, out);
        }
        Expr::Int(_) | Expr::Float(_) | Expr::Bool(_) | Expr::Var { .. } | Expr::Pitch(_) => {}
    }
}

fn check_duration_field(field: &Expr, line: usize, ctx: &WalkCtx, out: &mut Vec<Diagnostic>) {
    let Expr::Int(n) = field else { return };
    if *n > 0 {
        return;
    }
    let start = diagnostics::position_at(ctx.source, line.max(1), 1);
    let span = Span {
        file: ctx.file.to_string(),
        start,
        end: start,
    };
    let excerpt = if ctx.excerpt {
        Some(diagnostics::render_excerpt(ctx.source, &span))
    } else {
        None
    };
    out.push(Diagnostic {
        severity: Severity::Error,
        code: "WIL-E5002",
        message: format!("Duration value must be positive, found {n}."),
        span,
        excerpt,
        suggestions: Vec::new(),
    });
}

/// `WIL-W6001`: a track whose statements — recursing into `Loop`/`If`
/// bodies — contain zero `Stmt::Chord`, AND call no function that might
/// contain one. An early, cheap catch for the common "wrote a track, forgot
/// to actually play anything in it" mistake.
///
/// Whether a *called* function ever plays a note is not decidable without
/// evaluating it (idiomatic wilios puts the actual notes inside helper
/// functions like the FM presets in `lib/lib.wilios` — a track typically
/// calls `epiano()` then just plays chords, but a piece can equally put the
/// whole part, chords included, inside one function the track calls once —
/// see `examples/example_1.wilios`, whose tracks do exactly this). Per the
/// tool's own "never a false positive" principle, a call to anything other
/// than a known builtin (which provably can't itself contain a `Chord`
/// statement) suppresses this lint rather than risk warning about a track
/// that actually does play something.
pub fn track_no_events_lint(track: &TrackAst, ctx: &WalkCtx) -> Option<Diagnostic> {
    if maybe_produces_events(&track.statements) {
        return None;
    }
    let start = diagnostics::position_at(ctx.source, track.line.max(1), 1);
    let span = Span {
        file: ctx.file.to_string(),
        start,
        end: start,
    };
    let excerpt = if ctx.excerpt {
        Some(diagnostics::render_excerpt(ctx.source, &span))
    } else {
        None
    };
    Some(Diagnostic {
        severity: Severity::Warning,
        code: "WIL-W6001",
        message: format!("Track {} is declared but produces no events.", track.id),
        span,
        excerpt,
        suggestions: Vec::new(),
    })
}

fn maybe_produces_events(stmts: &[Stmt]) -> bool {
    stmts.iter().any(|s| match s {
        Stmt::Chord { .. } => true,
        Stmt::Loop { body, .. } => maybe_produces_events(body),
        Stmt::If {
            then_body,
            else_body,
            ..
        } => {
            maybe_produces_events(then_body)
                || else_body
                    .as_ref()
                    .is_some_and(|eb| maybe_produces_events(eb))
        }
        Stmt::Call { callee, .. } => call_could_produce_events(callee),
        _ => false,
    })
}

/// A call can safely be ruled out as ever producing events only when its
/// callee is a bare reference to one of the 4 real builtins — those are
/// plain Rust functions with no way to emit a `Chord` event. Anything else
/// (a user-defined function, an unresolved name, a non-identifier callee
/// like an inline function-literal call) is treated conservatively as
/// "might play something", per this lint's own doc comment.
fn call_could_produce_events(callee: &crate::parser::ast::Expr) -> bool {
    match callee {
        crate::parser::ast::Expr::Var { name, .. } => !crate::interpreter::BUILTINS
            .iter()
            .any(|b| b.name == name.0),
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interpreter::BUILTINS;
    use crate::lexer::Lexer;
    use crate::parser::parser::Parser;

    fn ctx(source: &str) -> WalkCtx<'_> {
        WalkCtx {
            file: "<source>",
            source,
            suggestions: false,
            excerpt: false,
        }
    }

    fn find_builtin(name: &str) -> &'static BuiltinSpec {
        BUILTINS.iter().find(|b| b.name == name).unwrap()
    }

    #[test]
    fn arity_in_range_is_not_flagged() {
        let len = find_builtin("len");
        assert!(builtin_arity_diagnostic(len, 1, 1, 1, &ctx("")).is_none());
    }

    #[test]
    fn arity_out_of_range_is_flagged_with_exact_message() {
        let len = find_builtin("len");
        let d = builtin_arity_diagnostic(len, 2, 3, 5, &ctx("let n = len(a, b)\n")).unwrap();
        assert_eq!(d.code, "WIL-E4001");
        assert!(d.message.contains("len"));
        assert!(d.message.contains('1'));
        assert!(d.message.contains('2'));
    }

    #[test]
    fn variadic_builtin_only_enforces_minimum() {
        let print = find_builtin("print");
        assert!(builtin_arity_diagnostic(print, 0, 1, 1, &ctx("")).is_none());
        assert!(builtin_arity_diagnostic(print, 10, 1, 1, &ctx("")).is_none());
    }

    fn parse(src: &str) -> crate::parser::parser::Program {
        let tokens = Lexer::new(src).lex().unwrap();
        Parser::new(tokens).parse().unwrap()
    }

    #[test]
    fn non_positive_duration_literal_is_flagged() {
        let src = "track 0\nrest 1/0\n";
        let program = parse(src);
        let mut out = Vec::new();
        duration_diagnostics(&program.tracks[0].statements, &ctx(src), &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].code, "WIL-E5002");
    }

    #[test]
    fn positive_duration_literal_is_clean() {
        let src = "track 0\nrest 1/4\n";
        let program = parse(src);
        let mut out = Vec::new();
        duration_diagnostics(&program.tracks[0].statements, &ctx(src), &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn computed_duration_denominator_is_not_flagged() {
        // `n` is a runtime value — not decidable without evaluation. (The
        // lexer only supports a bare identifier on the `beats` side of a
        // duration, e.g. `n/4` — a leading digit is greedily consumed as
        // part of a numeric/duration token, so `1/n` doesn't even lex.)
        let src = "track 0\nlet n = 4\nrest n/4\n";
        let program = parse(src);
        let mut out = Vec::new();
        duration_diagnostics(&program.tracks[0].statements, &ctx(src), &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn duration_inside_function_body_is_still_checked() {
        let src = "let play = func() { rest 1/0 }\n";
        let program = parse(src);
        let mut out = Vec::new();
        duration_diagnostics(&program.global_stmts, &ctx(src), &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].code, "WIL-E5002");
    }

    #[test]
    fn track_with_a_chord_is_not_flagged() {
        let src = "track 0\n<C4> 1/4\n";
        let program = parse(src);
        assert!(track_no_events_lint(&program.tracks[0], &ctx(src)).is_none());
    }

    #[test]
    fn track_with_no_chord_is_flagged() {
        let src = "track 0\ntempo 120\n";
        let program = parse(src);
        let d = track_no_events_lint(&program.tracks[0], &ctx(src)).unwrap();
        assert_eq!(d.code, "WIL-W6001");
        assert_eq!(d.severity, Severity::Warning);
    }

    #[test]
    fn track_calling_a_user_function_is_not_flagged() {
        // Whether the called function ever plays a note isn't decidable
        // without evaluation, and idiomatic wilios puts the actual notes
        // inside helper functions (see `examples/example_1.wilios`, whose
        // tracks call `rytm7()`/`verse()`/`bb()` etc. and never write a
        // bare `<...>` chord directly in the track body) — so a call to
        // anything other than a known builtin must suppress this lint
        // rather than risk a false positive on exactly that pattern.
        let src = "let play = func() { <C4> 1/4 }\ntrack 0\nplay()\n";
        let program = parse(src);
        let d = track_no_events_lint(&program.tracks[0], &ctx(src));
        assert!(d.is_none());
    }

    #[test]
    fn track_calling_only_builtins_is_still_flagged() {
        // Builtins provably never emit a `Chord` event, so this remains
        // decidable and the lint should still fire.
        let src = "track 0\nprint(1)\n";
        let program = parse(src);
        let d = track_no_events_lint(&program.tracks[0], &ctx(src));
        assert!(d.is_some());
    }

    #[test]
    fn track_with_chord_inside_loop_is_not_flagged() {
        let src = "track 0\nlet i = 0\nloop (i < 4) { <C4> 1/4\ni = i + 1 }\n";
        let program = parse(src);
        assert!(track_no_events_lint(&program.tracks[0], &ctx(src)).is_none());
    }
}
