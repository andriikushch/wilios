use std::collections::HashSet;
use std::path::PathBuf;

use super::{Parser, Program, TrackAst};
use crate::lexer::Lexer;
use crate::parser::ast::{BinaryOp, Duration, Expr, FmOperator, Ident, Pitch, Stmt, Waveform};

#[test]
fn parse_tempo_1() {
    let mut l = Lexer::new("track 0\ntempo 10");
    let tokens = l.lex().unwrap();

    let program = Parser::new(tokens).parse().unwrap();

    let tracks = TrackAst {
        id: 0,
        statements: vec![Stmt::Tempo(10)],
    };

    let expected = Program {
        global_stmts: vec![],
        tracks: vec![tracks],
    };
    assert_eq!(program, expected);
}

#[test]
fn parse_rest_1() {
    let mut l = Lexer::new("track 0\nrest 1/4");
    let tokens = l.lex().unwrap();

    let program = Parser::new(tokens).parse().unwrap();

    let expected = Program {
        global_stmts: vec![],
        tracks: vec![TrackAst {
            id: 0,
            statements: vec![Stmt::Rest {
                duration: Duration {
                    beats: Expr::Int(1),
                    division: Expr::Int(4),
                    dotted: false,
                },
            }],
        }],
    };

    assert_eq!(program, expected);
}

#[test]
fn parse_rest_2() {
    let mut l = Lexer::new(
        "
    track 0
    rest 1/4
    track 2
    rest 1/4
    track 0
    rest 1/4",
    );
    let tokens = l.lex().unwrap();

    let program = Parser::new(tokens).parse().unwrap();

    let tracks = vec![
        TrackAst {
            id: 0,
            statements: vec![
                Stmt::Rest {
                    duration: Duration {
                        beats: Expr::Int(1),
                        division: Expr::Int(4),
                        dotted: false,
                    },
                },
                Stmt::Rest {
                    duration: Duration {
                        beats: Expr::Int(1),
                        division: Expr::Int(4),
                        dotted: false,
                    },
                },
            ],
        },
        TrackAst {
            id: 2,
            statements: vec![Stmt::Rest {
                duration: Duration {
                    beats: Expr::Int(1),
                    division: Expr::Int(4),
                    dotted: false,
                },
            }],
        },
    ];

    let expected = Program {
        global_stmts: vec![],
        tracks: tracks,
    };

    assert_eq!(program, expected);
}

#[test]
fn parse_note_1() {
    let mut l = Lexer::new("track 0\n<Eb4> 1/4");
    let tokens = l.lex().unwrap();

    let program = Parser::new(tokens).parse().unwrap();

    let expected = Program {
        global_stmts: vec![],
        tracks: vec![TrackAst {
            id: 0,
            statements: vec![Stmt::Chord {
                pitches: vec![Expr::Pitch(Pitch {
                    letter: 'E',
                    accidental: -1,
                    octave: 4,
                })],
                duration: Duration {
                    beats: Expr::Int(1),
                    division: Expr::Int(4),
                    dotted: false,
                },
            }],
        }],
    };

    assert_eq!(program, expected);
}

#[test]
fn parse_chord_1() {
    let mut l = Lexer::new("track 0\n< C3 , Db3 , E5 > 1/4");
    let tokens = l.lex().unwrap();

    let program = Parser::new(tokens).parse().unwrap();

    let expected = Program {
        global_stmts: vec![],
        tracks: vec![TrackAst {
            id: 0,
            statements: vec![Stmt::Chord {
                pitches: vec![
                    Expr::Pitch(Pitch {
                        letter: 'C',
                        accidental: 0,
                        octave: 3,
                    }),
                    Expr::Pitch(Pitch {
                        letter: 'D',
                        accidental: -1,
                        octave: 3,
                    }),
                    Expr::Pitch(Pitch {
                        letter: 'E',
                        accidental: 0,
                        octave: 5,
                    }),
                ],
                duration: Duration {
                    beats: Expr::Int(1),
                    division: Expr::Int(4),
                    dotted: false,
                },
            }],
        }],
    };

    assert_eq!(program, expected);
}

