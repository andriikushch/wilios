use std::collections::HashSet;
use std::path::PathBuf;

use crate::{
    lexer::{Lexer, Spanned, Token},
    parser::ast::{Duration, Expr, FmOperator, Ident, Pitch, Stmt, UnaryOp, Waveform},
};

#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub line: usize,
    pub col: usize,
    pub message: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[line {}, col {}] {}", self.line, self.col, self.message)
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct TrackAst {
    pub id: usize,
    pub statements: Vec<Stmt>,
}

#[derive(Debug, PartialEq)]
pub struct Program {
    pub global_stmts: Vec<Stmt>,
    pub tracks: Vec<TrackAst>,
}

pub struct Parser {
    tokens: std::vec::IntoIter<Spanned<Token>>,
    current: Option<Spanned<Token>>,
    peek: Option<Spanned<Token>>,
    current_scope: Option<usize>,
    last_pos: (usize, usize),
    base_dir: Option<PathBuf>,
    loaded: HashSet<PathBuf>,
}

impl Parser {
    pub fn new(tokens: Vec<Spanned<Token>>) -> Self {
        Self::new_with_context(tokens, None, HashSet::new())
    }

    pub fn new_with_context(
        tokens: Vec<Spanned<Token>>,
        base_dir: Option<PathBuf>,
        loaded: HashSet<PathBuf>,
    ) -> Self {
        let mut iter = tokens.into_iter();
        let current = iter.next();
        let peek = iter.next();

        Self {
            tokens: iter,
            current,
            peek,
            current_scope: None,
            last_pos: (1, 1),
            base_dir,
            loaded,
        }
    }

    fn next(&mut self) {
        if let Some(ref s) = self.current {
            self.last_pos = (s.line, s.col);
        }
        self.current = self.peek.take();
        self.peek = self.tokens.next();
    }

    fn current_token(&self) -> Option<&Token> {
        self.current.as_ref().map(|s| &s.token)
    }

    fn peek_token(&self) -> Option<&Token> {
        self.peek.as_ref().map(|s| &s.token)
    }

    fn current_pos(&self) -> (usize, usize) {
        match &self.current {
            Some(s) => (s.line, s.col),
            None => self.last_pos,
        }
    }

    fn make_error(&self, message: impl Into<String>) -> ParseError {
        let (line, col) = self.current_pos();
        ParseError {
            line,
            col,
            message: message.into(),
        }
    }

    fn expect(&mut self, expected: Token) -> Result<(), ParseError> {
        match self.current_token() {
            Some(actual) if std::mem::discriminant(actual) == std::mem::discriminant(&expected) => {
                self.next();
                Ok(())
            }
            Some(actual) => {
                Err(self.make_error(format!("expected {:?}, found {:?}", expected, actual)))
            }
            None => {
                Err(self.make_error(format!("expected {:?}, but reached end of input", expected)))
            }
        }
    }

    fn parse_ident(&mut self) -> Result<Ident, ParseError> {
        match self.current_token() {
            Some(Token::Ident(s)) => {
                let s = s.clone();
                self.next();
                Ok(Ident(s))
            }
            _ => Err(self.make_error("expected identifier")),
        }
    }

    // =========================================================
    // ENTRY
    // =========================================================

    pub fn parse(&mut self) -> Result<Program, ParseError> {
        let mut program = Program {
            global_stmts: vec![],
            tracks: vec![],
        };

        while self.current_token() != Some(&Token::EOF) {
            // Handle import at top level
            if self.current_token() == Some(&Token::Import) {
                let imported = self.parse_import()?;
                program.global_stmts.extend(imported.global_stmts);
                for track in imported.tracks {
                    if let Some(t) = program.tracks.iter_mut().find(|t| t.id == track.id) {
                        t.statements.extend(track.statements);
                    } else {
                        program.tracks.push(track);
                    }
                }
                // skip trailing newline after import
                while self.current_token() == Some(&Token::Newline) {
                    self.next();
                }
                continue;
            }

            match self.parse_statement()? {
                Some(stmt) => match stmt {
                    Stmt::Track(id) => self.current_scope = Some(id),
                    Stmt::Global => self.current_scope = None,
                    _ => match self.current_scope {
                        None => program.global_stmts.push(stmt),
                        Some(track_id) => {
                            let track = program.tracks.iter_mut().find(|t| t.id == track_id);

                            if let Some(t) = track {
                                t.statements.push(stmt);
                            } else {
                                program.tracks.push(TrackAst {
                                    id: track_id,
                                    statements: vec![stmt],
                                });
                            }
                        }
                    },
                },
                None => {
                    self.next();
                }
            }
        }

        Ok(program)
    }

