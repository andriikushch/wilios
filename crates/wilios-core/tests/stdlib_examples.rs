//! Every `example` string in `wilios_core::interpreter::BUILTINS` and
//! `wilios_core::stdlib::PRESETS` must actually lex, parse, and run —
//! otherwise the "minimal runnable example" served by wilios-mcp's
//! `describe_symbol` tool is a lie.

use wilios_core::interpreter::BUILTINS;
use wilios_core::interpreter::interpreter::Interpreter;
use wilios_core::lexer::Lexer;
use wilios_core::parser::parser::Parser;
use wilios_core::stdlib::PRESETS;

const LIB_PRESETS: &str = include_str!("../../../lib/lib.wilios");

fn run(src: &str, context: &str) {
    let tokens = Lexer::new(src)
        .lex()
        .unwrap_or_else(|e| panic!("lex error in {context} example:\n{src}\n\n{e}"));
    let program = Parser::new(tokens)
        .parse()
        .unwrap_or_else(|e| panic!("parse error in {context} example:\n{src}\n\n{e}"));
    Interpreter::new(program)
        .unwrap_or_else(|e| {
            panic!("interpreter construction error in {context} example:\n{src}\n\n{e:?}")
        })
        .schedule_until(0, 1_000_000_000_000)
        .unwrap_or_else(|e| panic!("runtime error in {context} example:\n{src}\n\n{e:?}"));
}

#[test]
fn builtin_examples_run() {
    for b in BUILTINS {
        run(b.example, &format!("builtin `{}`", b.name));
    }
}

#[test]
fn preset_examples_run() {
    for p in PRESETS {
        let src = format!("{LIB_PRESETS}\n{}", p.example);
        run(&src, &format!("preset `{}`", p.name));
    }
}
