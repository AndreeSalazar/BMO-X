//! ÑEXO Lexer — Tokenización completa con soporte para:
//! - Keywords en español (32 keywords)
//! - Literales: enteros (decimal, hex, bin, oct), flotantes, strings, bytes
//! - Operadores: aritméticos, lógicos, bitwise, comparación
//! - Delimitadores y especiales
//! - Comments: `//` línea, `/* */` bloque

#![allow(dead_code)]

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

use crate::bmo_core::barex::BxResult;

/// Token types for ÑEXO.
#[derive(Debug, Clone)]
pub enum Token {
    // ── Literals ──────────────────────────────────────────────
    IntLit(u64),
    FloatLit(u64),  // stored as bits to avoid f64 Eq issue
    StrLit(String),
    ByteLit(u8),
    BoolLit(bool),

    // ── Identifier ────────────────────────────────────────────
    Ident(String),

    // ── Keywords ──────────────────────────────────────────────
    Fn,         // `fn`
    Let,        // `let`
    Mut,        // `mut`
    If,         // `si`
    Else,       // `sino`
    While,      // `mientras`
    For,        // `para`
    In,         // `en`
    Return,     // `retorna`
    Break,      // `rompe`
    Continue,   // `continua`
    Match,      // `empareja`
    Case,       // `caso`
    Default,    // `defecto`
    Struct,     // `tipo`
    Enum,       // `enumera`
    Impl,       // `implementa`
    Trait,      // `traza`
    Type,       // `alias`
    Module,     // `modulo`
    Pub,        // `pub`
    Use,        // `usa`
    As,         // `como`
    Import,     // `importa`
    Export,     // `exporta`
    True,       // `verdadero`
    False,      // `falso`
    Null,       // `nulo`
    Syscall,    // `sys`
    Emit,       // `emit`
    Reg,        // `reg`
    Aloc,       // `aloc`
    Libre,      // `libre`

    // ── Operators ─────────────────────────────────────────────
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

    // ── Delimiters ────────────────────────────────────────────
    LParen,     // (
    RParen,     // )
    LBrace,     // {
    RBrace,     // }
    LBracket,   // [
    RBracket,   // ]

    // ── Special ───────────────────────────────────────────────
    Eof,
}

