#![no_main]

use libfuzzer_sys::fuzz_target;
use wilios::lexer::Lexer;

fuzz_target!(|data: &[u8]| {
    let input = String::from_utf8_lossy(data);
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        Lexer::new(&input).lex();
    }));
});
