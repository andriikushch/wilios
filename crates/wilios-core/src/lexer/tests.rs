use super::Lexer;
use crate::lexer::Token;

#[test]
fn lex_simple_note_sharp() {
    let mut l = Lexer::new(" G#4 1/8.");
    let tokens = l
        .lex()
        .unwrap()
        .into_iter()
        .map(|s| s.token)
        .collect::<Vec<_>>();

    assert_eq!(
        tokens,
        vec![
            Token::Pitch {
                letter: 'G',
                accidental: 1,
                octave: 4
            },
            Token::Duration {
                beats: 1,
                division: 8,
                dotted: true
            },
            Token::EOF
        ]
    );
}

#[test]
fn lex_simple_note_flat() {
    let mut l = Lexer::new(" Bb3 1/4");
    let tokens = l
        .lex()
        .unwrap()
        .into_iter()
        .map(|s| s.token)
        .collect::<Vec<_>>();

    assert_eq!(
        tokens,
        vec![
            Token::Pitch {
                letter: 'B',
                accidental: -1,
                octave: 3
            },
            Token::Duration {
                beats: 1,
                division: 4,
                dotted: false
            },
            Token::EOF
        ]
    );
}

#[test]
fn lex_simple_note_natural() {
    let mut l = Lexer::new(" C5 1/2.");
    let tokens = l
        .lex()
        .unwrap()
        .into_iter()
        .map(|s| s.token)
        .collect::<Vec<_>>();

    assert_eq!(
        tokens,
        vec![
            Token::Pitch {
                letter: 'C',
                accidental: 0,
                octave: 5
            },
            Token::Duration {
                beats: 1,
                division: 2,
                dotted: true
            },
            Token::EOF
        ]
    );
}

#[test]
fn lex_multiple_notes() {
    let mut l = Lexer::new(" G4 1/4 \n  F#3 1/8 \n  Bb3 1/2.");
    let tokens = l
        .lex()
        .unwrap()
        .into_iter()
        .map(|s| s.token)
        .collect::<Vec<_>>();

    assert_eq!(
        tokens,
        vec![
            Token::Pitch {
                letter: 'G',
                accidental: 0,
                octave: 4
            },
            Token::Duration {
                beats: 1,
                division: 4,
                dotted: false
            },
            Token::Newline,
            Token::Pitch {
                letter: 'F',
                accidental: 1,
                octave: 3
            },
            Token::Duration {
                beats: 1,
                division: 8,
                dotted: false
            },
            Token::Newline,
            Token::Pitch {
                letter: 'B',
                accidental: -1,
                octave: 3
            },
            Token::Duration {
                beats: 1,
                division: 2,
                dotted: true
            },
            Token::EOF
        ]
    );
}

#[test]
fn lex_rest() {
    let mut l = Lexer::new("rest 1/2.");
    let tokens = l
        .lex()
        .unwrap()
        .into_iter()
        .map(|s| s.token)
        .collect::<Vec<_>>();

    assert_eq!(
        tokens,
        vec![
            Token::Rest,
            Token::Duration {
                beats: 1,
                division: 2,
                dotted: true
            },
            Token::EOF
        ]
    );
}

#[test]
fn lex_track() {
    let mut l = Lexer::new("track 1");
    let tokens = l
        .lex()
        .unwrap()
        .into_iter()
        .map(|s| s.token)
        .collect::<Vec<_>>();

    assert_eq!(tokens, vec![Token::Track, Token::Int(1), Token::EOF]);
}

#[test]
fn lex_global() {
    let mut l = Lexer::new("global");
    let tokens = l
        .lex()
        .unwrap()
        .into_iter()
        .map(|s| s.token)
        .collect::<Vec<_>>();

    assert_eq!(tokens, vec![Token::Global, Token::EOF]);
}

#[test]
fn lex_tempo() {
    let mut l = Lexer::new("tempo 0");
    let tokens = l
        .lex()
        .unwrap()
        .into_iter()
        .map(|s| s.token)
        .collect::<Vec<_>>();

    assert_eq!(tokens, vec![Token::Tempo, Token::Int(0), Token::EOF]);
}

