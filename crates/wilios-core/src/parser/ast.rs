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
    Track(usize),
    Global,
    Pan(isize),
    Volume(usize),

    // synth
    Wave(Waveform),
    Attack(Expr),
    Decay(Expr),
    Sustain(Expr),
    Release(Expr),
    FmRatio(Expr),
    FmDepth(Expr),
    Swing(Expr),
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
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Int(isize),
    Float(f32),
    Bool(bool),
    Var(Ident),

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