#[test]
fn parse_pan() {
    let mut l = Lexer::new("track 0\npan 100 \n pan -50");
    let tokens = l.lex().unwrap();

    let program = Parser::new(tokens).parse().unwrap();

    let tracks = TrackAst {
        id: 0,
        statements: vec![Stmt::Pan(100), Stmt::Pan(-50)],
    };

    let expected = Program {
        global_stmts: vec![],
        tracks: vec![tracks],
    };

    assert_eq!(program, expected);
}

#[test]
fn parse_volume() {
    let mut l = Lexer::new("track 0\nvolume 100 \n volume 0");
    let tokens = l.lex().unwrap();

    let program = Parser::new(tokens).parse().unwrap();

    let tracks = TrackAst {
        id: 0,
        statements: vec![Stmt::Volume(100), Stmt::Volume(0)],
    };

    let expected = Program {
        global_stmts: vec![],
        tracks: vec![tracks],
    };

    assert_eq!(program, expected,);
}

#[test]
fn parse_loop_with_condition() {
    let mut l = Lexer::new(
        "
    track 0
    loop (1+2==3) {
    }
    ",
    );
    let tokens = l.lex().unwrap();

    let program = Parser::new(tokens).parse().unwrap();

    let expected = Program {
        global_stmts: vec![],
        tracks: vec![TrackAst {
            id: 0,
            statements: vec![Stmt::Loop {
                condition: Expr::Binary {
                    left: Box::new(Expr::Binary {
                        left: Box::new(Expr::Int(1)),
                        op: BinaryOp::Add,
                        right: Box::new(Expr::Int(2)),
                    }),
                    op: BinaryOp::Eq,
                    right: Box::new(Expr::Int(3)),
                },
                body: vec![],
            }],
        }],
    };

    assert_eq!(program, expected);
}

#[test]
fn parse_lopp_with_condition_true() {
    let mut l = Lexer::new(
        "
    track 0
    loop (true) {
    }
    ",
    );
    let tokens = l.lex().unwrap();

    let program = Parser::new(tokens).parse().unwrap();

    let tracks = TrackAst {
        id: 0,
        statements: vec![Stmt::Loop {
            condition: Expr::Bool(true),
            body: vec![],
        }],
    };

    let expected = Program {
        global_stmts: vec![],
        tracks: vec![tracks],
    };

    assert_eq!(program, expected,);
}

#[test]
fn parse_let() {
    let mut l = Lexer::new("let my_var = 123");
    let tokens = l.lex().unwrap();

    let program = Parser::new(tokens).parse().unwrap();

    let expected = Program {
        global_stmts: vec![Stmt::Let {
            name: Ident("my_var".to_string()),
            value: Expr::Int(123),
        }],
        tracks: vec![],
    };

    assert_eq!(program, expected);
}

#[test]
fn parse_let_1() {
    let mut l = Lexer::new(
        "
    let my_var = 123 + 1
    }
    ",
    );
    let tokens = l.lex().unwrap();

    let program = Parser::new(tokens).parse().unwrap();

    let expected = Program {
        global_stmts: vec![Stmt::Let {
            name: Ident("my_var".to_string()),
            value: Expr::Binary {
                left: Box::new(Expr::Int(123)),
                op: BinaryOp::Add,
                right: Box::new(Expr::Int(1)),
            },
        }],
        tracks: vec![],
    };

    assert_eq!(program, expected,);
}

#[test]
fn parse_let_2() {
    let mut l = Lexer::new(
        "
    let my_var_a = 1
    let my_var_b = my_var_a + 1
    }
    ",
    );
    let tokens = l.lex().unwrap();

    let program = Parser::new(tokens).parse().unwrap();

    let expected = Program {
        global_stmts: vec![
            Stmt::Let {
                name: Ident("my_var_a".to_string()),
                value: Expr::Int(1),
            },
            Stmt::Let {
                name: Ident("my_var_b".to_string()),
                value: Expr::Binary {
                    left: Box::new(Expr::Var(Ident("my_var_a".to_string()))),
                    op: BinaryOp::Add,
                    right: Box::new(Expr::Int(1)),
                },
            },
        ],
        tracks: vec![],
    };

    assert_eq!(program, expected,);
}

