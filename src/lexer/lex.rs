use crate::lexer::{Spanned, Token};

#[derive(Debug, Clone, PartialEq)]
pub struct LexError {
    pub line: usize,
    pub col: usize,
    pub message: String,
}

impl std::fmt::Display for LexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[line {}, col {}] {}", self.line, self.col, self.message)
    }
}

#[derive(Clone)]
pub struct Lexer {
    input: Vec<char>,
    pos: usize,
    line: usize,
    col: usize,
}

impl Lexer {
    pub fn new(input: &str) -> Self {
        Self {
            input: input.chars().collect(),
            pos: 0,
            line: 1,
            col: 1,
        }
    }

    // ================= CORE STREAM HELPERS =================

    fn current(&self) -> Option<char> {
        self.input.get(self.pos).copied()
    }

    fn advance(&mut self) {
        if self.input.get(self.pos) == Some(&'\n') {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        self.pos += 1;
    }

    fn make_error(&self, message: impl Into<String>) -> LexError {
        LexError {
            line: self.line,
            col: self.col,
            message: message.into(),
        }
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.current(), Some(c) if c.is_whitespace() && c != '\n') {
            self.advance();
        }
    }

    fn read_while<F>(&mut self, cond: F) -> String
    where
        F: Fn(char) -> bool,
    {
        let mut s = String::new();
        while let Some(c) = self.current() {
            if cond(c) {
                s.push(c);
                self.advance();
            } else {
                break;
            }
        }
        s
    }

    // ================= MAIN LEX FUNCTION =================

    pub fn lex(&mut self) -> Result<Vec<Spanned<Token>>, LexError> {
        let mut tokens = Vec::new();

        while self.current().is_some() {
            self.skip_whitespace();

            let c = match self.current() {
                Some(c) => c,
                None => break,
            };

            let (tok_line, tok_col) = (self.line, self.col);

            macro_rules! push {
                ($tok:expr) => {
                    tokens.push(Spanned::new($tok, tok_line, tok_col))
                };
            }

            match c {
                '-' => {
                    self.advance();
                    if self.current() == Some('>') {
                        self.advance();
                        push!(Token::Arrow);
                    } else {
                        push!(Token::Minus);
                    }
                }
                '+' => {
                    self.advance();
                    push!(Token::Plus);
                }
                '{' => {
                    self.advance();
                    push!(Token::LBrace);
                }
                '}' => {
                    self.advance();
                    push!(Token::RBrace);
                }
                ',' => {
                    self.advance();
                    push!(Token::Comma);
                }
                '[' => {
                    self.advance();
                    push!(Token::LBracket);
                }
                ']' => {
                    self.advance();
                    push!(Token::RBracket);
                }
                '\n' => {
                    self.advance();
                    push!(Token::Newline);
                }
                '(' => {
                    self.advance();
                    push!(Token::LParenthesis);
                }
                ')' => {
                    self.advance();
                    push!(Token::RParenthesis);
                }

                // string literals
                '"' => {
                    self.advance(); // consume opening quote
                    let mut s = String::new();
                    loop {
                        match self.current() {
                            Some('"') => {
                                self.advance();
                                break;
                            }
                            Some('\\') => {
                                self.advance();
                                match self.current() {
                                    Some('n') => {
                                        s.push('\n');
                                        self.advance();
                                    }
                                    Some('t') => {
                                        s.push('\t');
                                        self.advance();
                                    }
                                    Some('\\') => {
                                        s.push('\\');
                                        self.advance();
                                    }
                                    Some('"') => {
                                        s.push('"');
                                        self.advance();
                                    }
                                    Some(c) => {
                                        return Err(
                                            self.make_error(format!("unknown escape: \\{}", c))
                                        );
                                    }
                                    None => return Err(self.make_error("unterminated string")),
                                }
                            }
                            Some(c) => {
                                s.push(c);
                                self.advance();
                            }
                            None => return Err(self.make_error("unterminated string literal")),
                        }
                    }
                    push!(Token::StringLit(s));
                }

                // comments //
                '/' => {
                    self.advance();
                    if self.current() == Some('/') {
                        self.advance();
                        self.read_while(|c| c != '\n');
                    } else {
                        push!(Token::Slash);
                    }
                }

                '=' => {
                    self.advance();
                    if self.current() == Some('=') {
                        self.advance();
                        push!(Token::Eq);
                    } else {
                        push!(Token::Assignment);
                    }
                }

                '>' => {
                    self.advance();
                    if self.current() == Some('=') {
                        self.advance();
                        push!(Token::GtEq);
                    } else {
                        push!(Token::Gt);
                    }
                }

                '<' => {
                    self.advance();
                    if self.current() == Some('=') {
                        self.advance();
                        push!(Token::LtEq);
                    } else {
                        push!(Token::Lt);
                    }
                }

                '!' => {
                    self.advance();
                    if self.current() == Some('=') {
                        self.advance();
                        push!(Token::NotEq);
                    } else {
                        push!(Token::Not);
                    }
                }

                '*' => {
                    self.advance();
                    push!(Token::Star);
                }

                '%' => {
                    self.advance();
                    push!(Token::Percent);
                }

                '&' => {
                    self.advance();
                    if self.current() == Some('&') {
                        self.advance();
                        push!(Token::And);
                    } else {
                        return Err(self.make_error("expected '&&', got single '&'"));
                    }
                }

                '|' => {
                    self.advance();
                    if self.current() == Some('|') {
                        self.advance();
                        push!(Token::Or);
                    } else {
                        return Err(self.make_error("expected '||', got single '|'"));
                    }
                }

                // numbers or durations (1, 1/4, 1/8.)
                c if c.is_ascii_digit() => {
                    let word = self.read_while(|c| c.is_ascii_digit() || c == '/' || c == '.');

                    if let Some(tok) = self.lex_duration(&word) {
                        push!(tok);
                    } else if word.contains('.') {
                        let f: f32 = word
                            .parse()
                            .map_err(|_| self.make_error(format!("invalid float: '{}'", word)))?;
                        push!(Token::Float(f));
                    } else {
                        let n = word
                            .parse()
                            .map_err(|_| self.make_error(format!("invalid integer: '{}'", word)))?;
                        push!(Token::Int(n));
                    }
                }

                // keywords / notes
                c if c.is_alphabetic() => {
                    let word = self
                        .read_while(|c| c.is_alphanumeric() || c == '#' || c == 'b' || c == '_');

                    if let Some(tok) = self.keyword(&word) {
                        push!(tok);
                    } else if let Some(tok) = self.lex_note(&word) {
                        push!(tok);
                    } else {
                        let mut wchars = word.chars();
                        let first = wchars.next().unwrap();
                        let rest_ok = wchars.all(|c| c.is_ascii_lowercase() || c == '_' || c.is_ascii_digit());
                        if !first.is_ascii_lowercase() || !rest_ok {
                            return Err(self.make_error(format!(
                                "invalid identifier '{}': names must start with [a-z] and may only contain [a-z_0-9]",
                                word
                            )));
                        }
                        push!(Token::Ident(word));
                    }
                }

                _ => return Err(self.make_error(format!("unexpected character: {:?}", c))),
            }
        }

        tokens.push(Spanned::new(Token::EOF, self.line, self.col));
        Ok(tokens)
    }

