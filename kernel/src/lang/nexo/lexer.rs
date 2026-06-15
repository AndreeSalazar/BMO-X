//! ÑEXO Lexer — Tokenización del fuente.
//!
//! Convierte caracteres en tokens para el parser.
//! Soporta: identifiers, keywords, literals, operators, delimiters.

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;

/// Token types for ÑEXO.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Literals
    IntLit(u64),
    FloatLit(f64),
    StrLit(String),
    ByteLit(u8),
    BoolLit(bool),

    // Identifier
    Ident(String),

    // Keywords
    Fn,
    Let,
    Mut,
    If,
    Else,
    While,
    For,
    In,
    Return,
    Break,
    Continue,
    Match,
    Struct,
    Enum,
    Impl,
    Trait,
    Type,
    Module,
    Pub,
    Use,
    As,
    Import,
    Export,
    True,
    False,
    Null,
    Syscall,

    // Operators
    Plus,       // +
    Minus,      // -
    Star,       // *
    Slash,      // /
    Percent,    // %
    Amp,        // &
    Pipe,       // |
    Caret,      // ^
    Tilde,      // ~
    Bang,       // !
    Eq,         // =
    EqEq,       // ==
    Ne,         // !=
    Lt,         // <
    Gt,         // >
    Le,         // <=
    Ge,         // >=
    AmpAmp,     // &&
    PipePipe,   // ||
    Shl,        // <<
    Shr,        // >>
    PlusEq,     // +=
    MinusEq,    // -=
    StarEq,     // *=
    SlashEq,    // /=
    Arrow,      // ->
    FatArrow,   // =>
    Dot,        // .
    Colon,      // :
    ColonColon, // ::
    Semi,       // ;
    Comma,      // ,
    Pound,      // #
    At,         // @

    // Delimiters
    LParen,     // (
    RParen,     // )
    LBrace,     // {
    RBrace,     // }
    LBracket,   // [
    RBracket,   // ]

    // Special
    Eof,
}