#[test]
fn parse_assign_1() {
    let mut l = Lexer::new(
        "
    let my_var_a = 1
    my_var_a = my_var_a + 1
    ",
    );
    let tokens = l.lex().unwrap();

    let program = Parser::new(tokens).parse().unwrap();

    let expected = Program {
        global_stmts: vec![
            Stmt::Let {
                name: Ident("my_var_a".to_string()),
                value: Expr::Int(1),
            },
            Stmt::Assign {
                name: Ident("my_var_a".to_string()),
                value: Expr::Binary {
                    left: Box::new(Expr::Var(Ident("my_var_a".to_string()))),
                    op: BinaryOp::Add,
                    right: Box::new(Expr::Int(1)),
                },
            },
        ],
        tracks: vec![],
    };

    assert_eq!(program, expected);
}

#[test]
fn parse_if_no_else() {
    let mut l = Lexer::new("track 0\nif (true) { }");
    let tokens = l.lex().unwrap();

    let program = Parser::new(tokens).parse().unwrap();

    let expected = Program {
        global_stmts: vec![],
        tracks: vec![TrackAst {
            id: 0,
            statements: vec![Stmt::If {
                condition: Expr::Bool(true),
                then_body: vec![],
                else_body: None,
            }],
        }],
    };

    assert_eq!(program, expected);
}

#[test]
fn parse_if_else() {
    let mut l = Lexer::new("track 0\nif (true) { } else { }");
    let tokens = l.lex().unwrap();

    let program = Parser::new(tokens).parse().unwrap();

    let expected = Program {
        global_stmts: vec![],
        tracks: vec![TrackAst {
            id: 0,
            statements: vec![Stmt::If {
                condition: Expr::Bool(true),
                then_body: vec![],
                else_body: Some(vec![]),
            }],
        }],
    };

    assert_eq!(program, expected);
}

#[test]
fn parse_fm_block_algorithm_only() {
    let src = "track 1\nfm {\n    algorithm [2->1]\n}";
    let tokens = Lexer::new(src).lex().unwrap();
    let program = Parser::new(tokens).parse().unwrap();

    let expected = Program {
        global_stmts: vec![],
        tracks: vec![TrackAst {
            id: 1,
            statements: vec![Stmt::FmBlock {
                ops: vec![],
                algorithm: vec![(2, 1)],
            }],
        }],
    };
    assert_eq!(program, expected);
}

#[test]
fn parse_fm_block_multi_algorithm() {
    let src = "track 1\nfm {\n    algorithm [3->2, 2->1]\n}";
    let tokens = Lexer::new(src).lex().unwrap();
    let program = Parser::new(tokens).parse().unwrap();

    let stmt = &program.tracks[0].statements[0];
    if let Stmt::FmBlock { algorithm, .. } = stmt {
        assert_eq!(algorithm, &vec![(3, 2), (2, 1)]);
    } else {
        panic!("expected FmBlock");
    }
}

#[test]
fn parse_fm_block_feedback() {
    let src = "track 1\nfm {\n    algorithm [1->1]\n}";
    let tokens = Lexer::new(src).lex().unwrap();
    let program = Parser::new(tokens).parse().unwrap();

    let stmt = &program.tracks[0].statements[0];
    if let Stmt::FmBlock { algorithm, .. } = stmt {
        assert_eq!(algorithm, &vec![(1, 1)]);
    } else {
        panic!("expected FmBlock");
    }
}