#[test]
fn lex_parenthesis() {
    let mut l = Lexer::new("()");
    let tokens = l
        .lex()
        .unwrap()
        .into_iter()
        .map(|s| s.token)
        .collect::<Vec<_>>();

    assert_eq!(
        tokens,
        vec![Token::LParenthesis, Token::RParenthesis, Token::EOF]
    );
}

#[test]
fn lex_volume() {
    let mut l = Lexer::new("volume 100");
    let tokens = l
        .lex()
        .unwrap()
        .into_iter()
        .map(|s| s.token)
        .collect::<Vec<_>>();

    assert_eq!(tokens, vec![Token::Volume, Token::Int(100), Token::EOF]);
}

#[test]
fn lex_pan() {
    let mut l = Lexer::new("pan -100");
    let tokens = l
        .lex()
        .unwrap()
        .into_iter()
        .map(|s| s.token)
        .collect::<Vec<_>>();

    assert_eq!(
        tokens,
        vec![Token::Pan, Token::Minus, Token::Int(100), Token::EOF]
    );
}

#[test]
fn lex_loop_1() {
    let mut l = Lexer::new("loop {");
    let tokens = l
        .lex()
        .unwrap()
        .into_iter()
        .map(|s| s.token)
        .collect::<Vec<_>>();

    assert_eq!(tokens, vec![Token::Loop, Token::LBrace, Token::EOF]);
}

#[test]
fn lex_loop_2() {
    let mut l = Lexer::new("loop (true) {");
    let tokens = l
        .lex()
        .unwrap()
        .into_iter()
        .map(|s| s.token)
        .collect::<Vec<_>>();

    assert_eq!(
        tokens,
        vec![
            Token::Loop,
            Token::LParenthesis,
            Token::Bool(true),
            Token::RParenthesis,
            Token::LBrace,
            Token::EOF
        ]
    );
}

#[test]
fn lex_rbrace() {
    let mut l = Lexer::new("}");
    let tokens = l
        .lex()
        .unwrap()
        .into_iter()
        .map(|s| s.token)
        .collect::<Vec<_>>();

    assert_eq!(tokens, vec![Token::RBrace, Token::EOF]);
}

#[test]
fn lex_if_1() {
    let mut l = Lexer::new("if (true) {");
    let tokens = l
        .lex()
        .unwrap()
        .into_iter()
        .map(|s| s.token)
        .collect::<Vec<_>>();

    assert_eq!(
        tokens,
        vec![
            Token::If,
            Token::LParenthesis,
            Token::Bool(true),
            Token::RParenthesis,
            Token::LBrace,
            Token::EOF
        ]
    );
}

#[test]
fn lex_if_else() {
    let mut l = Lexer::new("if (true) { } else {");
    let tokens = l
        .lex()
        .unwrap()
        .into_iter()
        .map(|s| s.token)
        .collect::<Vec<_>>();

    assert_eq!(
        tokens,
        vec![
            Token::If,
            Token::LParenthesis,
            Token::Bool(true),
            Token::RParenthesis,
            Token::LBrace,
            Token::RBrace,
            Token::Else,
            Token::LBrace,
            Token::EOF
        ]
    );
}

#[test]
fn lex_boolean() {
    let mut l = Lexer::new("true false");
    let tokens = l
        .lex()
        .unwrap()
        .into_iter()
        .map(|s| s.token)
        .collect::<Vec<_>>();

    assert_eq!(
        tokens,
        vec![Token::Bool(true), Token::Bool(false), Token::EOF]
    );
}

#[test]
fn lex_plus() {
    let mut l = Lexer::new("1+1");
    let tokens = l
        .lex()
        .unwrap()
        .into_iter()
        .map(|s| s.token)
        .collect::<Vec<_>>();

    assert_eq!(
        tokens,
        vec![Token::Int(1), Token::Plus, Token::Int(1), Token::EOF]
    );
}

#[test]
fn lex_minus() {
    let mut l = Lexer::new("1-1");
    let tokens = l
        .lex()
        .unwrap()
        .into_iter()
        .map(|s| s.token)
        .collect::<Vec<_>>();

    assert_eq!(
        tokens,
        vec![Token::Int(1), Token::Minus, Token::Int(1), Token::EOF]
    );
}

