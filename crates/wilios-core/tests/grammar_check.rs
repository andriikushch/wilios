//! Grammar correctness tests for wilios.
//!
//! Each section corresponds to a rule in doc/grammar.ebnf and verifies
//! that the lexer + parser accept exactly the constructs the grammar
//! specifies and reject constructs outside it.

use wilios_core::lexer::Lexer;
use wilios_core::parser::ast::*;
use wilios_core::parser::parser::Parser;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn lex_ok(src: &str) -> Vec<wilios_core::lexer::Spanned<wilios_core::lexer::Token>> {
    Lexer::new(src)
        .lex()
        .unwrap_or_else(|e| panic!("lex error on {:?}: {}", src, e))
}

fn lex_err(src: &str) {
    assert!(
        Lexer::new(src).lex().is_err(),
        "expected lex error for {:?} but got Ok",
        src
    );
}

fn parse_ok(src: &str) -> wilios_core::parser::parser::Program {
    let tokens = lex_ok(src);
    Parser::new(tokens)
        .parse()
        .unwrap_or_else(|e| panic!("parse error on {:?}: {}", src, e))
}

fn parse_err(src: &str) {
    let tokens = Lexer::new(src).lex();
    let result = match tokens {
        Err(_) => return, // lex error is also acceptable
        Ok(t) => Parser::new(t).parse(),
    };
    assert!(
        result.is_err(),
        "expected parse error for {:?} but got Ok",
        src
    );
}

fn global_stmts(src: &str) -> Vec<Stmt> {
    parse_ok(src).global_stmts
}

fn track_stmts(src: &str, id: usize) -> Vec<Stmt> {
    let prog = parse_ok(src);
    prog.tracks
        .into_iter()
        .find(|t| t.id == id)
        .map(|t| t.statements)
        .unwrap_or_else(|| panic!("track {} not found", id))
}

// ---------------------------------------------------------------------------
// LEXER — identifiers
// ---------------------------------------------------------------------------

#[test]
fn lex_ident_lowercase() {
    lex_ok("let foo = 1");
    lex_ok("let x_y = 1"); // underscore allowed after first char
}

#[test]
fn lex_ident_actual_constraints() {
    // The lexer implements [a-z][a-z_0-9]* :
    //   - '_' cannot be the first character (lexer: unexpected character)
    //   - digits ARE allowed after the first character
    lex_err("let _bar = 1"); // leading underscore rejected
    lex_ok("let x0 = 1"); // digit after first char is now allowed
    lex_ok("let x_0_y = 1"); // digit after first char is now allowed
}

#[test]
fn lex_ident_uppercase_rejected() {
    lex_err("let Foo = 1");
    lex_err("let FOO = 1");
    lex_err("let myVar = 1"); // camelCase rejected
}

#[test]
fn lex_ident_uppercase_still_rejected() {
    // Digits after the first character are now allowed, but uppercase letters
    // remain forbidden anywhere in an identifier.
    lex_err("let myVar2 = 1"); // uppercase 'V' still rejected
    lex_ok("let myvar2 = 1"); // all-lowercase with trailing digit is fine
}

// ---------------------------------------------------------------------------
// LEXER — pitches
// ---------------------------------------------------------------------------

#[test]
fn lex_pitch_natural() {
    lex_ok("<C4> 1/4");
    lex_ok("<A0> 1/4");
    lex_ok("<G9> 1/4");
}

#[test]
fn lex_pitch_sharp() {
    lex_ok("<F#5> 1/4");
    lex_ok("<C#4> 1/4");
}

#[test]
fn lex_pitch_flat() {
    lex_ok("<Bb3> 1/4");
    lex_ok("<Eb4> 1/4");
}

// ---------------------------------------------------------------------------
// LEXER — durations
// ---------------------------------------------------------------------------

#[test]
fn lex_duration_plain() {
    lex_ok("rest 1/4");
    lex_ok("rest 3/8");
    lex_ok("rest 1/16");
}

#[test]
fn lex_duration_dotted() {
    lex_ok("rest 1/4.");
    lex_ok("rest 1/2.");
}

// ---------------------------------------------------------------------------
// LEXER — literals
// ---------------------------------------------------------------------------