#[test]
fn parse_fm_block_with_ops() {
    let src = "track 1\nfm {\n    algorithm [2->1]\n    op 1 { ratio 1.0  level 1.0 }\n    op 2 { ratio 2.0  level 3.0 }\n}";
    let tokens = Lexer::new(src).lex().unwrap();
    let program = Parser::new(tokens).parse().unwrap();

    let expected_ops = vec![
        FmOperator {
            id: 1,
            ratio: Expr::Float(1.0),
            level: Expr::Float(1.0),
            wave: None,
            attack_ms: None,
            decay_ms: None,
            sustain_level: None,
            release_ms: None,
        },
        FmOperator {
            id: 2,
            ratio: Expr::Float(2.0),
            level: Expr::Float(3.0),
            wave: None,
            attack_ms: None,
            decay_ms: None,
            sustain_level: None,
            release_ms: None,
        },
    ];

    let stmt = &program.tracks[0].statements[0];
    if let Stmt::FmBlock { ops, algorithm } = stmt {
        assert_eq!(ops, &expected_ops);
        assert_eq!(algorithm, &vec![(2, 1)]);
    } else {
        panic!("expected FmBlock");
    }
}

#[test]
fn parse_fm_block_op_defaults() {
    // op with no ratio/level uses 1.0 defaults
    let src = "track 1\nfm {\n    op 1 {}\n}";
    let tokens = Lexer::new(src).lex().unwrap();
    let program = Parser::new(tokens).parse().unwrap();

    let stmt = &program.tracks[0].statements[0];
    if let Stmt::FmBlock { ops, .. } = stmt {
        assert_eq!(ops[0].ratio, Expr::Float(1.0));
        assert_eq!(ops[0].level, Expr::Float(1.0));
        assert_eq!(ops[0].wave, None);
    } else {
        panic!("expected FmBlock");
    }
}

#[test]
fn parse_fm_block_op_with_wave() {
    let src = "track 1\nfm {\n    op 1 { ratio 1.0  level 1.0  wave sine }\n}";
    let tokens = Lexer::new(src).lex().unwrap();
    let program = Parser::new(tokens).parse().unwrap();

    let stmt = &program.tracks[0].statements[0];
    if let Stmt::FmBlock { ops, .. } = stmt {
        assert_eq!(ops[0].wave, Some(Waveform::Sine));
    } else {
        panic!("expected FmBlock");
    }
}

#[test]
fn parse_fm_block_op_with_adsr() {
    let src = "track 1\nfm {\n    op 1 {\n        ratio 1.0\n        level 1.0\n        attack 10\n        decay 50\n        sustain 80\n        release 200\n    }\n}";
    let tokens = Lexer::new(src).lex().unwrap();
    let program = Parser::new(tokens).parse().unwrap();

    let stmt = &program.tracks[0].statements[0];
    if let Stmt::FmBlock { ops, .. } = stmt {
        assert_eq!(ops[0].attack_ms, Some(Expr::Int(10)));
        assert_eq!(ops[0].decay_ms, Some(Expr::Int(50)));
        assert_eq!(ops[0].sustain_level, Some(Expr::Int(80)));
        assert_eq!(ops[0].release_ms, Some(Expr::Int(200)));
    } else {
        panic!("expected FmBlock");
    }
}

#[test]
fn parse_global_default_scope() {
    // Statements before any track/global keyword go to global_stmts by default
    let mut l = Lexer::new("let x = 42");
    let tokens = l.lex().unwrap();

    let program = Parser::new(tokens).parse().unwrap();

    let expected = Program {
        global_stmts: vec![Stmt::Let {
            name: Ident("x".to_string()),
            value: Expr::Int(42),
        }],
        tracks: vec![],
    };

    assert_eq!(program, expected);
}

#[test]
fn parse_global_reentry() {
    // `global` keyword can be used to switch back to global scope after a track
    let mut l = Lexer::new(
        "
    let x = 1
    track 1
    tempo 120
    global
    let y = 2
    track 2
    tempo 240
    ",
    );
    let tokens = l.lex().unwrap();

    let program = Parser::new(tokens).parse().unwrap();

    let expected = Program {
        global_stmts: vec![
            Stmt::Let {
                name: Ident("x".to_string()),
                value: Expr::Int(1),
            },
            Stmt::Let {
                name: Ident("y".to_string()),
                value: Expr::Int(2),
            },
        ],
        tracks: vec![
            TrackAst {
                id: 1,
                statements: vec![Stmt::Tempo(120)],
            },
            TrackAst {
                id: 2,
                statements: vec![Stmt::Tempo(240)],
            },
        ],
    };

    assert_eq!(program, expected);
}

