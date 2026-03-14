#![no_main]

use libfuzzer_sys::fuzz_target;
use wilios::lexer::Lexer;
use wilios::parser::parser::Parser;

fuzz_target!(|data: &[u8]| {
    let input = String::from_utf8_lossy(data);
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let Ok(tokens) = Lexer::new(&input).lex() else { return; };
        Parser::new(tokens).parse();
    }));
});