/// Lexer state.
pub struct Lexer<'a> {
    src: &'a [u8],
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a [u8]) -> Self {
        Self { src, pos: 0 }
    }

    /// Tokenize the entire source and return all tokens.
    pub fn tokenize(&mut self) -> BxResult<Vec<Token>> {
        let mut tokens = Vec::new();
        loop {
            let tok = self.next_token()?;
            if tok == Token::Eof {
                tokens.push(Token::Eof);
                break;
            }
            tokens.push(tok);
        }
        Ok(tokens)
    }

    fn next_token(&mut self) -> BxResult<Token> {
        self.skip_whitespace_and_comments();
        if self.pos >= self.src.len() {
            return Ok(Token::Eof);
        }
        let ch = self.src[self.pos];
        match ch {
            b'0'..=b'9' => self.read_number(),
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => self.read_ident_or_keyword(),
            b'"' | b'\'' => self.read_string(),
            b'+' => { self.pos += 1; Ok(Token::Plus) }
            b'-' => {
                self.pos += 1;
                if self.peek() == b'>' { self.pos += 1; Ok(Token::Arrow) }
                else if self.peek() == b'=' { self.pos += 1; Ok(Token::MinusEq) }
                else { Ok(Token::Minus) }
            }
            b'*' => { self.pos += 1; if self.peek() == b'=' { self.pos += 1; Ok(Token::StarEq) } else { Ok(Token::Star) } }
            b'/' => { self.pos += 1; if self.peek() == b'=' { self.pos += 1; Ok(Token::SlashEq) } else { Ok(Token::Slash) } }
            b'%' => { self.pos += 1; Ok(Token::Percent) }
            b'=' => {
                self.pos += 1;
                if self.peek() == b'=' { self.pos += 1; Ok(Token::EqEq) }
                else if self.peek() == b'>' { self.pos += 1; Ok(Token::FatArrow) }
                else { Ok(Token::Eq) }
            }
            b'!' => {
                self.pos += 1;
                if self.peek() == b'=' { self.pos += 1; Ok(Token::Ne) }
                else { Ok(Token::Bang) }
            }
            b'<' => {
                self.pos += 1;
                if self.peek() == b'=' { self.pos += 1; Ok(Token::Le) }
                else if self.peek() == b'<' { self.pos += 1; Ok(Token::Shl) }
                else { Ok(Token::Lt) }
            }
            b'>' => {
                self.pos += 1;
                if self.peek() == b'=' { self.pos += 1; Ok(Token::Ge) }
                else if self.peek() == b'>' { self.pos += 1; Ok(Token::Shr) }
                else { Ok(Token::Gt) }
            }
            b'&' => { self.pos += 1; if self.peek() == b'&' { self.pos += 1; Ok(Token::AmpAmp) } else { Ok(Token::Amp) } }
            b'|' => { self.pos += 1; if self.peek() == b'|' { self.pos += 1; Ok(Token::PipePipe) } else { Ok(Token::Pipe) } }
            b'^' => { self.pos += 1; Ok(Token::Caret) }
            b'~' => { self.pos += 1; Ok(Token::Tilde) }
            b'(' => { self.pos += 1; Ok(Token::LParen) }
            b')' => { self.pos += 1; Ok(Token::RParen) }
            b'{' => { self.pos += 1; Ok(Token::LBrace) }
            b'}' => { self.pos += 1; Ok(Token::RBrace) }
            b'[' => { self.pos += 1; Ok(Token::LBracket) }
            b']' => { self.pos += 1; Ok(Token::RBracket) }
            b';' => { self.pos += 1; Ok(Token::Semi) }
            b',' => { self.pos += 1; Ok(Token::Comma) }
            b':' => {
                self.pos += 1;
                if self.peek() == b':' { self.pos += 1; Ok(Token::ColonColon) }
                else { Ok(Token::Colon) }
            }
            b'.' => { self.pos += 1; Ok(Token::Dot) }
            b'#' => { self.pos += 1; Ok(Token::Pound) }
            b'@' => { self.pos += 1; Ok(Token::At) }
            _ => {
                self.pos += 1;
                Ok(Token::Eof) // Unknown char, treat as EOF for now
            }
        }
    }

    fn skip_whitespace_and_comments(&mut self) {
        while self.pos < self.src.len() {
            match self.src[self.pos] {
                b' ' | b'\t' | b'\r' | b'\n' => { self.pos += 1; }
                b'/' if self.peek_next() == Some(b'/') => {
                    // Line comment
                    while self.pos < self.src.len() && self.src[self.pos] != b'\n' {
                        self.pos += 1;
                    }
                }
                b'/' if self.peek_next() == Some(b'*') => {
                    // Block comment
                    self.pos += 2;
                    while self.pos + 1 < self.src.len() {
                        if self.src[self.pos] == b'*' && self.src[self.pos + 1] == b'/' {
                            self.pos += 2;
                            break;
                        }
                        self.pos += 1;
                    }
                }
                _ => break,
            }
        }
    }

    fn read_number(&mut self) -> BxResult<Token> {
        let start = self.pos;
        while self.pos < self.src.len() && self.src[self.pos].is_ascii_digit() {
            self.pos += 1;
        }
        let s = core::str::from_utf8(&self.src[start..self.pos]).unwrap_or("0");
        let val = s.parse::<u64>().unwrap_or(0);
        Ok(Token::IntLit(val))
    }

    fn read_ident_or_keyword(&mut self) -> BxResult<Token> {
        let start = self.pos;
        while self.pos < self.src.len() && (self.src[self.pos].is_ascii_alphanumeric() || self.src[self.pos] == b'_') {
            self.pos += 1;
        }
        let word = core::str::from_utf8(&self.src[start..self.pos]).unwrap_or("");
        Ok(match word {
            "fn" => Token::Fn,
            "let" => Token::Let,
            "mut" => Token::Mut,
            "si" => Token::If,
            "sino" => Token::Else,
            "mientras" => Token::While,
            "para" => Token::For,
            "en" => Token::In,
            "retorna" => Token::Return,
            "rompe" => Token::Break,
            "continua" => Token::Continue,
            "empareja" => Token::Match,
            "tipo" => Token::Struct,
            "enumera" => Token::Enum,
            "implementa" => Token::Impl,
            "traza" => Token::Trait,
            "alias" => Token::Type,
            "modulo" => Token::Module,
            "pub" => Token::Pub,
            "usa" => Token::Use,
            "como" => Token::As,
            "importa" => Token::Import,
            "exporta" => Token::Export,
            "verdadero" => Token::True,
            "falso" => Token::False,
            "nulo" => Token::Null,
            "sys" => Token::Syscall,
            _ => Token::Ident(String::from(word)),
        })
    }

    fn read_string(&mut self) -> BxResult<Token> {
        let quote = self.src[self.pos];
        self.pos += 1;
        let start = self.pos;
        while self.pos < self.src.len() && self.src[self.pos] != quote {
            if self.src[self.pos] == b'\\' { self.pos += 1; } // skip escape
            self.pos += 1;
        }
        let s = core::str::from_utf8(&self.src[start..self.pos]).unwrap_or("");
        if self.pos < self.src.len() { self.pos += 1; } // skip closing quote
        Ok(Token::StrLit(String::from(s)))
    }

    fn peek(&self) -> u8 {
        if self.pos < self.src.len() { self.src[self.pos] } else { 0 }
    }

    fn peek_next(&self) -> Option<u8> {
        if self.pos + 1 < self.src.len() { Some(self.src[self.pos + 1]) } else { None }
    }
}

use crate::barex::BxResult;