// =========================================================
// IMPORT TESTS
// =========================================================

fn parse_with_context(
    src: &str,
    base_dir: Option<PathBuf>,
    loaded: HashSet<PathBuf>,
) -> Result<Program, super::ParseError> {
    let tokens = Lexer::new(src).lex().unwrap();
    Parser::new_with_context(tokens, base_dir, loaded).parse()
}

#[test]
fn parse_import_global_stmts() {
    let path = "/tmp/trx_test_import_global.trx";
    std::fs::write(path, "let x = 42\n").unwrap();

    let src = format!("import \"{}\"\nlet y = 1\n", path);
    let program = parse_with_context(&src, None, HashSet::new()).unwrap();

    // imported let x comes first, then let y
    assert_eq!(
        program.global_stmts,
        vec![
            Stmt::Let {
                name: Ident("x".into()),
                value: Expr::Int(42)
            },
            Stmt::Let {
                name: Ident("y".into()),
                value: Expr::Int(1)
            },
        ]
    );
    assert_eq!(program.tracks.len(), 0);
}

#[test]
fn parse_import_track_stmts() {
    let path = "/tmp/trx_test_import_track.trx";
    std::fs::write(path, "track 1\ntempo 120\n").unwrap();

    let src = format!("import \"{}\"\n", path);
    let program = parse_with_context(&src, None, HashSet::new()).unwrap();

    assert_eq!(program.tracks.len(), 1);
    assert_eq!(program.tracks[0].id, 1);
    assert_eq!(program.tracks[0].statements, vec![Stmt::Tempo(120)]);
}

#[test]
fn parse_import_merges_same_track() {
    let lib1 = "/tmp/trx_test_merge1.trx";
    let lib2 = "/tmp/trx_test_merge2.trx";
    std::fs::write(lib1, "track 1\ntempo 120\n").unwrap();
    std::fs::write(lib2, "track 1\ntempo 240\n").unwrap();

    let src = format!("import \"{}\"\nimport \"{}\"\n", lib1, lib2);
    let program = parse_with_context(&src, None, HashSet::new()).unwrap();

    // Both tempo stmts end up in track 1
    assert_eq!(program.tracks.len(), 1);
    assert_eq!(program.tracks[0].statements.len(), 2);
    assert_eq!(program.tracks[0].statements[0], Stmt::Tempo(120));
    assert_eq!(program.tracks[0].statements[1], Stmt::Tempo(240));
}

#[test]
fn parse_import_missing_file_error() {
    let src = "import \"/tmp/trx_does_not_exist_xyz.trx\"\n";
    let result = parse_with_context(src, None, HashSet::new());
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .message
            .contains("cannot resolve import")
    );
}

#[test]
fn parse_import_missing_string_literal_error() {
    let src = "import 42\n";
    let result = parse_with_context(src, None, HashSet::new());
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .message
            .contains("expected string literal")
    );
}

#[test]
fn parse_import_circular_protection() {
    let path = "/tmp/trx_test_circular.trx";
    std::fs::write(path, format!("import \"{}\"\nlet x = 1\n", path)).unwrap();

    let canonical = PathBuf::from(path).canonicalize().unwrap();
    let base_dir = canonical.parent().map(|p| p.to_path_buf());
    // Pre-populate loaded with the file's own canonical path — simulates being inside it
    let loaded = HashSet::from([canonical]);

    let src = std::fs::read_to_string(path).unwrap();
    let result = parse_with_context(&src, base_dir, loaded).unwrap();

    // The self-import is silently skipped; let x = 1 is still parsed
    assert_eq!(
        result.global_stmts,
        vec![Stmt::Let {
            name: Ident("x".into()),
            value: Expr::Int(1)
        },]
    );
}