    // =========================================================
    // STATEMENTS
    // =========================================================

    fn parse_statement(&mut self) -> Result<Option<Stmt>, ParseError> {
        match self.current_token() {
            None => Ok(None),
            Some(Token::Rest) => self.parse_rest().map(Some),
            Some(Token::Lt) => self.parse_chord().map(Some),

            Some(Token::Track) => self.parse_track().map(Some),
            Some(Token::Global) => self.parse_global().map(Some),
            Some(Token::Tempo) => self.parse_tempo().map(Some),
            Some(Token::Pan) => self.parse_pan().map(Some),
            Some(Token::Volume) => self.parse_volume().map(Some),

            Some(Token::Wave) => self.parse_wave().map(Some),
            Some(Token::Attack) => self.parse_adsr_param(Token::Attack).map(Some),
            Some(Token::Decay) => self.parse_adsr_param(Token::Decay).map(Some),
            Some(Token::Sustain) => self.parse_adsr_param(Token::Sustain).map(Some),
            Some(Token::Release) => self.parse_adsr_param(Token::Release).map(Some),
            Some(Token::FmRatio) => self.parse_fm_param(Token::FmRatio).map(Some),
            Some(Token::FmDepth) => self.parse_fm_param(Token::FmDepth).map(Some),
            Some(Token::Swing) => self.parse_swing().map(Some),
            Some(Token::Fm) => self.parse_fm_block().map(Some),

            Some(Token::Loop) => self.parse_loop().map(Some),
            Some(Token::If) => self.parse_if().map(Some),
            Some(Token::Let) => self.parse_let().map(Some),
            Some(Token::Return) => self.parse_return().map(Some),
            Some(Token::Ident(_)) => {
                if self.peek_token() == Some(&Token::Assignment) {
                    self.parse_assignment().map(Some)
                } else if self.peek_token() == Some(&Token::LParenthesis) {
                    self.parse_call_stmt().map(Some)
                } else if self.peek_token() == Some(&Token::LBracket) {
                    self.parse_index_assign().map(Some)
                } else {
                    Err(self.make_error(format!(
                        "unexpected token after identifier: {:?}",
                        self.peek_token()
                    )))
                }
            }

            Some(Token::Newline) | Some(Token::RBrace) | Some(Token::EOF) => Ok(None),

            _ => Err(self.make_error(format!("unexpected token: {:?}", self.current_token()))),
        }
    }

    // =========================================================
    // SIMPLE STATEMENTS
    // =========================================================

    fn parse_track(&mut self) -> Result<Stmt, ParseError> {
        self.next();
        match self.current_token() {
            Some(Token::Int(v)) => {
                let id = *v as usize;
                self.next();
                Ok(Stmt::Track(id))
            }
            _ => Err(self.make_error("expected integer after `track`")),
        }
    }

    fn parse_global(&mut self) -> Result<Stmt, ParseError> {
        self.next();
        Ok(Stmt::Global)
    }

    fn parse_tempo(&mut self) -> Result<Stmt, ParseError> {
        self.next();
        match self.current_token() {
            Some(Token::Int(v)) => {
                let value = *v as usize;
                self.next();
                Ok(Stmt::Tempo(value))
            }
            _ => Err(self.make_error("expected integer after `tempo`")),
        }
    }

    fn parse_pan(&mut self) -> Result<Stmt, ParseError> {
        self.next();

        let sign = if self.current_token() == Some(&Token::Minus) {
            self.next();
            -1
        } else {
            1
        };

        match self.current_token() {
            Some(Token::Int(v)) => {
                let value = *v;
                self.next();
                Ok(Stmt::Pan(value * sign))
            }
            _ => Err(self.make_error("expected integer after `pan`")),
        }
    }

