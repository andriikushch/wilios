use crate::lexer::Token;

#[derive(Debug, PartialEq, Clone)]
pub struct FmOperator {
    pub id: usize,
    pub ratio: Expr,
    pub level: Expr,
    pub wave: Option<Waveform>,
    pub attack_ms: Option<Expr>,
    pub decay_ms: Option<Expr>,
    pub sustain_level: Option<Expr>,
    pub release_ms: Option<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Waveform {
    Sine,
    Square,
    Saw,
    Triangle,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Duration {
    pub beats: Expr,
    pub division: Expr,
    pub dotted: bool,
    pub line: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeSignature {
    pub numerator: usize,
    pub denominator: usize,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Pitch {
    pub letter: char,
    pub accidental: isize,
    pub octave: usize,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Stmt {
    // musical
    Chord {
        pitches: Vec<Expr>,
        duration: Duration,
    },
    Rest {
        duration: Duration,
    },

    // control
    Tempo(usize),
    Track {
        id: usize,
        line: usize,
    },
    Global,
    Pan(isize),
    Volume(usize),
    TimeSignature(TimeSignature),

    // synth
    Wave(Waveform),
    Attack(Expr),
    Decay(Expr),
    Sustain(Expr),
    Release(Expr),
    FmRatio(Expr),
    FmDepth(Expr),
    Swing(Expr),
    /// Per-track resonant low-pass filter cutoff, in Hz.
    Cutoff(Expr),
    /// Per-track filter resonance, 0..1.
    Resonance(Expr),
    /// Per-track vibrato: `vibrato <depth_cents> <rate_hz>`.
    Vibrato {
        depth: Expr,
        rate: Expr,
    },
    FmBlock {
        ops: Vec<FmOperator>,
        algorithm: Vec<(usize, usize)>, // (modulator_id, target_id)
    },

    Loop {
        condition: Expr,
        body: Vec<Stmt>,
    },

    If {
        condition: Expr,
        then_body: Vec<Stmt>,
        else_body: Option<Vec<Stmt>>,
    },

    Let {
        name: Ident,
        value: Expr,
    },

    Assign {
        name: Ident,
        value: Expr,
    },

    Call {
        callee: Expr,
        args: Vec<Expr>,
    },

    Return {
        value: Expr,
    },

    /// Index write: `name[index] = value`
    IndexAssign {
        name: Ident,
        index: Expr,
        value: Expr,
    },

    /// `import "path/to/file.wilios"` as parsed in shallow mode (see
    /// `Parser::new_shallow`) — records the import syntactically without
    /// resolving or recursing into it. Never produced by the normal
    /// recursive `Parser::parse_import` path (which resolves and inlines
    /// the imported file's statements instead), so the interpreter's
    /// wildcard match arms cover it with no new runtime behavior.
    Import {
        path: String,
        line: usize,
        col: usize,
    },
}

#[derive(Debug, Clone)]
pub enum Expr {
    Int(isize),
    Float(f32),
    Bool(bool),
    /// `line`/`col` are the position of the identifier token itself, used
    /// only for diagnostics (see `wilios_core::resolve`); they are not
    /// significant to equality — see the hand-written `PartialEq` below.
    Var {
        name: Ident,
        line: usize,
        col: usize,
    },

    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },

    Binary {
        left: Box<Expr>,
        op: BinaryOp,
        right: Box<Expr>,
    },

    Func {
        params: Vec<Ident>,
        body: Vec<Stmt>,
    },

    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },

    Pitch(Pitch),

    Chord(Vec<Expr>),

    /// Array literal: `[expr, expr, ...]`
    Array(Vec<Expr>),

    /// Index read: `expr[expr]`
    Index {
        array: Box<Expr>,
        index: Box<Expr>,
    },
}

/// Hand-written rather than derived so that `Var`'s diagnostic-only
/// `line`/`col` fields never affect equality — every other arm reproduces
/// what `#[derive(PartialEq)]` would generate.
impl PartialEq for Expr {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Expr::Int(a), Expr::Int(b)) => a == b,
            (Expr::Float(a), Expr::Float(b)) => a == b,
            (Expr::Bool(a), Expr::Bool(b)) => a == b,
            (Expr::Var { name: a, .. }, Expr::Var { name: b, .. }) => a == b,
            (Expr::Unary { op: a_op, expr: a }, Expr::Unary { op: b_op, expr: b }) => {
                a_op == b_op && a == b
            }
            (
                Expr::Binary {
                    left: a_l,
                    op: a_op,
                    right: a_r,
                },
                Expr::Binary {
                    left: b_l,
                    op: b_op,
                    right: b_r,
                },
            ) => a_l == b_l && a_op == b_op && a_r == b_r,
            (
                Expr::Func {
                    params: a_p,
                    body: a_b,
                },
                Expr::Func {
                    params: b_p,
                    body: b_b,
                },
            ) => a_p == b_p && a_b == b_b,
            (
                Expr::Call {
                    callee: a_c,
                    args: a_a,
                },
                Expr::Call {
                    callee: b_c,
                    args: b_a,
                },
            ) => a_c == b_c && a_a == b_a,
            (Expr::Pitch(a), Expr::Pitch(b)) => a == b,
            (Expr::Chord(a), Expr::Chord(b)) => a == b,
            (Expr::Array(a), Expr::Array(b)) => a == b,
            (
                Expr::Index {
                    array: a_a,
                    index: a_i,
                },
                Expr::Index {
                    array: b_a,
                    index: b_i,
                },
            ) => a_a == b_a && a_i == b_i,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,

    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,

    And,
    Or,
}

impl From<Token> for UnaryOp {
    fn from(tok: Token) -> Self {
        match tok {
            Token::Minus => UnaryOp::Neg,
            Token::Not => UnaryOp::Not,
            _ => panic!("Invalid unary op: {:?}", tok),
        }
    }
}

impl From<Token> for BinaryOp {
    fn from(tok: Token) -> Self {
        match tok {
            Token::Plus => BinaryOp::Add,
            Token::Minus => BinaryOp::Sub,
            Token::Star => BinaryOp::Mul,
            Token::Slash => BinaryOp::Div,
            Token::Percent => BinaryOp::Mod,

            Token::Eq => BinaryOp::Eq,
            Token::NotEq => BinaryOp::NotEq,
            Token::Lt => BinaryOp::Lt,
            Token::LtEq => BinaryOp::LtEq,
            Token::Gt => BinaryOp::Gt,
            Token::GtEq => BinaryOp::GtEq,

            Token::And => BinaryOp::And,
            Token::Or => BinaryOp::Or,

            _ => panic!("Invalid binary op: {:?}", tok),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Ident(pub String);