#[test]
fn parse_import_duplicate_ignored() {
    // Importing the same file twice produces its stmts only once
    let path = "/tmp/trx_test_dedup.trx";
    std::fs::write(path, "let shared = 99\n").unwrap();

    let src = format!("import \"{}\"\nimport \"{}\"\n", path, path);
    let program = parse_with_context(&src, None, HashSet::new()).unwrap();

    // Only one let shared = 99 (second import is skipped)
    assert_eq!(program.global_stmts.len(), 1);
}

#[test]
fn parse_import_relative_path() {
    // Write lib in /tmp, import it with a relative path from /tmp
    let lib_path = "/tmp/trx_rel_lib.trx";
    std::fs::write(lib_path, "let imported = 7\n").unwrap();

    let src = "import \"trx_rel_lib.trx\"\n";
    let base_dir = Some(PathBuf::from("/tmp"));
    let program = parse_with_context(src, base_dir, HashSet::new()).unwrap();

    assert_eq!(
        program.global_stmts,
        vec![Stmt::Let {
            name: Ident("imported".into()),
            value: Expr::Int(7)
        },]
    );
}

// =========================================================
// ARRAY TESTS
// =========================================================

fn parse_global(src: &str) -> Program {
    let tokens = Lexer::new(src).lex().unwrap();
    Parser::new(tokens).parse().unwrap()
}

#[test]
fn parse_array_empty() {
    let program = parse_global("let a = []");
    assert_eq!(
        program.global_stmts,
        vec![Stmt::Let {
            name: Ident("a".into()),
            value: Expr::Array(vec![]),
        }]
    );
}

#[test]
fn parse_array_int_literal() {
    let program = parse_global("let a = [1, 2, 3]");
    assert_eq!(
        program.global_stmts,
        vec![Stmt::Let {
            name: Ident("a".into()),
            value: Expr::Array(vec![Expr::Int(1), Expr::Int(2), Expr::Int(3)]),
        }]
    );
}

#[test]
fn parse_array_pitch_literal() {
    let program = parse_global("let notes = [C4, E4, G4]");
    assert_eq!(
        program.global_stmts,
        vec![Stmt::Let {
            name: Ident("notes".into()),
            value: Expr::Array(vec![
                Expr::Pitch(Pitch { letter: 'C', accidental: 0, octave: 4 }),
                Expr::Pitch(Pitch { letter: 'E', accidental: 0, octave: 4 }),
                Expr::Pitch(Pitch { letter: 'G', accidental: 0, octave: 4 }),
            ]),
        }]
    );
}

#[test]
fn parse_array_chord_literal() {
    let program = parse_global("let chords = [<C4, E4>, <D4, F4>]");
    assert_eq!(
        program.global_stmts,
        vec![Stmt::Let {
            name: Ident("chords".into()),
            value: Expr::Array(vec![
                Expr::Chord(vec![
                    Expr::Pitch(Pitch { letter: 'C', accidental: 0, octave: 4 }),
                    Expr::Pitch(Pitch { letter: 'E', accidental: 0, octave: 4 }),
                ]),
                Expr::Chord(vec![
                    Expr::Pitch(Pitch { letter: 'D', accidental: 0, octave: 4 }),
                    Expr::Pitch(Pitch { letter: 'F', accidental: 0, octave: 4 }),
                ]),
            ]),
        }]
    );
}

#[test]
fn parse_array_index_read() {
    let program = parse_global("let x = a[0]");
    assert_eq!(
        program.global_stmts,
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
fn parse_array_index_assign() {
    let program = parse_global("a[0] = 5");
    assert_eq!(
        program.global_stmts,
        vec![Stmt::IndexAssign {
            name: Ident("a".into()),
            index: Expr::Int(0),
            value: Expr::Int(5),
        }]
    );
}

#[test]
fn parse_array_nested_index() {
    // a[b[0]] — index with an index expression
    let program = parse_global("let x = a[b[0]]");
    assert_eq!(
        program.global_stmts,
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
fn parse_array_index_in_expr() {
    // a[0] + a[1]
    let program = parse_global("let x = a[0] + a[1]");
    assert_eq!(
        program.global_stmts,
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