#[test]
fn lex_float_distinguished_from_duration() {
    // "2.0" is a Float, not a Duration (no "/")
    lex_ok("let x = 2.0");
}

#[test]
fn lex_string_literal_escapes() {
    lex_ok(r#"import "/tmp/fake.wilios""#);
    lex_ok(r#"let s = "hello\nworld""#);
    lex_ok(r#"let s = "tab\there""#);
    lex_ok(r#"let s = "quote\"here""#);
    lex_ok(r#"let s = "slash\\here""#);
}

#[test]
fn lex_bool_literals() {
    lex_ok("let x = true");
    lex_ok("let y = false");
}

#[test]
fn lex_comment_ignored() {
    // Comments must not produce tokens
    let prog = parse_ok("// this is a comment\nlet x = 1");
    assert_eq!(prog.global_stmts.len(), 1);
}

// ---------------------------------------------------------------------------
// LEXER — reserved keywords cannot be identifiers
// ---------------------------------------------------------------------------

#[test]
fn lex_keywords_reserved() {
    // None of these should successfully parse as `let <kw> = 1`
    for kw in &[
        "track",
        "global",
        "loop",
        "if",
        "else",
        "let",
        "func",
        "return",
        "tempo",
        "volume",
        "pan",
        "rest",
        "wave",
        "attack",
        "decay",
        "sustain",
        "release",
        "fm_ratio",
        "fm_depth",
        "fm",
        "op",
        "algorithm",
        "level",
        "ratio",
        "sine",
        "square",
        "saw",
        "tri",
        "import",
        "true",
        "false",
    ] {
        // Using a reserved word as an identifier in assignment is invalid
        let src = format!("let {} = 1", kw);
        let tokens = Lexer::new(&src).lex();
        // Either lex fails or parse fails — either is correct
        if let Ok(t) = tokens {
            assert!(
                Parser::new(t).parse().is_err(),
                "expected error when using keyword {:?} as identifier",
                kw
            );
        }
    }
}

// ---------------------------------------------------------------------------
// STATEMENTS — musical
// ---------------------------------------------------------------------------

#[test]
fn stmt_rest_plain() {
    let stmts = track_stmts("track 0\nrest 1/4", 0);
    assert!(matches!(stmts[0], Stmt::Rest { .. }));
}

#[test]
fn stmt_rest_dotted() {
    let stmts = track_stmts("track 0\nrest 1/4.", 0);
    if let Stmt::Rest { duration } = &stmts[0] {
        assert!(duration.dotted);
    } else {
        panic!("expected Rest");
    }
}

#[test]
fn stmt_chord_single_pitch() {
    let stmts = track_stmts("track 0\n<C4> 1/4", 0);
    if let Stmt::Chord { pitches, .. } = &stmts[0] {
        assert_eq!(pitches.len(), 1);
    } else {
        panic!("expected Chord");
    }
}

#[test]
fn stmt_chord_multi_pitch() {
    let stmts = track_stmts("track 0\n<C4, E4, G4> 1/2", 0);
    if let Stmt::Chord { pitches, .. } = &stmts[0] {
        assert_eq!(pitches.len(), 3);
    } else {
        panic!("expected Chord");
    }
}

#[test]
fn stmt_chord_flat_pitch() {
    let stmts = track_stmts("track 0\n<Eb4> 1/4", 0);
    if let Stmt::Chord { pitches, .. } = &stmts[0] {
        if let Expr::Pitch(p) = &pitches[0] {
            assert_eq!(p.letter, 'E');
            assert_eq!(p.accidental, -1);
            assert_eq!(p.octave, 4);
        } else {
            panic!("expected Pitch");
        }
    }
}

#[test]
fn stmt_chord_sharp_pitch() {
    let stmts = track_stmts("track 0\n<F#5> 1/4", 0);
    if let Stmt::Chord { pitches, .. } = &stmts[0] {
        if let Expr::Pitch(p) = &pitches[0] {
            assert_eq!(p.letter, 'F');
            assert_eq!(p.accidental, 1);
            assert_eq!(p.octave, 5);
        } else {
            panic!("expected Pitch");
        }
    }
}

// ---------------------------------------------------------------------------
// STATEMENTS — performance control
// ---------------------------------------------------------------------------

#[test]
fn stmt_tempo_literal_int() {
    let stmts = track_stmts("track 0\ntempo 120", 0);
    assert_eq!(stmts[0], Stmt::Tempo(120));
}

#[test]
fn stmt_tempo_rejects_non_int() {
    parse_err("track 0\ntempo 1.5");
    parse_err("track 0\ntempo x");
}

#[test]
fn stmt_volume_range() {
    let stmts = track_stmts("track 0\nvolume 0", 0);
    assert_eq!(stmts[0], Stmt::Volume(0));

    let stmts = track_stmts("track 0\nvolume 127", 0);
    assert_eq!(stmts[0], Stmt::Volume(127));
}

#[test]
fn stmt_pan_positive() {
    let stmts = track_stmts("track 0\npan 100", 0);
    assert_eq!(stmts[0], Stmt::Pan(100));
}

#[test]
fn stmt_pan_negative() {
    let stmts = track_stmts("track 0\npan -50", 0);
    assert_eq!(stmts[0], Stmt::Pan(-50));
}

#[test]
fn stmt_pan_rejects_non_int() {
    parse_err("track 0\npan 1.5");
}

// ---------------------------------------------------------------------------
// STATEMENTS — synthesis
// ---------------------------------------------------------------------------

#[test]
fn stmt_wave_all_waveforms() {
    for (name, expected) in &[
        ("sine", Waveform::Sine),
        ("square", Waveform::Square),
        ("saw", Waveform::Saw),
        ("tri", Waveform::Triangle),
    ] {
        let stmts = track_stmts(&format!("track 0\nwave {}", name), 0);
        assert_eq!(stmts[0], Stmt::Wave(expected.clone()));
    }
}

#[test]
fn stmt_wave_rejects_unknown() {
    parse_err("track 0\nwave flute");
}

#[test]
fn stmt_adsr_params() {
    let stmts = track_stmts("track 0\nattack 10\ndecay 50\nsustain 80\nrelease 200", 0);
    assert!(matches!(stmts[0], Stmt::Attack(_)));
    assert!(matches!(stmts[1], Stmt::Decay(_)));
    assert!(matches!(stmts[2], Stmt::Sustain(_)));
    assert!(matches!(stmts[3], Stmt::Release(_)));
}

#[test]
fn stmt_fm_ratio_and_depth() {
    let stmts = track_stmts("track 0\nfm_ratio 2.0\nfm_depth 1.5", 0);
    assert!(matches!(stmts[0], Stmt::FmRatio(_)));
    assert!(matches!(stmts[1], Stmt::FmDepth(_)));
}

#[test]
fn stmt_fm_ratio_rejects_non_numeric() {
    parse_err("track 0\nfm_ratio x");
}

// ---------------------------------------------------------------------------
// STATEMENTS — FM block
// ---------------------------------------------------------------------------

#[test]
fn stmt_fm_block_empty() {
    let stmts = track_stmts("track 0\nfm { }", 0);
    if let Stmt::FmBlock { ops, algorithm } = &stmts[0] {
        assert!(ops.is_empty());
        assert!(algorithm.is_empty());
    } else {
        panic!("expected FmBlock");
    }
}

#[test]
fn stmt_fm_block_algorithm() {
    let stmts = track_stmts("track 0\nfm {\n    algorithm [2->1]\n}", 0);
    if let Stmt::FmBlock { algorithm, .. } = &stmts[0] {
        assert_eq!(algorithm, &[(2, 1)]);
    }
}

#[test]
fn stmt_fm_block_multi_algorithm() {
    let stmts = track_stmts("track 0\nfm {\n    algorithm [3->2, 2->1]\n}", 0);
    if let Stmt::FmBlock { algorithm, .. } = &stmts[0] {
        assert_eq!(algorithm, &[(3, 2), (2, 1)]);
    }
}

#[test]
fn stmt_fm_block_self_feedback() {
    // Self-routing (N->N) is valid: represents operator self-feedback
    let stmts = track_stmts("track 0\nfm {\n    algorithm [1->1]\n}", 0);
    if let Stmt::FmBlock { algorithm, .. } = &stmts[0] {
        assert_eq!(algorithm, &[(1, 1)]);
    }
}

#[test]
fn stmt_fm_block_op_defaults() {
    // op with no fields uses ratio=1.0, level=1.0, wave=None, all ADSR=None
    let stmts = track_stmts("track 0\nfm {\n    op 1 {}\n}", 0);
    if let Stmt::FmBlock { ops, .. } = &stmts[0] {
        assert_eq!(ops[0].ratio, Expr::Float(1.0));
        assert_eq!(ops[0].level, Expr::Float(1.0));
        assert_eq!(ops[0].wave, None);
        assert_eq!(ops[0].attack_ms, None);
        assert_eq!(ops[0].decay_ms, None);
        assert_eq!(ops[0].sustain_level, None);
        assert_eq!(ops[0].release_ms, None);
    }
}

#[test]
fn stmt_fm_block_op_all_fields() {
    let src = "track 0\nfm {\n    op 1 {\n        ratio 2.0\n        level 0.5\n        wave square\n        attack 5\n        decay 100\n        sustain 75\n        release 300\n    }\n}";
    let stmts = track_stmts(src, 0);
    if let Stmt::FmBlock { ops, .. } = &stmts[0] {
        let op = &ops[0];
        assert_eq!(op.id, 1);
        assert_eq!(op.ratio, Expr::Float(2.0));
        assert_eq!(op.level, Expr::Float(0.5));
        assert_eq!(op.wave, Some(Waveform::Square));
        assert!(op.attack_ms.is_some());
        assert!(op.decay_ms.is_some());
        assert!(op.sustain_level.is_some());
        assert!(op.release_ms.is_some());
    }
}

#[test]
fn stmt_fm_block_op_all_waveforms() {
    for (name, expected) in &[
        ("sine", Waveform::Sine),
        ("square", Waveform::Square),
        ("saw", Waveform::Saw),
        ("tri", Waveform::Triangle),
    ] {
        let src = format!("track 0\nfm {{\n    op 1 {{ wave {} }}\n}}", name);
        let stmts = track_stmts(&src, 0);
        if let Stmt::FmBlock { ops, .. } = &stmts[0] {
            assert_eq!(ops[0].wave, Some(expected.clone()), "waveform {}", name);
        }
    }
}

#[test]
fn stmt_fm_block_rejects_unknown_field() {
    parse_err("track 0\nfm {\n    op 1 { foo 1.0 }\n}");
}

// ---------------------------------------------------------------------------
// STATEMENTS — control flow
// ---------------------------------------------------------------------------

#[test]
fn stmt_loop_empty_body() {
    let stmts = track_stmts("track 0\nloop (true) { }", 0);
    if let Stmt::Loop { body, .. } = &stmts[0] {
        assert!(body.is_empty());
    }
}

#[test]
fn stmt_loop_with_body() {
    let stmts = track_stmts("track 0\nloop (true) {\n    rest 1/4\n}", 0);
    if let Stmt::Loop { body, .. } = &stmts[0] {
        assert_eq!(body.len(), 1);
        assert!(matches!(body[0], Stmt::Rest { .. }));
    }
}

#[test]
fn stmt_if_no_else() {
    let stmts = track_stmts("track 0\nif (true) { }", 0);
    if let Stmt::If { else_body, .. } = &stmts[0] {
        assert!(else_body.is_none());
    }
}

#[test]
fn stmt_if_with_else() {
    let stmts = track_stmts("track 0\nif (true) { } else { }", 0);
    if let Stmt::If { else_body, .. } = &stmts[0] {
        assert!(else_body.is_some());
    }
}

#[test]
fn stmt_loop_missing_parens_rejected() {
    parse_err("track 0\nloop true { }");
}

#[test]
fn stmt_if_missing_parens_rejected() {
    parse_err("track 0\nif true { }");
}

// ---------------------------------------------------------------------------
// STATEMENTS — variables
// ---------------------------------------------------------------------------

#[test]
fn stmt_let_int() {
    let stmts = global_stmts("let x = 42");
    assert_eq!(
        stmts[0],
        Stmt::Let {
            name: Ident("x".into()),
            value: Expr::Int(42),
        }
    );
}

#[test]
fn stmt_let_float() {
    let stmts = global_stmts("let x = 3.14");
    assert!(matches!(
        stmts[0],
        Stmt::Let {
            value: Expr::Float(_),
            ..
        }
    ));
}

#[test]
fn stmt_let_bool_true() {
    let stmts = global_stmts("let x = true");
    assert_eq!(
        stmts[0],
        Stmt::Let {
            name: Ident("x".into()),
            value: Expr::Bool(true),
        }
    );
}

#[test]
fn stmt_let_bool_false() {
    let stmts = global_stmts("let x = false");
    assert_eq!(
        stmts[0],
        Stmt::Let {
            name: Ident("x".into()),
            value: Expr::Bool(false),
        }
    );
}

#[test]
fn stmt_assign_variable() {
    let stmts = global_stmts("let i = 0\ni = 1");
    assert!(matches!(stmts[1], Stmt::Assign { .. }));
}

#[test]
fn stmt_let_missing_equals_rejected() {
    parse_err("let x 42");
}

// ---------------------------------------------------------------------------
// STATEMENTS — functions
// ---------------------------------------------------------------------------

#[test]
fn stmt_func_definition_and_call() {
    let stmts = global_stmts("let f = func(x) { return x }\nf(1)");
    assert!(matches!(stmts[0], Stmt::Let { .. }));
    assert!(matches!(stmts[1], Stmt::Call { .. }));
}

#[test]
fn stmt_func_no_params() {
    let stmts = global_stmts("let f = func() { }\nf()");
    if let Stmt::Let {
        value: Expr::Func { params, .. },
        ..
    } = &stmts[0]
    {
        assert!(params.is_empty());
    } else {
        panic!("expected Func");
    }
}

#[test]
fn stmt_func_multiple_params() {
    let stmts = global_stmts("let f = func(a, b, c) { return a }");
    if let Stmt::Let {
        value: Expr::Func { params, .. },
        ..
    } = &stmts[0]
    {
        assert_eq!(params.len(), 3);
    }
}

#[test]
fn stmt_return_expr() {
    let stmts = global_stmts("let f = func(x) { return x }");
    if let Stmt::Let {
        value: Expr::Func { body, .. },
        ..
    } = &stmts[0]
    {
        assert!(matches!(body[0], Stmt::Return { .. }));
    }
}

// ---------------------------------------------------------------------------
// SCOPE
// ---------------------------------------------------------------------------

#[test]
fn scope_global_default() {
    // Statements before any track keyword go to global scope
    let prog = parse_ok("let x = 1");
    assert_eq!(prog.global_stmts.len(), 1);
    assert!(prog.tracks.is_empty());
}

#[test]
fn scope_track_switch() {
    let prog = parse_ok("track 3\ntempo 100");
    assert!(prog.global_stmts.is_empty());
    assert_eq!(prog.tracks.len(), 1);
    assert_eq!(prog.tracks[0].id, 3);
}

#[test]
fn scope_global_keyword_switches_back() {
    let prog = parse_ok("track 1\ntempo 120\nglobal\nlet x = 1");
    assert_eq!(prog.global_stmts.len(), 1);
    assert_eq!(prog.tracks[0].statements.len(), 1);
}

#[test]
fn scope_multiple_tracks() {
    let prog = parse_ok("track 0\ntempo 120\ntrack 1\ntempo 240");
    assert_eq!(prog.tracks.len(), 2);
    assert_eq!(prog.tracks[0].id, 0);
    assert_eq!(prog.tracks[1].id, 1);
}

#[test]
fn scope_track_revisit_appends() {
    let prog = parse_ok("track 0\ntempo 120\ntrack 0\ntempo 240");
    // Both tempo stmts end up in track 0
    assert_eq!(prog.tracks.len(), 1);
    assert_eq!(prog.tracks[0].statements.len(), 2);
}

// ---------------------------------------------------------------------------
// EXPRESSIONS — precedence
// ---------------------------------------------------------------------------

#[test]
fn expr_precedence_mul_over_add() {
    // 2 + 3 * 4 should parse as 2 + (3 * 4)
    let stmts = global_stmts("let x = 2 + 3 * 4");
    if let Stmt::Let { value, .. } = &stmts[0] {
        if let Expr::Binary { left, op, right } = value {
            assert_eq!(*op, BinaryOp::Add);
            assert!(matches!(**left, Expr::Int(2)));
            if let Expr::Binary { op: inner_op, .. } = right.as_ref() {
                assert_eq!(*inner_op, BinaryOp::Mul);
            } else {
                panic!("right side should be Mul");
            }
        } else {
            panic!("expected Binary");
        }
    }
}

#[test]
fn expr_precedence_parens_override() {
    // (2 + 3) * 4 should parse as Mul(Add(2, 3), 4)
    let stmts = global_stmts("let x = (2 + 3) * 4");
    if let Stmt::Let { value, .. } = &stmts[0] {
        if let Expr::Binary { op, left, .. } = value {
            assert_eq!(*op, BinaryOp::Mul);
            assert!(matches!(
                **left,
                Expr::Binary {
                    op: BinaryOp::Add,
                    ..
                }
            ));
        } else {
            panic!("expected Mul at top level");
        }
    }
}

#[test]
fn expr_precedence_cmp_over_logical() {
    // a < b && c > d  →  (a < b) && (c > d)
    let stmts = global_stmts("let x = 1 < 2 && 3 > 4");
    if let Stmt::Let { value, .. } = &stmts[0] {
        if let Expr::Binary { op, .. } = value {
            assert_eq!(*op, BinaryOp::And);
        } else {
            panic!("expected And at top level");
        }
    }
}

#[test]
fn expr_precedence_or_lowest() {
    // a && b || c && d  →  (a && b) || (c && d)
    let stmts = global_stmts("let x = true && false || true && false");
    if let Stmt::Let { value, .. } = &stmts[0] {
        if let Expr::Binary { op, .. } = value {
            assert_eq!(*op, BinaryOp::Or);
        } else {
            panic!("expected Or at top level");
        }
    }
}

#[test]
fn expr_left_associativity_add() {
    // 1 - 2 - 3  →  (1 - 2) - 3
    let stmts = global_stmts("let x = 1 - 2 - 3");
    if let Stmt::Let { value, .. } = &stmts[0] {
        if let Expr::Binary { op, left, right } = value {
            assert_eq!(*op, BinaryOp::Sub);
            // right should be Int(3), meaning left is (1 - 2)
            assert!(matches!(**right, Expr::Int(3)));
            assert!(matches!(
                **left,
                Expr::Binary {
                    op: BinaryOp::Sub,
                    ..
                }
            ));
        }
    }
}

#[test]
fn expr_unary_minus() {
    let stmts = global_stmts("let x = -5");
    if let Stmt::Let { value, .. } = &stmts[0] {
        // -5 parsed as Unary(Neg, 5) or as Int(-5) — either is acceptable
        // (the lexer sees negative integers)
        let _ = value;
    }
}

#[test]
fn expr_function_call_in_expr() {
    let stmts = global_stmts("let f = func(x) { return x }\nlet y = f(42)");
    if let Stmt::Let {
        value: Expr::Call { .. },
        ..
    } = &stmts[1]
    {
        // ok
    } else {
        panic!("expected Call expr in let y");
    }
}

#[test]
fn expr_nested_call() {
    let stmts =
        global_stmts("let f = func(x) { return x }\nlet g = func(x) { return x }\nlet y = f(g(1))");
    if let Stmt::Let {
        value: Expr::Call { args, .. },
        ..
    } = &stmts[2]
    {
        assert!(matches!(args[0], Expr::Call { .. }));
    } else {
        panic!("expected nested Call");
    }
}

// ---------------------------------------------------------------------------
// EXPRESSIONS — chord in expression context
// ---------------------------------------------------------------------------

#[test]
fn expr_chord_in_let() {
    let stmts = global_stmts("let c = <C4, E4, G4>");
    if let Stmt::Let {
        value: Expr::Chord(pitches),
        ..
    } = &stmts[0]
    {
        assert_eq!(pitches.len(), 3);
    } else {
        panic!("expected Chord expr");
    }
}

// ---------------------------------------------------------------------------
// DURATION — variable form
// ---------------------------------------------------------------------------

#[test]
fn duration_variable_beats() {
    let _stmts = global_stmts("let n = 1\ntrack 0\nrest n/4");
    if let Stmt::Rest { duration } = &track_stmts("let n = 1\ntrack 0\nrest n/4", 0)[0] {
        assert!(matches!(duration.beats, Expr::Var(_)));
    }
}

#[test]
fn duration_variable_division() {
    if let Stmt::Rest { duration } = &track_stmts("track 0\nrest 1/4", 0)[0] {
        assert!(matches!(duration.division, Expr::Int(4)));
        assert!(!duration.dotted);
    }
}

// ---------------------------------------------------------------------------
// PROGRAM — empty
// ---------------------------------------------------------------------------

#[test]
fn program_empty() {
    let prog = parse_ok("");
    assert!(prog.global_stmts.is_empty());
    assert!(prog.tracks.is_empty());
}

#[test]
fn program_only_comments() {
    let prog = parse_ok("// just a comment\n// another comment\n");
    assert!(prog.global_stmts.is_empty());
    assert!(prog.tracks.is_empty());
}

#[test]
fn program_only_newlines() {
    let prog = parse_ok("\n\n\n");
    assert!(prog.global_stmts.is_empty());
    assert!(prog.tracks.is_empty());
}

// ---------------------------------------------------------------------------
// ERROR CASES — malformed syntax
// ---------------------------------------------------------------------------

#[test]
fn error_chord_missing_close() {
    parse_err("track 0\n<C4 1/4");
}

#[test]
fn error_chord_missing_duration() {
    parse_err("track 0\n<C4>");
}

#[test]
fn error_let_missing_value() {
    parse_err("let x =");
}

#[test]
fn error_fm_block_missing_brace() {
    parse_err("track 0\nfm {\n    op 1 { ratio 1.0\n}");
}

#[test]
fn error_loop_missing_body() {
    parse_err("track 0\nloop (true)");
}

#[test]
fn error_unknown_statement() {
    parse_err("flute 4");
}

#[test]
fn brace_at_top_level_silently_ignored() {
    // The parser returns Ok(None) for '}' at the top level (it is treated as
    // a block-end sentinel) and skips it. A lone '}' does not cause an error.
    let prog = parse_ok("}");
    assert!(prog.global_stmts.is_empty());
}

// ---------------------------------------------------------------------------
// KNOWN LIMITATION — `!` unary not is not implemented in the parser
// ---------------------------------------------------------------------------

#[test]
fn known_limitation_unary_not_not_parseable() {
    // The `!` operator token exists (Token::Not, UnaryOp::Not) but the Pratt
    // parser's `nud()` does not handle it. Parsing `!true` must fail.
    parse_err("let x = !true");
}

// ---------------------------------------------------------------------------
// ARRAYS
// ---------------------------------------------------------------------------

#[test]
fn array_empty_literal() {
    let prog = parse_ok("let a = []");
    assert_eq!(
        prog.global_stmts,
        vec![Stmt::Let {
            name: Ident("a".into()),
            value: Expr::Array(vec![]),
        }]
    );
}

#[test]
fn array_int_literal() {
    let prog = parse_ok("let a = [1, 2, 3]");
    assert_eq!(
        prog.global_stmts,
        vec![Stmt::Let {
            name: Ident("a".into()),
            value: Expr::Array(vec![Expr::Int(1), Expr::Int(2), Expr::Int(3)]),
        }]
    );
}

#[test]
fn array_single_element() {
    let prog = parse_ok("let a = [42]");
    assert_eq!(
        prog.global_stmts,
        vec![Stmt::Let {
            name: Ident("a".into()),
            value: Expr::Array(vec![Expr::Int(42)]),
        }]
    );
}

#[test]
fn array_pitch_elements() {
    let prog = parse_ok("let notes = [C4, E4, G4]");
    assert_eq!(
        prog.global_stmts,
        vec![Stmt::Let {
            name: Ident("notes".into()),
            value: Expr::Array(vec![
                Expr::Pitch(Pitch {
                    letter: 'C',
                    accidental: 0,
                    octave: 4
                }),
                Expr::Pitch(Pitch {
                    letter: 'E',
                    accidental: 0,
                    octave: 4
                }),
                Expr::Pitch(Pitch {
                    letter: 'G',
                    accidental: 0,
                    octave: 4
                }),
            ]),
        }]
    );
}

#[test]
fn array_chord_elements() {
    let prog = parse_ok("let chords = [<C4, E4>, <D4, F4>]");
    assert_eq!(
        prog.global_stmts,
        vec![Stmt::Let {
            name: Ident("chords".into()),
            value: Expr::Array(vec![
                Expr::Chord(vec![
                    Expr::Pitch(Pitch {
                        letter: 'C',
                        accidental: 0,
                        octave: 4
                    }),
                    Expr::Pitch(Pitch {
                        letter: 'E',
                        accidental: 0,
                        octave: 4
                    }),
                ]),
                Expr::Chord(vec![
                    Expr::Pitch(Pitch {
                        letter: 'D',
                        accidental: 0,
                        octave: 4
                    }),
                    Expr::Pitch(Pitch {
                        letter: 'F',
                        accidental: 0,
                        octave: 4
                    }),
                ]),
            ]),
        }]
    );
}

#[test]
fn array_index_read_expr() {
    let prog = parse_ok("let x = a[0]");
    assert_eq!(
        prog.global_stmts,
        vec![Stmt::Let {
            name: Ident("x".into()),
            value: Expr::Index {
                array: Box::new(Expr::Var(Ident("a".into()))),
                index: Box::new(Expr::Int(0)),
            },
        }]
    );
}

#[test]
fn array_index_assign_stmt() {
    let prog = parse_ok("a[2] = 99");
    assert_eq!(
        prog.global_stmts,
        vec![Stmt::IndexAssign {
            name: Ident("a".into()),
            index: Expr::Int(2),
            value: Expr::Int(99),
        }]
    );
}

#[test]
fn array_index_with_variable() {
    let prog = parse_ok("let x = arr[i]");
    assert_eq!(
        prog.global_stmts,
        vec![Stmt::Let {
            name: Ident("x".into()),
            value: Expr::Index {
                array: Box::new(Expr::Var(Ident("arr".into()))),
                index: Box::new(Expr::Var(Ident("i".into()))),
            },
        }]
    );
}

#[test]
fn array_index_in_arithmetic() {
    // a[0] + a[1] — indexing composes with binary ops
    let prog = parse_ok("let x = a[0] + a[1]");
    assert_eq!(
        prog.global_stmts,
        vec![Stmt::Let {
            name: Ident("x".into()),
            value: Expr::Binary {
                left: Box::new(Expr::Index {
                    array: Box::new(Expr::Var(Ident("a".into()))),
                    index: Box::new(Expr::Int(0)),
                }),
                op: BinaryOp::Add,
                right: Box::new(Expr::Index {
                    array: Box::new(Expr::Var(Ident("a".into()))),
                    index: Box::new(Expr::Int(1)),
                }),
            },
        }]
    );
}

#[test]
fn array_nested_index() {
    let prog = parse_ok("let x = a[b[0]]");
    assert_eq!(
        prog.global_stmts,
        vec![Stmt::Let {
            name: Ident("x".into()),
            value: Expr::Index {
                array: Box::new(Expr::Var(Ident("a".into()))),
                index: Box::new(Expr::Index {
                    array: Box::new(Expr::Var(Ident("b".into()))),
                    index: Box::new(Expr::Int(0)),
                }),
            },
        }]
    );
}

#[test]
fn array_index_on_literal() {
    // Indexing directly on an array literal: [10, 20, 30][1]
    let prog = parse_ok("let x = [10, 20, 30][1]");
    assert_eq!(
        prog.global_stmts,
        vec![Stmt::Let {
            name: Ident("x".into()),
            value: Expr::Index {
                array: Box::new(Expr::Array(vec![
                    Expr::Int(10),
                    Expr::Int(20),
                    Expr::Int(30)
                ])),
                index: Box::new(Expr::Int(1)),
            },
        }]
    );
}
