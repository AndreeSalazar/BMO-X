//! Python Lexer — minimal, indent-significant.

#![allow(dead_code)]

extern crate alloc;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;

use crate::bmo_core::barex::BxResult;

/// Python token.
#[derive(Debug, Clone, PartialEq)]
pub enum PyToken {
    // Literals
    IntLit(i64),
    FloatLit(u64),  // bits
    StrLit(String),
    Name(String),
    // Keywords
    Def, Class, If, Elif, Else, While, For, In, Return, Break, Continue,
    Pass, Import, From, As, Try, Except, Finally, With, Lambda, And, Or, Not,
    Is, None, True, False, Global, Nonlocal, Yield, Assert, Raise,
    // Operators
    Plus, Minus, Star, Slash, Percent, DblSlash, DblStar,
    Eq, EqEq, NotEq, Lt, Gt, Le, Ge,
    Amp, Pipe, Caret, Tilde, Shl, Shr,
    // Assignment
    Assign, PlusEq, MinusEq, StarEq, SlashEq, PercentEq,
    AmpEq, PipeEq, CaretEq, ShlEq, ShrEq, DblStarEq,
    // Delimiters
    LParen, RParen, LBracket, RBracket, LBrace, RBrace,
    Comma, Colon, Semi, Dot, At, Arrow,
    // Indentation
    Indent,
    Dedent,
    Newline,
    // Special
    Eof,
}

/// Python Lexer.
pub struct PyLexer {
    src: Vec<u8>,
    pos: usize,
    /// Stack of indentation levels.
    indent_stack: Vec<usize>,
    /// Pending DEDENT count to emit before next token.
    pending_dedents: usize,
    /// Whether to emit a NEWLINE before the next statement.
    at_line_start: bool,
    tokens: Vec<PyToken>,
}

impl PyLexer {
    pub fn new(source: &[u8]) -> Self {
        Self {
            src: source.to_vec(),
            pos: 0,
            indent_stack: vec![0],
            pending_dedents: 0,
            at_line_start: true,
            tokens: Vec::new(),
        }
    }

    pub fn tokenize(&mut self) -> BxResult<Vec<PyToken>> {
        while self.pos < self.src.len() {
            if self.at_line_start {
                self.handle_indent()?;
                self.at_line_start = false;
            }
            self.skip_inline_whitespace();
            if self.pos >= self.src.len() { break; }
            let b = self.peek();
            match b {
                b'\n' => {
                    self.advance();
                    if !self.tokens.is_empty() && !matches!(self.tokens.last(), Some(PyToken::Newline) | Some(PyToken::Indent) | Some(PyToken::Dedent)) {
                        self.tokens.push(PyToken::Newline);
                    }
                    self.at_line_start = true;
                }
                b'#' => {
                    while self.pos < self.src.len() && self.peek() != b'\n' { self.advance(); }
                }
                b'(' | b')' | b'[' | b']' | b'{' | b'}' | b',' | b':' | b';' | b'@' => {
                    let t = self.single_char_punct();
                    self.tokens.push(t);
                }
                b'.' => {
                    if self.pos + 1 < self.src.len() && self.src[self.pos + 1].is_ascii_digit() {
                        self.read_number()?;
                    } else {
                        self.advance();
                        self.tokens.push(PyToken::Dot);
                    }
                }
                b'"' | b'\'' => {
                    self.read_string(b)?;
                }
                b'+' | b'-' | b'*' | b'/' | b'%' | b'&' | b'|' | b'^' | b'~' | b'<' | b'>' | b'=' | b'!' => {
                    self.read_operator()?;
                }
                b'\r' => { self.advance(); }
                _ if b.is_ascii_digit() => { self.read_number()?; }
                _ if b.is_ascii_alphabetic() || b == b'_' => { self.read_name_or_keyword()?; }
                _ => { self.advance(); } // skip unknown
            }
        }
        // Emit remaining DEDENTs and final NEWLINE.
        while self.indent_stack.len() > 1 {
            self.indent_stack.pop();
            self.tokens.push(PyToken::Dedent);
        }
        if !matches!(self.tokens.last(), Some(PyToken::Newline)) {
            self.tokens.push(PyToken::Newline);
        }
        self.tokens.push(PyToken::Eof);
        Ok(self.tokens.clone())
    }