    fn parse_volume(&mut self) -> Result<Stmt, ParseError> {
        self.next();
        match self.current_token() {
            Some(Token::Int(v)) => {
                let value = *v as usize;
                self.next();
                Ok(Stmt::Volume(value))
            }
            _ => Err(self.make_error("expected integer after `volume`")),
        }
    }

    // =========================================================
    // LOOP / IF / BLOCK
    // =========================================================

    fn parse_block(&mut self) -> Result<Vec<Stmt>, ParseError> {
        let mut body = Vec::new();

        while self.current_token() != Some(&Token::RBrace)
            && self.current_token().is_some()
            && self.current_token() != Some(&Token::EOF)
        {
            if self.current_token() == Some(&Token::Newline) {
                self.next();
                continue;
            }

            match self.parse_statement()? {
                Some(stmt) => body.push(stmt),
                None => {
                    self.next();
                }
            }
        }

        if self.current_token() == Some(&Token::RBrace) {
            self.next();
        } else {
            return Err(self.make_error("expected '}' at end of block"));
        }

        Ok(body)
    }

    fn parse_loop(&mut self) -> Result<Stmt, ParseError> {
        self.next(); // consume `loop`
        self.expect(Token::LParenthesis)?;
        let condition = self.parse_expr(0)?;
        self.expect(Token::RParenthesis)?;
        self.expect(Token::LBrace)?;
        let body = self.parse_block()?;
        Ok(Stmt::Loop { condition, body })
    }

    fn parse_if(&mut self) -> Result<Stmt, ParseError> {
        self.next(); // consume `if`
        self.expect(Token::LParenthesis)?;
        let condition = self.parse_expr(0)?;
        self.expect(Token::RParenthesis)?;
        self.expect(Token::LBrace)?;
        let then_body = self.parse_block()?;
        // skip newlines before optional `else`
        while self.current_token() == Some(&Token::Newline) {
            self.next();
        }
        let else_body = if self.current_token() == Some(&Token::Else) {
            self.next(); // consume `else`
            self.expect(Token::LBrace)?;
            Some(self.parse_block()?)
        } else {
            None
        };
        Ok(Stmt::If {
            condition,
            then_body,
            else_body,
        })
    }

    fn parse_func_body(&mut self) -> Result<(Vec<Ident>, Vec<Stmt>), ParseError> {
        self.expect(Token::LParenthesis)?;
        let mut params = Vec::new();
        while self.current_token() != Some(&Token::RParenthesis) {
            params.push(self.parse_ident()?);
            if self.current_token() == Some(&Token::Comma) {
                self.next();
            }
        }
        self.expect(Token::RParenthesis)?;
        self.expect(Token::LBrace)?;
        let body = self.parse_block()?;
        Ok((params, body))
    }

    fn parse_arg_list(&mut self) -> Result<Vec<Expr>, ParseError> {
        self.expect(Token::LParenthesis)?;
        let mut args = Vec::new();
        while self.current_token() != Some(&Token::RParenthesis) {
            args.push(self.parse_expr(0)?);
            if self.current_token() == Some(&Token::Comma) {
                self.next();
            }
        }
        self.expect(Token::RParenthesis)?;
        Ok(args)
    }

    fn parse_call_stmt(&mut self) -> Result<Stmt, ParseError> {
        let name = self.parse_ident()?;
        let args = self.parse_arg_list()?;
        Ok(Stmt::Call {
            callee: Expr::Var(name),
            args,
        })
    }

    fn parse_return(&mut self) -> Result<Stmt, ParseError> {
        self.next(); // consume `return`
        let value = self.parse_expr(0)?;
        Ok(Stmt::Return { value })
    }