    // ================= YOUR ORIGINAL HELPERS =================

    fn keyword(&self, word: &str) -> Option<Token> {
        match word {
            "track" => Some(Token::Track),
            "global" => Some(Token::Global),
            "tempo" => Some(Token::Tempo),
            "volume" => Some(Token::Volume),
            "pan" => Some(Token::Pan),
            "rest" => Some(Token::Rest),
            "loop" => Some(Token::Loop),
            "if" => Some(Token::If),
            "else" => Some(Token::Else),
            "true" => Some(Token::Bool(true)),
            "false" => Some(Token::Bool(false)),
            "let" => Some(Token::Let),
            "func" => Some(Token::Func),
            "return" => Some(Token::Return),
            "wave" => Some(Token::Wave),
            "attack" => Some(Token::Attack),
            "decay" => Some(Token::Decay),
            "sustain" => Some(Token::Sustain),
            "release" => Some(Token::Release),
            "fm_ratio" => Some(Token::FmRatio),
            "fm_depth" => Some(Token::FmDepth),
            "swing" => Some(Token::Swing),
            "import" => Some(Token::Import),
            "fm" => Some(Token::Fm),
            "op" => Some(Token::Op),
            "algorithm" => Some(Token::Algorithm),
            "level" => Some(Token::Level),
            "ratio" => Some(Token::Ratio),
            "sine" => Some(Token::WaveSine),
            "square" => Some(Token::WaveSquare),
            "saw" => Some(Token::WaveSaw),
            "tri" => Some(Token::WaveTri),
            _ => None,
        }
    }

    fn lex_note(&self, s: &str) -> Option<Token> {
        if s.len() < 2 || s.len() > 3 {
            return None;
        }

        let mut chars = s.chars();
        let letter = chars.next()?;
        if !"ABCDEFG".contains(letter) {
            return None;
        }

        let (accidental, octave_char) = match chars.next() {
            Some('#') => (1, chars.next()?),
            Some('b') => (-1, chars.next()?),
            Some(c) if c.is_ascii_digit() => (0, c),
            _ => return None,
        };

        let octave = octave_char.to_digit(10)? as usize;

        Some(Token::Pitch {
            letter,
            accidental,
            octave,
        })
    }

    fn lex_duration(&self, s: &str) -> Option<Token> {
        let dotted = s.ends_with('.');
        let s = if dotted { &s[..s.len() - 1] } else { s };

        let mut parts = s.split('/');
        let beats = parts.next()?.parse().ok()?;
        let division = parts.next()?.parse().ok()?;

        Some(Token::Duration {
            beats,
            division,
            dotted,
        })
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