    fn handle_indent(&mut self) -> BxResult<()> {
        let mut spaces = 0;
        while self.pos < self.src.len() && self.peek() == b' ' { spaces += 1; self.advance(); }
        // Skip blank/comment lines
        if self.pos < self.src.len() && (self.peek() == b'\n' || self.peek() == b'#') {
            return Ok(());
        }
        let current = *self.indent_stack.last().unwrap();
        if spaces > current {
            self.indent_stack.push(spaces);
            self.tokens.push(PyToken::Indent);
        } else if spaces < current {
            while *self.indent_stack.last().unwrap() > spaces {
                self.indent_stack.pop();
                self.tokens.push(PyToken::Dedent);
            }
        }
        Ok(())
    }

    fn skip_inline_whitespace(&mut self) {
        while self.pos < self.src.len() && (self.peek() == b' ' || self.peek() == b'\t') {
            self.advance();
        }
    }

    fn peek(&self) -> u8 {
        if self.pos < self.src.len() { self.src[self.pos] } else { 0 }
    }

    fn advance(&mut self) -> u8 {
        let b = self.peek();
        if self.pos < self.src.len() { self.pos += 1; }
        b
    }

    fn single_char_punct(&mut self) -> PyToken {
        let b = self.advance();
        match b {
            b'(' => PyToken::LParen,
            b')' => PyToken::RParen,
            b'[' => PyToken::LBracket,
            b']' => PyToken::RBracket,
            b'{' => PyToken::LBrace,
            b'}' => PyToken::RBrace,
            b',' => PyToken::Comma,
            b':' => PyToken::Colon,
            b';' => PyToken::Semi,
            b'@' => PyToken::At,
            _ => PyToken::Eof,
        }
    }

    fn read_string(&mut self, quote: u8) -> BxResult<()> {
        let _ = self.advance(); // opening quote
        let mut s = String::new();
        while self.pos < self.src.len() && self.peek() != quote {
            let b = self.advance();
            if b == b'\\' && self.pos < self.src.len() {
                let esc = self.advance();
                match esc {
                    b'n' => s.push('\n'),
                    b't' => s.push('\t'),
                    b'r' => s.push('\r'),
                    b'\\' => s.push('\\'),
                    b'\'' => s.push('\''),
                    b'"' => s.push('"'),
                    b'0' => s.push('\0'),
                    _ => s.push(esc as char),
                }
            } else {
                s.push(b as char);
            }
        }
        if self.pos < self.src.len() { let _ = self.advance(); } // closing quote
        self.tokens.push(PyToken::StrLit(s));
        Ok(())
    }

    fn read_number(&mut self) -> BxResult<()> {
        let start = self.pos;
        while self.pos < self.src.len() && (self.src[self.pos].is_ascii_digit() || self.src[self.pos] == b'_') {
            self.pos += 1;
        }
        let is_float = self.pos < self.src.len() && self.src[self.pos] == b'.';
        if is_float { self.pos += 1; }
        while self.pos < self.src.len() && (self.src[self.pos].is_ascii_digit() || self.src[self.pos] == b'_') {
            self.pos += 1;
        }
        let s: Vec<u8> = self.src[start..self.pos].iter().filter(|&&b| b != b'_').copied().collect();
        if is_float {
            let bits = s_to_f64_bits(&s);
            self.tokens.push(PyToken::FloatLit(bits));
        } else {
            let n: i64 = core::str::from_utf8(&s).unwrap_or("0").parse().unwrap_or(0);
            self.tokens.push(PyToken::IntLit(n));
        }
        Ok(())
    }