    fn parse_import(&mut self) -> Result<Program, ParseError> {
        self.next(); // consume `import`

        let path_str = match self.current_token() {
            Some(Token::StringLit(s)) => s.clone(),
            _ => return Err(self.make_error("expected string literal after `import`")),
        };
        self.next(); // consume string literal

        // Only .wilios files may be imported
        let has_wilios_ext = std::path::Path::new(&path_str)
            .extension()
            .is_some_and(|ext| ext == "wilios");
        if !has_wilios_ext {
            return Err(self.make_error(format!(
                "cannot resolve import '{}': imports must be .wilios files",
                path_str
            )));
        }

        // Import paths must be relative — absolute paths would let a script
        // reach any file on disk regardless of base_dir.
        if PathBuf::from(&path_str).is_absolute() {
            return Err(self.make_error(format!(
                "cannot resolve import '{}': import paths must be relative",
                path_str
            )));
        }

        // Resolve path relative to base_dir
        let path = if let Some(ref base) = self.base_dir {
            base.join(&path_str)
        } else {
            PathBuf::from(&path_str)
        };

        let canonical = path
            .canonicalize()
            .map_err(|e| self.make_error(format!("cannot resolve import '{}': {}", path_str, e)))?;

        // Confine resolved imports to the project directory (the process's
        // current working directory) so `..` traversal can't escape it.
        let project_root = std::env::current_dir()
            .and_then(|dir| dir.canonicalize())
            .map_err(|e| self.make_error(format!("cannot resolve import '{}': {}", path_str, e)))?;
        if !canonical.starts_with(&project_root) {
            return Err(self.make_error(format!(
                "cannot resolve import '{}': import escapes the project directory",
                path_str
            )));
        }

        // Skip already-loaded files (circular import protection)
        if self.loaded.contains(&canonical) {
            return Ok(Program {
                global_stmts: vec![],
                tracks: vec![],
            });
        }
        self.loaded.insert(canonical.clone());

        let source = std::fs::read_to_string(&canonical)
            .map_err(|e| self.make_error(format!("cannot read import '{}': {}", path_str, e)))?;

        let tokens = Lexer::new(&source)
            .lex()
            .map_err(|e| self.make_error(format!("lex error in '{}': {}", path_str, e)))?;

        let imported_base = canonical.parent().map(|p| p.to_path_buf());
        let imported = Parser::new_with_context(tokens, imported_base, self.loaded.clone())
            .parse()
            .map_err(|e| self.make_error(format!("parse error in '{}': {}", path_str, e)))?;

        Ok(imported)
    }

    // =========================================================
    // VARIABLES
    // =========================================================

    fn parse_let(&mut self) -> Result<Stmt, ParseError> {
        self.next();
        let name = self.parse_ident()?;
        self.expect(Token::Assignment)?;
        let value = self.parse_expr(0)?;
        Ok(Stmt::Let { name, value })
    }

    fn parse_assignment(&mut self) -> Result<Stmt, ParseError> {
        let name = self.parse_ident()?;
        self.expect(Token::Assignment)?;
        let value = self.parse_expr(0)?;
        Ok(Stmt::Assign { name, value })
    }

    fn parse_index_assign(&mut self) -> Result<Stmt, ParseError> {
        let name = self.parse_ident()?;
        self.expect(Token::LBracket)?;
        let index = self.parse_expr(0)?;
        self.expect(Token::RBracket)?;
        self.expect(Token::Assignment)?;
        let value = self.parse_expr(0)?;
        Ok(Stmt::IndexAssign { name, index, value })
    }

    // =========================================================
    // PRATT EXPRESSION PARSER
    // =========================================================

    fn parse_expr(&mut self, min_bp: u8) -> Result<Expr, ParseError> {
        let mut lhs = self.nud()?;

        while let Some(op_token) = self.current_token().cloned() {
            let (l_bp, r_bp) = match Self::binding_power(&op_token) {
                Some(bp) => bp,
                None => break,
            };

            if l_bp < min_bp {
                break;
            }

            if op_token == Token::LParenthesis {
                let args = self.parse_arg_list()?;
                lhs = Expr::Call {
                    callee: Box::new(lhs),
                    args,
                };
                continue;
            }

            if op_token == Token::LBracket {
                self.next(); // consume `[`
                let index = self.parse_expr(0)?;
                self.expect(Token::RBracket)?;
                lhs = Expr::Index {
                    array: Box::new(lhs),
                    index: Box::new(index),
                };
                continue;
            }

            self.next(); // consume operator

            let rhs = self.parse_expr(r_bp)?;

            lhs = Expr::Binary {
                left: Box::new(lhs),
                op: op_token.into(),
                right: Box::new(rhs),
            };
        }

        Ok(lhs)
    }

