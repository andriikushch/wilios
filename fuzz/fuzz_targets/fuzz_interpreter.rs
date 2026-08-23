#![no_main]

use libfuzzer_sys::fuzz_target;
use wilios_core::interpreter::interpreter::Interpreter;
use wilios_core::lexer::Lexer;
use wilios_core::parser::parser::Parser;

// 100 ms: prevents infinite hangs from programs like `loop(true) {}`
// (empty loop bodies advance no time, so schedule_until never returns).
const FUZZ_WINDOW_MS: u64 = 100;

fuzz_target!(|data: &[u8]| {
    let input = String::from_utf8_lossy(data);
    let Ok(tokens) = Lexer::new(&input).lex() else { return; };
    let Ok(program) = Parser::new(tokens).parse() else { return; };
    let Ok(mut interp) = Interpreter::new(program) else { return; };
    let _ = interp.schedule_until(0, FUZZ_WINDOW_MS);
});