#[test]
fn lex_eq_1() {
    let mut l = Lexer::new("==");
    let tokens = l
        .lex()
        .unwrap()
        .into_iter()
        .map(|s| s.token)
        .collect::<Vec<_>>();

    assert_eq!(tokens, vec![Token::Eq, Token::EOF]);
}

#[test]
fn lex_gt_eq() {
    let mut l = Lexer::new(">=");
    let tokens = l
        .lex()
        .unwrap()
        .into_iter()
        .map(|s| s.token)
        .collect::<Vec<_>>();

    assert_eq!(tokens, vec![Token::GtEq, Token::EOF]);
}

#[test]
fn lex_gt() {
    let mut l = Lexer::new(">");
    let tokens = l
        .lex()
        .unwrap()
        .into_iter()
        .map(|s| s.token)
        .collect::<Vec<_>>();

    assert_eq!(tokens, vec![Token::Gt, Token::EOF]);
}

#[test]
fn lex_lt_eq() {
    let mut l = Lexer::new("<=");
    let tokens = l
        .lex()
        .unwrap()
        .into_iter()
        .map(|s| s.token)
        .collect::<Vec<_>>();

    assert_eq!(tokens, vec![Token::LtEq, Token::EOF]);
}

#[test]
fn lex_lt() {
    let mut l = Lexer::new("<");
    let tokens = l
        .lex()
        .unwrap()
        .into_iter()
        .map(|s| s.token)
        .collect::<Vec<_>>();

    assert_eq!(tokens, vec![Token::Lt, Token::EOF]);
}

#[test]
fn lex_not_not_eq() {
    let mut l = Lexer::new("! !=");
    let tokens = l
        .lex()
        .unwrap()
        .into_iter()
        .map(|s| s.token)
        .collect::<Vec<_>>();

    assert_eq!(tokens, vec![Token::Not, Token::NotEq, Token::EOF]);
}

#[test]
fn lex_star() {
    let mut l = Lexer::new("*");
    let tokens = l
        .lex()
        .unwrap()
        .into_iter()
        .map(|s| s.token)
        .collect::<Vec<_>>();

    assert_eq!(tokens, vec![Token::Star, Token::EOF]);
}

#[test]
fn lex_slash() {
    let mut l = Lexer::new("/");
    let tokens = l
        .lex()
        .unwrap()
        .into_iter()
        .map(|s| s.token)
        .collect::<Vec<_>>();

    assert_eq!(tokens, vec![Token::Slash, Token::EOF]);
}

#[test]
fn lex_percent() {
    let mut l = Lexer::new("%");
    let tokens = l
        .lex()
        .unwrap()
        .into_iter()
        .map(|s| s.token)
        .collect::<Vec<_>>();

    assert_eq!(tokens, vec![Token::Percent, Token::EOF]);
}

#[test]
fn lex_fm_keywords() {
    let mut l = Lexer::new("fm op algorithm level ratio");
    let tokens = l
        .lex()
        .unwrap()
        .into_iter()
        .map(|s| s.token)
        .collect::<Vec<_>>();
    assert_eq!(
        tokens,
        vec![
            Token::Fm,
            Token::Op,
            Token::Algorithm,
            Token::Level,
            Token::Ratio,
            Token::EOF
        ]
    );
}

#[test]
fn lex_arrow() {
    let mut l = Lexer::new("->");
    let tokens = l
        .lex()
        .unwrap()
        .into_iter()
        .map(|s| s.token)
        .collect::<Vec<_>>();
    assert_eq!(tokens, vec![Token::Arrow, Token::EOF]);
}

#[test]
fn lex_brackets() {
    let mut l = Lexer::new("[]");
    let tokens = l
        .lex()
        .unwrap()
        .into_iter()
        .map(|s| s.token)
        .collect::<Vec<_>>();
    assert_eq!(tokens, vec![Token::LBracket, Token::RBracket, Token::EOF]);
}

#[test]
fn lex_arrow_vs_minus() {
    // `->` is Arrow; `-` alone or followed by a digit is still Minus
    let mut l = Lexer::new("2->1 2-1");
    let tokens = l
        .lex()
        .unwrap()
        .into_iter()
        .map(|s| s.token)
        .collect::<Vec<_>>();
    assert_eq!(
        tokens,
        vec![
            Token::Int(2),
            Token::Arrow,
            Token::Int(1),
            Token::Int(2),
            Token::Minus,
            Token::Int(1),
            Token::EOF
        ]
    );
}