    fn nud(&mut self) -> Result<Expr, ParseError> {
        let tok = match self.current_token().cloned() {
            Some(t) => t,
            None => return Err(self.make_error("unexpected end of input in expression")),
        };
        self.next();

        match tok {
            Token::Int(n) => Ok(Expr::Int(n)),
            Token::Float(f) => Ok(Expr::Float(f)),
            Token::Bool(b) => Ok(Expr::Bool(b)),
            Token::Ident(s) => Ok(Expr::Var(Ident(s))),

            Token::Pitch {
                letter,
                accidental,
                octave,
            } => Ok(Expr::Pitch(Pitch {
                letter,
                accidental,
                octave,
            })),

            Token::Lt => {
                let mut pitches: Vec<Expr> = Vec::new();
                loop {
                    pitches.push(self.parse_expr(9)?);
                    match self.current_token() {
                        Some(Token::Comma) => self.next(),
                        Some(Token::Gt) => break,
                        _ => return Err(self.make_error("expected ',' or '>' in chord expression")),
                    }
                }
                self.next(); // consume `>`
                Ok(Expr::Chord(pitches))
            }

            Token::Minus => {
                let rhs = self.parse_expr(0)?;
                Ok(Expr::Unary {
                    op: UnaryOp::Neg,
                    expr: Box::new(rhs),
                })
            }
            Token::Plus => {
                let rhs = self.parse_expr(0)?;
                Ok(rhs)
            }

            Token::LParenthesis => {
                let expr = self.parse_expr(0)?;
                self.expect(Token::RParenthesis)?;
                Ok(expr)
            }

            Token::Func => {
                let (params, body) = self.parse_func_body()?;
                Ok(Expr::Func { params, body })
            }

            Token::LBracket => {
                let mut elements = Vec::new();
                while self.current_token() != Some(&Token::RBracket) {
                    elements.push(self.parse_expr(0)?);
                    if self.current_token() == Some(&Token::Comma) {
                        self.next();
                    }
                }
                self.expect(Token::RBracket)?;
                Ok(Expr::Array(elements))
            }

            _ => Err(self.make_error(format!("unexpected token in expression: {:?}", tok))),
        }
    }

    fn binding_power(tok: &Token) -> Option<(u8, u8)> {
        match tok {
            Token::Or => Some((1, 2)),
            Token::And => Some((3, 4)),
            Token::Eq | Token::NotEq => Some((5, 6)),
            Token::Lt | Token::LtEq | Token::Gt | Token::GtEq => Some((7, 8)),
            Token::Plus | Token::Minus => Some((9, 10)),
            Token::Star | Token::Slash | Token::Percent => Some((11, 12)),
            Token::LParenthesis => Some((13, 0)), // function call (postfix)
            Token::LBracket => Some((13, 0)),     // index (postfix)
            _ => None,
        }
    }

    // =========================================================
    // SYNTH PARAMETERS
    // =========================================================

    fn parse_wave(&mut self) -> Result<Stmt, ParseError> {
        self.next(); // consume `wave`
        match self.current_token().cloned() {
            Some(Token::WaveSine) => {
                self.next();
                Ok(Stmt::Wave(Waveform::Sine))
            }
            Some(Token::WaveSquare) => {
                self.next();
                Ok(Stmt::Wave(Waveform::Square))
            }
            Some(Token::WaveSaw) => {
                self.next();
                Ok(Stmt::Wave(Waveform::Saw))
            }
            Some(Token::WaveTri) => {
                self.next();
                Ok(Stmt::Wave(Waveform::Triangle))
            }
            _ => Err(self.make_error("expected waveform name: sine, square, saw, or tri")),
        }
    }

    fn parse_adsr_param(&mut self, keyword: Token) -> Result<Stmt, ParseError> {
        self.next(); // consume keyword
        let value = self.parse_expr(0)?;
        match keyword {
            Token::Attack => Ok(Stmt::Attack(value)),
            Token::Decay => Ok(Stmt::Decay(value)),
            Token::Sustain => Ok(Stmt::Sustain(value)),
            Token::Release => Ok(Stmt::Release(value)),
            _ => unreachable!(),
        }
    }