    fn read_name_or_keyword(&mut self) -> BxResult<()> {
        let start = self.pos;
        while self.pos < self.src.len() && (self.src[self.pos].is_ascii_alphanumeric() || self.src[self.pos] == b'_') {
            self.pos += 1;
        }
        let name = core::str::from_utf8(&self.src[start..self.pos]).unwrap_or("").to_string();
        let kw = match name.as_str() {
            "def" => Some(PyToken::Def),
            "class" => Some(PyToken::Class),
            "if" => Some(PyToken::If),
            "elif" => Some(PyToken::Elif),
            "else" => Some(PyToken::Else),
            "while" => Some(PyToken::While),
            "for" => Some(PyToken::For),
            "in" => Some(PyToken::In),
            "return" => Some(PyToken::Return),
            "break" => Some(PyToken::Break),
            "continue" => Some(PyToken::Continue),
            "pass" => Some(PyToken::Pass),
            "import" => Some(PyToken::Import),
            "from" => Some(PyToken::From),
            "as" => Some(PyToken::As),
            "try" => Some(PyToken::Try),
            "except" => Some(PyToken::Except),
            "finally" => Some(PyToken::Finally),
            "with" => Some(PyToken::With),
            "lambda" => Some(PyToken::Lambda),
            "and" => Some(PyToken::And),
            "or" => Some(PyToken::Or),
            "not" => Some(PyToken::Not),
            "is" => Some(PyToken::Is),
            "None" => Some(PyToken::None),
            "True" => Some(PyToken::True),
            "False" => Some(PyToken::False),
            "global" => Some(PyToken::Global),
            "nonlocal" => Some(PyToken::Nonlocal),
            "yield" => Some(PyToken::Yield),
            "assert" => Some(PyToken::Assert),
            "raise" => Some(PyToken::Raise),
            _ => None,
        };
        self.tokens.push(kw.unwrap_or(PyToken::Name(name)));
        Ok(())
    }

    fn read_operator(&mut self) -> BxResult<()> {
        let b = self.advance();
        let next = self.peek();
        let tok = match (b, next) {
            (b'+', b'=') => { self.advance(); PyToken::PlusEq }
            (b'-', b'=') => { self.advance(); PyToken::MinusEq }
            (b'*', b'=') => { self.advance(); PyToken::StarEq }
            (b'/', b'=') => { self.advance(); PyToken::SlashEq }
            (b'%', b'=') => { self.advance(); PyToken::PercentEq }
            (b'&', b'=') => { self.advance(); PyToken::AmpEq }
            (b'|', b'=') => { self.advance(); PyToken::PipeEq }
            (b'^', b'=') => { self.advance(); PyToken::CaretEq }
            (b'<', b'<') => {
                self.advance();
                if self.peek() == b'=' { self.advance(); PyToken::ShlEq }
                else { PyToken::Shl }
            }
            (b'>', b'>') => {
                self.advance();
                if self.peek() == b'=' { self.advance(); PyToken::ShrEq }
                else { PyToken::Shr }
            }
            (b'*', b'*') => {
                self.advance();
                if self.peek() == b'=' { self.advance(); PyToken::DblStarEq }
                else { PyToken::DblStar }
            }
            (b'/', b'/') => { self.advance(); PyToken::DblSlash }
            (b'=', b'=') => { self.advance(); PyToken::EqEq }
            (b'!', b'=') => { self.advance(); PyToken::NotEq }
            (b'<', b'=') => { self.advance(); PyToken::Le }
            (b'>', b'=') => { self.advance(); PyToken::Ge }
            (b'=', _) => PyToken::Assign,
            (b'<', _) => PyToken::Lt,
            (b'>', _) => PyToken::Gt,
            (b'+', _) => PyToken::Plus,
            (b'-', _) => PyToken::Minus,
            (b'*', _) => PyToken::Star,
            (b'/', _) => PyToken::Slash,
            (b'%', _) => PyToken::Percent,
            (b'&', _) => PyToken::Amp,
            (b'|', _) => PyToken::Pipe,
            (b'^', _) => PyToken::Caret,
            (b'~', _) => PyToken::Tilde,
            _ => PyToken::Eof,
        };
        self.tokens.push(tok);
        Ok(())
    }
}

fn s_to_f64_bits(s: &[u8]) -> u64 {
    let f: f64 = core::str::from_utf8(s).unwrap_or("0").parse().unwrap_or(0.0);
    f.to_bits()
}