impl Token {
    pub fn is_keyword(&self) -> bool {
        matches!(self,
            Token::Fn | Token::Let | Token::Mut | Token::If | Token::Else |
            Token::While | Token::For | Token::In | Token::Return |
            Token::Break | Token::Continue | Token::Match | Token::Case |
            Token::Default | Token::Struct | Token::Enum | Token::Impl |
            Token::Trait | Token::Type | Token::Module | Token::Pub |
            Token::Use | Token::As | Token::Import | Token::Export |
            Token::True | Token::False | Token::Null | Token::Syscall |
            Token::Emit | Token::Reg | Token::Aloc | Token::Libre
        )
    }
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
            self.skip_whitespace_and_comments();
            if self.pos >= self.src.len() {
                tokens.push(Token::Eof);
                break;
            }
            let tok = self.next_token()?;
            let is_eof = matches!(tok, Token::Eof);
            tokens.push(tok);
            if is_eof { break; }
        }
        Ok(tokens)
    }

    fn next_token(&mut self) -> BxResult<Token> {
        if self.pos >= self.src.len() {
            return Ok(Token::Eof);
        }
        let ch = self.src[self.pos];
        match ch {
            b'0' if self.peek_next() == Some(b'x') || self.peek_next() == Some(b'X') => self.read_hex(),
            b'0' if self.peek_next() == Some(b'b') || self.peek_next() == Some(b'B') => self.read_bin(),
            b'0' if self.peek_next() == Some(b'o') || self.peek_next() == Some(b'O') => self.read_oct(),
            b'0'..=b'9' => self.read_number(),
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => self.read_ident_or_keyword(),
            b'"' => self.read_string(b'"'),
            b'\'' => self.read_byte_or_char(),
            b'+' => { self.pos += 1; if self.peek() == b'=' { self.pos += 1; Ok(Token::PlusEq) } else { Ok(Token::Plus) } }
            b'-' => {
                self.pos += 1;
                match self.peek() {
                    b'>' => { self.pos += 1; Ok(Token::Arrow) }
                    b'=' => { self.pos += 1; Ok(Token::MinusEq) }
                    _ => Ok(Token::Minus),
                }
            }
            b'*' => { self.pos += 1; if self.peek() == b'=' { self.pos += 1; Ok(Token::StarEq) } else { Ok(Token::Star) } }
            b'/' => { self.pos += 1; if self.peek() == b'=' { self.pos += 1; Ok(Token::SlashEq) } else { Ok(Token::Slash) } }
            b'%' => { self.pos += 1; Ok(Token::Percent) }
            b'=' => {
                self.pos += 1;
                match self.peek() {
                    b'=' => { self.pos += 1; Ok(Token::EqEq) }
                    b'>' => { self.pos += 1; Ok(Token::FatArrow) }
                    _ => Ok(Token::Eq),
                }
            }
            b'!' => {
                self.pos += 1;
                if self.peek() == b'=' { self.pos += 1; Ok(Token::Ne) } else { Ok(Token::Bang) }
            }
            b'<' => {
                self.pos += 1;
                match self.peek() {
                    b'=' => { self.pos += 1; Ok(Token::Le) }
                    b'<' => { self.pos += 1; Ok(Token::Shl) }
                    _ => Ok(Token::Lt),
                }
            }
            b'>' => {
                self.pos += 1;
                match self.peek() {
                    b'=' => { self.pos += 1; Ok(Token::Ge) }
                    b'>' => { self.pos += 1; Ok(Token::Shr) }
                    _ => Ok(Token::Gt),
                }
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
                if self.peek() == b':' { self.pos += 1; Ok(Token::ColonColon) } else { Ok(Token::Colon) }
            }
            b'.' => { self.pos += 1; Ok(Token::Dot) }
            b'#' => { self.pos += 1; Ok(Token::Pound) }
            b'@' => { self.pos += 1; Ok(Token::At) }
            _ => { self.pos += 1; Ok(Token::Eof) }
        }
    }

    fn skip_whitespace_and_comments(&mut self) {
        while self.pos < self.src.len() {
            match self.src[self.pos] {
                b' ' | b'\t' | b'\r' | b'\n' => { self.pos += 1; }
                b'/' if self.peek_next() == Some(b'/') => {
                    self.pos += 2;
                    while self.pos < self.src.len() && self.src[self.pos] != b'\n' { self.pos += 1; }
                }
                b'/' if self.peek_next() == Some(b'*') => {
                    self.pos += 2;
                    while self.pos + 1 < self.src.len() {
                        if self.src[self.pos] == b'*' && self.src[self.pos + 1] == b'/' { self.pos += 2; break; }
                        self.pos += 1;
                    }
                }
                _ => break,
            }
        }
    }

    fn read_hex(&mut self) -> BxResult<Token> {
        self.pos += 2; // skip 0x
        let start = self.pos;
        while self.pos < self.src.len() && (self.src[self.pos].is_ascii_hexdigit() || self.src[self.pos] == b'_') {
            self.pos += 1;
        }
        let s = core::str::from_utf8(&self.src[start..self.pos]).unwrap_or("0");
        let cleaned: alloc::string::String = s.chars().filter(|c| *c != '_').collect();
        let val = u64::from_str_radix(&cleaned, 16).unwrap_or(0);
        Ok(Token::IntLit(val))
    }

    fn read_bin(&mut self) -> BxResult<Token> {
        self.pos += 2; // skip 0b
        let start = self.pos;
        while self.pos < self.src.len() && (self.src[self.pos] == b'0' || self.src[self.pos] == b'1' || self.src[self.pos] == b'_') {
            self.pos += 1;
        }
        let s = core::str::from_utf8(&self.src[start..self.pos]).unwrap_or("0");
        let cleaned: alloc::string::String = s.chars().filter(|c| *c != '_').collect();
        let val = u64::from_str_radix(&cleaned, 2).unwrap_or(0);
        Ok(Token::IntLit(val))
    }

    fn read_oct(&mut self) -> BxResult<Token> {
        self.pos += 2; // skip 0o
        let start = self.pos;
        while self.pos < self.src.len() && (self.src[self.pos] >= b'0' && self.src[self.pos] <= b'7' || self.src[self.pos] == b'_') {
            self.pos += 1;
        }
        let s = core::str::from_utf8(&self.src[start..self.pos]).unwrap_or("0");
        let cleaned: alloc::string::String = s.chars().filter(|c| *c != '_').collect();
        let val = u64::from_str_radix(&cleaned, 8).unwrap_or(0);
        Ok(Token::IntLit(val))
    }

    fn read_number(&mut self) -> BxResult<Token> {
        let start = self.pos;
        while self.pos < self.src.len() && (self.src[self.pos].is_ascii_digit() || self.src[self.pos] == b'_') {
            self.pos += 1;
        }
        // Check for float
        if self.peek() == b'.' && self.peek_next().map_or(false, |c| c.is_ascii_digit()) {
            self.pos += 1; // skip .
            while self.pos < self.src.len() && self.src[self.pos].is_ascii_digit() { self.pos += 1; }
            let s = core::str::from_utf8(&self.src[start..self.pos]).unwrap_or("0.0");
            let cleaned: alloc::string::String = s.chars().filter(|c| *c != '_').collect();
            let val = cleaned.parse::<f64>().unwrap_or(0.0);
            return Ok(Token::FloatLit(val.to_bits()));
        }
        let s = core::str::from_utf8(&self.src[start..self.pos]).unwrap_or("0");
        let cleaned: alloc::string::String = s.chars().filter(|c| *c != '_').collect();
        let val = cleaned.parse::<u64>().unwrap_or(0);
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
            "caso" => Token::Case,
            "defecto" => Token::Default,
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
            "emit" => Token::Emit,
            "reg" => Token::Reg,
            "aloc" => Token::Aloc,
            "libre" => Token::Libre,
            _ => Token::Ident(alloc::string::String::from(word)),
        })
    }

    fn read_string(&mut self, quote: u8) -> BxResult<Token> {
        self.pos += 1; // skip opening quote
        let mut result = alloc::string::String::new();
        while self.pos < self.src.len() && self.src[self.pos] != quote {
            if self.src[self.pos] == b'\\' {
                self.pos += 1;
                if self.pos < self.src.len() {
                    match self.src[self.pos] {
                        b'n' => result.push('\n'),
                        b't' => result.push('\t'),
                        b'r' => result.push('\r'),
                        b'\\' => result.push('\\'),
                        b'0' => result.push('\0'),
                        b'\'' => result.push('\''),
                        b'"' => result.push('"'),
                        b'x' => {
                            self.pos += 1;
                            if self.pos + 1 < self.src.len() {
                                let h1 = hex_digit(self.src[self.pos]);
                                let h2 = hex_digit(self.src[self.pos + 1]);
                                result.push((h1 * 16 + h2) as char);
                            }
                        }
                        c => result.push(c as char),
                    }
                }
            } else {
                result.push(self.src[self.pos] as char);
            }
            self.pos += 1;
        }
        if self.pos < self.src.len() { self.pos += 1; } // skip closing quote
        Ok(Token::StrLit(result))
    }

    fn read_byte_or_char(&mut self) -> BxResult<Token> {
        self.pos += 1; // skip opening quote
        let val = if self.src[self.pos] == b'\\' {
            self.pos += 1;
            match self.src[self.pos] {
                b'n' => b'\n',
                b't' => b'\t',
                b'r' => b'\r',
                b'\\' => b'\\',
                b'0' => 0,
                b'\'' => b'\'',
                b'"' => b'"',
                c => c,
            }
        } else {
            self.src[self.pos]
        };
        self.pos += 1;
        if self.pos < self.src.len() { self.pos += 1; } // skip closing quote
        Ok(Token::ByteLit(val))
    }

    fn peek(&self) -> u8 {
        if self.pos < self.src.len() { self.src[self.pos] } else { 0 }
    }

    fn peek_next(&self) -> Option<u8> {
        if self.pos + 1 < self.src.len() { Some(self.src[self.pos + 1]) } else { None }
    }
}

fn hex_digit(b: u8) -> u8 {
    match b {
        b'0'..=b'9' => b - b'0',
        b'a'..=b'f' => b - b'a' + 10,
        b'A'..=b'F' => b - b'A' + 10,
        _ => 0,
    }
}
