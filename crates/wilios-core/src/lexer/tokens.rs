#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Keywords
    Track,
    Global,
    Tempo,
    Volume,
    Pan,
    TimeSignature,
    Rest,
    Loop,
    If,
    Else,
    Comma,
    Let,
    Assignment,
    Func,
    Return,

    // Synth keywords
    Wave,
    Attack,
    Decay,
    Sustain,
    Release,
    FmRatio,
    FmDepth,
    Swing,

    // FM block keywords
    Fm,
    Op,
    Algorithm,
    Level,
    Ratio,

    // Waveform name tokens
    WaveSine,
    WaveSquare,
    WaveSaw,
    WaveTri,

    Minus,
    Plus,
    Star,
    Slash,
    Percent,
    And,
    Or,

    // Musical atoms
    Pitch {
        letter: char,      // A–G
        accidental: isize, // -1 = b, 0 = natural, +1 = #
        octave: usize,
    },

    Duration {
        beats: isize,    // numerator
        division: isize, // 1, 2, 4, 8, 16...
        dotted: bool,
    },

    // Import
    Import,

    // Literals
    Int(isize),
    Float(f32),
    Bool(bool),
    Ident(String),
    StringLit(String),

    // Structure
    LParenthesis,
    RParenthesis,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Arrow,
    Newline,
    EOF,
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    Not,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Spanned<T> {
    pub token: T,
    pub line: usize,
    pub col: usize,
}

impl<T> Spanned<T> {
    pub fn new(token: T, line: usize, col: usize) -> Self {
        Self { token, line, col }
    }
}