    fn parse_fm_param(&mut self, keyword: Token) -> Result<Stmt, ParseError> {
        self.next(); // consume keyword
        let value = match self.current_token().cloned() {
            Some(Token::Float(f)) => {
                self.next();
                Expr::Float(f)
            }
            Some(Token::Int(n)) => {
                self.next();
                Expr::Float(n as f32)
            }
            _ => return Err(self.make_error("expected numeric value")),
        };
        match keyword {
            Token::FmRatio => Ok(Stmt::FmRatio(value)),
            Token::FmDepth => Ok(Stmt::FmDepth(value)),
            _ => unreachable!(),
        }
    }

    fn parse_swing(&mut self) -> Result<Stmt, ParseError> {
        self.next(); // consume `swing`
        let value = match self.current_token().cloned() {
            Some(Token::Float(f)) => {
                self.next();
                Expr::Float(f)
            }
            Some(Token::Int(n)) => {
                self.next();
                Expr::Int(n)
            }
            _ => return Err(self.make_error("expected numeric value after `swing`")),
        };
        Ok(Stmt::Swing(value))
    }

    fn parse_fm_block(&mut self) -> Result<Stmt, ParseError> {
        self.next(); // consume `fm`
        self.expect(Token::LBrace)?;
        // skip leading newlines
        while self.current_token() == Some(&Token::Newline) {
            self.next();
        }

        let mut ops: Vec<FmOperator> = Vec::new();
        let mut algorithm: Vec<(usize, usize)> = Vec::new();

        while self.current_token() != Some(&Token::RBrace)
            && self.current_token() != Some(&Token::EOF)
        {
            match self.current_token() {
                Some(Token::Newline) => {
                    self.next();
                }
                Some(Token::Algorithm) => {
                    algorithm = self.parse_fm_algorithm()?;
                }
                Some(Token::Op) => {
                    ops.push(self.parse_fm_op()?);
                }
                _ => {
                    return Err(self.make_error(format!(
                        "unexpected token inside fm block: {:?}",
                        self.current_token()
                    )));
                }
            }
        }

        self.expect(Token::RBrace)?;
        Ok(Stmt::FmBlock { ops, algorithm })
    }

    fn parse_fm_algorithm(&mut self) -> Result<Vec<(usize, usize)>, ParseError> {
        self.next(); // consume `algorithm`
        self.expect(Token::LBracket)?;
        let mut pairs: Vec<(usize, usize)> = Vec::new();

        while self.current_token() != Some(&Token::RBracket)
            && self.current_token() != Some(&Token::EOF)
        {
            let src = match self.current_token() {
                Some(Token::Int(n)) => {
                    let v = *n as usize;
                    self.next();
                    v
                }
                _ => return Err(self.make_error("expected operator id (integer) in algorithm")),
            };
            self.expect(Token::Arrow)?;
            let dst = match self.current_token() {
                Some(Token::Int(n)) => {
                    let v = *n as usize;
                    self.next();
                    v
                }
                _ => {
                    return Err(
                        self.make_error("expected operator id (integer) after -> in algorithm")
                    );
                }
            };
            pairs.push((src, dst));
            if self.current_token() == Some(&Token::Comma) {
                self.next();
            }
        }

        self.expect(Token::RBracket)?;
        Ok(pairs)
    }

