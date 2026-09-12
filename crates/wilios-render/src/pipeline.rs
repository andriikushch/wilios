//! `lex → parse → interpret` — the device-independent front of the pipeline.
//!
//! Produces an [`Interpreter`] that has run its global scope but not yet
//! scheduled any events, so every consumer (live playback, offline render, a
//! future `dump`) starts from the same point.

use std::path::Path;

use wilios_core::interpreter::interpreter::Interpreter;
use wilios_core::lexer::Lexer;
use wilios_core::parser::parser::Parser;

/// Load `path`, lex it, parse it (resolving imports relative to the file's
/// directory), and build an [`Interpreter`].
///
/// All failures are flattened to a human-readable `String`; the caller decides
/// how to report them.
pub fn load_interpreter(path: &Path) -> Result<Interpreter, String> {
    let file_path = path
        .canonicalize()
        .map_err(|e| format!("Error resolving '{}': {e}", path.display()))?;
    let source = std::fs::read_to_string(&file_path)
        .map_err(|e| format!("Error reading '{}': {e}", file_path.display()))?;

    let tokens = Lexer::new(&source)
        .lex()
        .map_err(|e| format!("error: {e}"))?;

    let base_dir = file_path.parent().map(|p| p.to_path_buf());
    let loaded = std::collections::HashSet::from([file_path]);
    let program = Parser::new_with_context(tokens, base_dir, loaded)
        .parse()
        .map_err(|e| format!("parse error: {e}"))?;

    Interpreter::new(program).map_err(|e| format!("runtime error in global scope: {}", e.0))
}
