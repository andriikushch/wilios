//! Wire-agnostic diagnostic types shared by `wilios_core::resolve` (the
//! static-analysis pass that produces them) and any transport that wants to
//! surface them (currently `wilios-mcp`'s `validate` tool). Deliberately has
//! no `serde` dependency — `wilios-core` doesn't otherwise need one, and a
//! transport should map these into its own serializable DTOs, the same way
//! `wilios-mcp`'s existing `SymbolDoc` maps `wilios_core::stdlib::Symbol`.

pub mod suggest;

/// A 1-based line/column plus a 0-based byte offset into some source text.
///
/// `col` is a *character* index, matching the lexer's own convention (see
/// `offset_of`'s doc comment) — not a byte or grapheme-cluster index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub line: usize,
    pub col: usize,
    pub offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    /// Display name of the file this span is in. `"<source>"` for inline
    /// source with no path (see `wilios-mcp`'s `validate` tool).
    pub file: String,
    pub start: Position,
    pub end: Position,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confidence {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    pub replacement: String,
    pub confidence: Confidence,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: &'static str,
    pub message: String,
    pub span: Span,
    /// Rendered by `render_excerpt` when the caller wants one; `None` when
    /// the request asked to omit it (`excerpt: false`).
    pub excerpt: Option<String>,
    /// Ordered best-first, at most 3 (see `suggest::suggest`). Empty when
    /// suggestions were disabled or none qualified.
    pub suggestions: Vec<Suggestion>,
}

/// Computes the 0-based byte offset of a 1-based `(line, col)` position
/// within `source`.
///
/// `col` is a **character** index, not a byte index: the lexer's internal
/// cursor (`crates/wilios-core/src/lexer/lex.rs`) walks a `Vec<char>` and
/// increments `col` once per `char`, so a multi-byte UTF-8 character before
/// the target column still only counts as 1 toward `col` — this function is
/// the one place that fact needs to be reconciled with a byte-oriented
/// offset. Computed on demand (never threaded through the lexer) since
/// diagnostics are produced at a rate (≤500 per call, ≤1MB source) where an
/// O(source length) scan per diagnostic is negligible.
///
/// Degrades gracefully (rather than panicking) if `line`/`col` point past
/// the end of `source` — returns the offset of wherever the scan ran out.
pub fn offset_of(source: &str, line: usize, col: usize) -> usize {
    let mut offset = 0usize;
    let mut lines = source.split('\n');

    for _ in 1..line {
        match lines.next() {
            Some(l) => offset += l.len() + 1, // +1 for the '\n' consumed by split
            None => return offset,
        }
    }

    if let Some(target_line) = lines.next() {
        let mut chars = target_line.chars();
        for _ in 1..col {
            match chars.next() {
                Some(c) => offset += c.len_utf8(),
                None => break,
            }
        }
    }

    offset
}

/// Position for a `Position` given its line/col, computing the byte offset
/// via `offset_of`.
pub fn position_at(source: &str, line: usize, col: usize) -> Position {
    Position {
        line,
        col,
        offset: offset_of(source, line, col),
    }
}

/// The end position of a single-line token starting at `start`, given the
/// token's own text. Never crosses a newline — every v1 diagnostic's
/// offending construct (an identifier, an import path string, a duration
/// literal) is single-line by construction, so this is exact for all of
/// them rather than an approximation.
pub fn end_position(start: Position, token_text: &str) -> Position {
    Position {
        line: start.line,
        col: start.col + token_text.chars().count(),
        offset: start.offset + token_text.len(),
    }
}

/// Renders one source line plus a caret underline spanning `span`, capped
/// at 120 characters with an ellipsis so a long generated line doesn't
/// dominate the response. `span.start.line` selects the line (multi-line
/// spans are not supported in v1 — see `wilios_core::resolve`'s docs — so
/// the caret width degrades to 1 if `end` is on a different line).
pub fn render_excerpt(source: &str, span: &Span) -> String {
    const MAX_LEN: usize = 120;

    let raw_line = source
        .lines()
        .nth(span.start.line.saturating_sub(1))
        .unwrap_or("");
    let char_count = raw_line.chars().count();
    let (shown_line, ellipsis) = if char_count > MAX_LEN {
        (raw_line.chars().take(MAX_LEN).collect::<String>(), "...")
    } else {
        (raw_line.to_string(), "")
    };

    let gutter_label = span.start.line.to_string();
    let blank_gutter = " ".repeat(gutter_label.len());

    let caret_col = span.start.col.min(char_count + 1).max(1);
    let caret_indent = " ".repeat(caret_col - 1);
    let caret_len = if span.end.line == span.start.line && span.end.col > span.start.col {
        (span.end.col - span.start.col).min(MAX_LEN)
    } else {
        1
    };
    let carets = "^".repeat(caret_len);

    format!("{gutter_label} | {shown_line}{ellipsis}\n{blank_gutter} | {caret_indent}{carets}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offset_of_first_line_first_col_is_zero() {
        assert_eq!(offset_of("hello\nworld", 1, 1), 0);
    }

    #[test]
    fn offset_of_advances_within_line() {
        assert_eq!(offset_of("hello\nworld", 1, 3), 2);
    }

    #[test]
    fn offset_of_crosses_newline() {
        // "hello\n" is 6 bytes; col 1 of line 2 starts right after it.
        assert_eq!(offset_of("hello\nworld", 2, 1), 6);
        assert_eq!(offset_of("hello\nworld", 2, 3), 8);
    }

    #[test]
    fn offset_of_counts_multibyte_chars_as_one_col_each() {
        // "café" — 'é' is 2 bytes in UTF-8 but 1 char/col.
        let s = "café\nx";
        // col 5 is just past 'é' (1-based: c=1,a=2,f=3,é=4), i.e. the '\n'.
        assert_eq!(offset_of(s, 1, 5), "café".len());
    }

    #[test]
    fn end_position_adds_token_length() {
        let start = Position {
            line: 3,
            col: 5,
            offset: 20,
        };
        let end = end_position(start, "arpegio");
        assert_eq!(end.line, 3);
        assert_eq!(end.col, 12);
        assert_eq!(end.offset, 27);
    }

    #[test]
    fn render_excerpt_matches_expected_shape() {
        let source = "let x = 1\nlet y = arpegio(c4, 4)\n";
        let span = Span {
            file: "<source>".to_string(),
            start: Position {
                line: 2,
                col: 9,
                offset: 18,
            },
            end: Position {
                line: 2,
                col: 16,
                offset: 25,
            },
        };
        let excerpt = render_excerpt(source, &span);
        assert_eq!(excerpt, "2 | let y = arpegio(c4, 4)\n  |         ^^^^^^^");
    }

    #[test]
    fn render_excerpt_truncates_long_lines() {
        let long_line = "x".repeat(200);
        let source = format!("{long_line}\n");
        let span = Span {
            file: "<source>".to_string(),
            start: Position {
                line: 1,
                col: 1,
                offset: 0,
            },
            end: Position {
                line: 1,
                col: 2,
                offset: 1,
            },
        };
        let excerpt = render_excerpt(&source, &span);
        let first_line = excerpt.lines().next().unwrap();
        assert!(first_line.ends_with("..."));
        assert!(first_line.len() < 200);
    }
}