    fn parse_fm_op(&mut self) -> Result<FmOperator, ParseError> {
        self.next(); // consume `op`
        let id = match self.current_token() {
            Some(Token::Int(n)) => {
                let v = *n as usize;
                self.next();
                v
            }
            _ => return Err(self.make_error("expected operator id (integer) after `op`")),
        };
        self.expect(Token::LBrace)?;
        // skip leading newlines
        while self.current_token() == Some(&Token::Newline) {
            self.next();
        }

        let mut ratio: Option<Expr> = None;
        let mut level: Option<Expr> = None;
        let mut wave: Option<Waveform> = None;
        let mut attack_ms: Option<Expr> = None;
        let mut decay_ms: Option<Expr> = None;
        let mut sustain_level: Option<Expr> = None;
        let mut release_ms: Option<Expr> = None;

        while self.current_token() != Some(&Token::RBrace)
            && self.current_token() != Some(&Token::EOF)
        {
            match self.current_token().cloned() {
                Some(Token::Newline) => {
                    self.next();
                }
                Some(Token::Ratio) => {
                    self.next();
                    ratio = Some(self.parse_expr(0)?);
                }
                Some(Token::Level) => {
                    self.next();
                    level = Some(self.parse_expr(0)?);
                }
                Some(Token::Wave) => {
                    self.next();
                    wave = Some(match self.current_token().cloned() {
                        Some(Token::WaveSine) => {
                            self.next();
                            Waveform::Sine
                        }
                        Some(Token::WaveSquare) => {
                            self.next();
                            Waveform::Square
                        }
                        Some(Token::WaveSaw) => {
                            self.next();
                            Waveform::Saw
                        }
                        Some(Token::WaveTri) => {
                            self.next();
                            Waveform::Triangle
                        }
                        _ => return Err(self.make_error("expected waveform name inside op block")),
                    });
                }
                Some(Token::Attack) => {
                    self.next();
                    attack_ms = Some(self.parse_expr(0)?);
                }
                Some(Token::Decay) => {
                    self.next();
                    decay_ms = Some(self.parse_expr(0)?);
                }
                Some(Token::Sustain) => {
                    self.next();
                    sustain_level = Some(self.parse_expr(0)?);
                }
                Some(Token::Release) => {
                    self.next();
                    release_ms = Some(self.parse_expr(0)?);
                }
                _ => {
                    return Err(self.make_error(format!(
                        "unexpected token inside op block: {:?}",
                        self.current_token()
                    )));
                }
            }
        }

        self.expect(Token::RBrace)?;
        Ok(FmOperator {
            id,
            ratio: ratio.unwrap_or(Expr::Float(1.0)),
            level: level.unwrap_or(Expr::Float(1.0)),
            wave,
            attack_ms,
            decay_ms,
            sustain_level,
            release_ms,
        })
    }

    // =========================================================
    // MUSIC
    // =========================================================

    fn parse_rest(&mut self) -> Result<Stmt, ParseError> {
        self.next();
        let duration = self.parse_duration()?;
        Ok(Stmt::Rest { duration })
    }

    fn parse_chord(&mut self) -> Result<Stmt, ParseError> {
        self.next(); // consume `<`

        let mut pitches: Vec<Expr> = Vec::new();
        loop {
            pitches.push(self.parse_expr(9)?);

            match self.current_token() {
                Some(Token::Comma) => self.next(),
                Some(Token::Gt) => break,
                _ => return Err(self.make_error("expected ',' or '>' in chord")),
            }
        }

        self.next(); // consume `>`
        let duration = self.parse_duration()?;
        Ok(Stmt::Chord { pitches, duration })
    }

    fn parse_duration(&mut self) -> Result<Duration, ParseError> {
        // Path A: numeric literal like "1/4" or "1/4."
        if let Some(Token::Duration {
            beats,
            division,
            dotted,
        }) = self.current_token()
        {
            let d = Duration {
                beats: Expr::Int(*beats),
                division: Expr::Int(*division),
                dotted: *dotted,
            };
            self.next();
            return Ok(d);
        }

        // Path B: <ident_or_int> / <ident_or_int>
        let beats = match self.current_token() {
            Some(Token::Ident(name)) => {
                let e = Expr::Var(Ident(name.clone()));
                self.next();
                e
            }
            Some(Token::Int(n)) => {
                let e = Expr::Int(*n);
                self.next();
                e
            }
            _ => return Err(self.make_error("expected duration (e.g. 1/4)")),
        };
        if self.current_token() != Some(&Token::Slash) {
            return Err(self.make_error("expected '/' in duration"));
        }
        self.next(); // consume /
        let division = match self.current_token() {
            Some(Token::Ident(name)) => {
                let e = Expr::Var(Ident(name.clone()));
                self.next();
                e
            }
            Some(Token::Int(n)) => {
                let e = Expr::Int(*n);
                self.next();
                e
            }
            _ => {
                return Err(self.make_error("expected integer or identifier after '/' in duration"));
            }
        };
        Ok(Duration {
            beats,
            division,
            dotted: false,
        })
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