#[test]
fn lex_fm_algorithm_line() {
    let mut l = Lexer::new("algorithm [2->1, 3->2]");
    let tokens = l
        .lex()
        .unwrap()
        .into_iter()
        .map(|s| s.token)
        .collect::<Vec<_>>();
    assert_eq!(
        tokens,
        vec![
            Token::Algorithm,
            Token::LBracket,
            Token::Int(2),
            Token::Arrow,
            Token::Int(1),
            Token::Comma,
            Token::Int(3),
            Token::Arrow,
            Token::Int(2),
            Token::RBracket,
            Token::EOF
        ]
    );
}

#[test]
fn lex_import_keyword() {
    let mut l = Lexer::new("import");
    let tokens = l
        .lex()
        .unwrap()
        .into_iter()
        .map(|s| s.token)
        .collect::<Vec<_>>();
    assert_eq!(tokens, vec![Token::Import, Token::EOF]);
}

#[test]
fn lex_string_literal() {
    let mut l = Lexer::new(r#""hello.trx""#);
    let tokens = l
        .lex()
        .unwrap()
        .into_iter()
        .map(|s| s.token)
        .collect::<Vec<_>>();
    assert_eq!(
        tokens,
        vec![Token::StringLit("hello.trx".to_string()), Token::EOF]
    );
}

#[test]
fn lex_import_with_string() {
    let mut l = Lexer::new(r#"import "lib.trx""#);
    let tokens = l
        .lex()
        .unwrap()
        .into_iter()
        .map(|s| s.token)
        .collect::<Vec<_>>();
    assert_eq!(
        tokens,
        vec![
            Token::Import,
            Token::StringLit("lib.trx".to_string()),
            Token::EOF,
        ]
    );
}

#[test]
fn lex_string_escape_sequences() {
    let mut l = Lexer::new(r#""foo\nbar\\baz""#);
    let tokens = l
        .lex()
        .unwrap()
        .into_iter()
        .map(|s| s.token)
        .collect::<Vec<_>>();
    assert_eq!(
        tokens,
        vec![Token::StringLit("foo\nbar\\baz".to_string()), Token::EOF]
    );
}

#[test]
fn lex_string_unterminated_error() {
    let mut l = Lexer::new(r#""unterminated"#);
    assert!(l.lex().is_err());
}

#[test]
fn lex_and_or() {
    let mut l = Lexer::new("&&||");
    let tokens = l
        .lex()
        .unwrap()
        .into_iter()
        .map(|s| s.token)
        .collect::<Vec<_>>();

    assert_eq!(tokens, vec![Token::And, Token::Or, Token::EOF]);
}

#[test]
fn lex_equal() {
    let mut l = Lexer::new("true==!false");
    let tokens = l
        .lex()
        .unwrap()
        .into_iter()
        .map(|s| s.token)
        .collect::<Vec<_>>();

    assert_eq!(
        tokens,
        vec![
            Token::Bool(true),
            Token::Eq,
            Token::Not,
            Token::Bool(false),
            Token::EOF
        ]
    );
}

#[test]
fn lex_let() {
    let mut l = Lexer::new("let my_var = 12");
    let tokens = l
        .lex()
        .unwrap()
        .into_iter()
        .map(|s| s.token)
        .collect::<Vec<_>>();

    assert_eq!(
        tokens,
        vec![
            Token::Let,
            Token::Ident("my_var".to_string()),
            Token::Assignment,
            Token::Int(12),
            Token::EOF
        ]
    );
}

#[test]
fn lex_assign() {
    let mut l = Lexer::new("my_var = 12");
    let tokens = l
        .lex()
        .unwrap()
        .into_iter()
        .map(|s| s.token)
        .collect::<Vec<_>>();

    assert_eq!(
        tokens,
        vec![
            Token::Ident("my_var".to_string()),
            Token::Assignment,
            Token::Int(12),
            Token::EOF
        ]
    );
}
