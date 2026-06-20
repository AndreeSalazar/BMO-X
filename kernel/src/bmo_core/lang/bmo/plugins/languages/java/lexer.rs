//! Java Lexer — tokenizes the essential subset.

#![allow(dead_code)]

extern crate alloc;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;

use crate::bmo_gpu::BxResult;

/// Java token.
#[derive(Debug, Clone, PartialEq)]
pub enum JToken {
    // Literals
    IntLit(i64), FloatLit(u64), StrLit(String), CharLit(char), Ident(String),
    // Keywords
    Class, Interface, Extends, Implements, Public, Private, Protected,
    Static, Final, Abstract, Void, Boolean, Byte, Short, Int, Long,
    Float, Double, Char, If, Else, While, For, Do, Return, Break, Continue,
    New, This, Null, True, False, Instanceof, Try, Catch, Finally, Throw,
    Import, Package, Static2, // unused
    // Operators
    Plus, Minus, Star, Slash, Percent,
    PlusPlus, MinusMinus,
    Eq, EqEq, NotEq, Lt, Gt, Le, Ge,
    AmpAmp, PipePipe, Bang,
    Amp, Pipe, Caret, Tilde, Shl, Shr,
    PlusEq, MinusEq, StarEq, SlashEq, PercentEq,
    // Delimiters
    LParen, RParen, LBrace, RBrace, LBracket, RBracket,
    Semi, Comma, Dot,
    // Special
    Eof,
}

/// Java Lexer.
pub struct JLexer {
    src: Vec<u8>,
    pos: usize,
}

impl JLexer {
    pub fn new(source: &[u8]) -> Self {
        Self { src: source.to_vec(), pos: 0 }
    }

    pub fn tokenize(&mut self) -> BxResult<Vec<JToken>> {
        let mut out = Vec::new();
        while self.pos < self.src.len() {
            let b = self.peek();
            match b {
                b' ' | b'\t' | b'\r' | b'\n' => { self.advance(); }
                b'/' if self.pos + 1 < self.src.len() && self.src[self.pos + 1] == b'/' => {
                    while self.pos < self.src.len() && self.peek() != b'\n' { self.advance(); }
                }
                b'/' if self.pos + 1 < self.src.len() && self.src[self.pos + 1] == b'*' => {
                    self.advance(); self.advance();
                    while self.pos + 1 < self.src.len() {
                        if self.src[self.pos] == b'*' && self.src[self.pos + 1] == b'/' {
                            self.advance(); self.advance(); break;
                        }
                        self.advance();
                    }
                }
                b'0'..=b'9' => { self.read_number(&mut out); }
                b'"' => { self.read_string(&mut out); }
                b'\'' => { self.read_char(&mut out); }
                b'A'..=b'Z' | b'a'..=b'z' | b'_' | b'$' => { self.read_ident_or_kw(&mut out); }
                b'(' => { self.advance(); out.push(JToken::LParen); }
                b')' => { self.advance(); out.push(JToken::RParen); }
                b'{' => { self.advance(); out.push(JToken::LBrace); }
                b'}' => { self.advance(); out.push(JToken::RBrace); }
                b'[' => { self.advance(); out.push(JToken::LBracket); }
                b']' => { self.advance(); out.push(JToken::RBracket); }
                b';' => { self.advance(); out.push(JToken::Semi); }
                b',' => { self.advance(); out.push(JToken::Comma); }
                b'.' => { self.advance(); out.push(JToken::Dot); }
                b'+' => {
                    self.advance();
                    if self.peek() == b'+' { self.advance(); out.push(JToken::PlusPlus); }
                    else if self.peek() == b'=' { self.advance(); out.push(JToken::PlusEq); }
                    else { out.push(JToken::Plus); }
                }
                b'-' => {
                    self.advance();
                    if self.peek() == b'-' { self.advance(); out.push(JToken::MinusMinus); }
                    else if self.peek() == b'=' { self.advance(); out.push(JToken::MinusEq); }
                    else { out.push(JToken::Minus); }
                }
                b'*' => {
                    self.advance();
                    if self.peek() == b'=' { self.advance(); out.push(JToken::StarEq); }
                    else { out.push(JToken::Star); }
                }
                b'/' => {
                    self.advance();
                    if self.peek() == b'=' { self.advance(); out.push(JToken::SlashEq); }
                    else { out.push(JToken::Slash); }
                }
                b'%' => {
                    self.advance();
                    if self.peek() == b'=' { self.advance(); out.push(JToken::PercentEq); }
                    else { out.push(JToken::Percent); }
                }
                b'=' => { self.advance(); if self.peek() == b'=' { self.advance(); out.push(JToken::EqEq); } else { out.push(JToken::Eq); } }
                b'!' => { self.advance(); if self.peek() == b'=' { self.advance(); out.push(JToken::NotEq); } else { out.push(JToken::Bang); } }
                b'<' => {
                    self.advance();
                    if self.peek() == b'=' { self.advance(); out.push(JToken::Le); }
                    else if self.peek() == b'<' { self.advance(); out.push(JToken::Shl); }
                    else { out.push(JToken::Lt); }
                }
                b'>' => {
                    self.advance();
                    if self.peek() == b'=' { self.advance(); out.push(JToken::Ge); }
                    else if self.peek() == b'>' {
                        self.advance();
                        if self.peek() == b'>' { self.advance(); out.push(JToken::Shr); }
                        else { out.push(JToken::Shr); /* simplified */ }
                    }
                    else { out.push(JToken::Gt); }
                }
                b'&' => {
                    self.advance();
                    if self.peek() == b'&' { self.advance(); out.push(JToken::AmpAmp); }
                    else { out.push(JToken::Amp); }
                }
                b'|' => {
                    self.advance();
                    if self.peek() == b'|' { self.advance(); out.push(JToken::PipePipe); }
                    else { out.push(JToken::Pipe); }
                }
                b'^' => { self.advance(); out.push(JToken::Caret); }
                b'~' => { self.advance(); out.push(JToken::Tilde); }
                _ => { self.advance(); } // skip unknown
            }
        }
        out.push(JToken::Eof);
        Ok(out)
    }

    fn peek(&self) -> u8 { if self.pos < self.src.len() { self.src[self.pos] } else { 0 } }
    fn advance(&mut self) -> u8 {
        let b = self.peek();
        if self.pos < self.src.len() { self.pos += 1; }
        b
    }

    fn read_number(&mut self, out: &mut Vec<JToken>) {
        let start = self.pos;
        while self.pos < self.src.len() && (self.peek().is_ascii_digit() || self.peek() == b'_') { self.pos += 1; }
        if self.pos < self.src.len() && self.peek() == b'.' { self.pos += 1; }
        while self.pos < self.src.len() && (self.peek().is_ascii_digit() || self.peek() == b'_') { self.pos += 1; }
        let s = core::str::from_utf8(&self.src[start..self.pos]).unwrap_or("0");
        if s.contains('.') {
            let f: f64 = s.parse().unwrap_or(0.0);
            out.push(JToken::FloatLit(f.to_bits()));
        } else {
            let n: i64 = s.parse().unwrap_or(0);
            out.push(JToken::IntLit(n));
        }
    }

    fn read_string(&mut self, out: &mut Vec<JToken>) {
        self.advance(); // opening "
        let mut s = String::new();
        while self.pos < self.src.len() && self.peek() != b'"' {
            let b = self.advance();
            if b == b'\\' && self.pos < self.src.len() {
                let esc = self.advance();
                match esc {
                    b'n' => s.push('\n'),
                    b't' => s.push('\t'),
                    _ => s.push(esc as char),
                }
            } else { s.push(b as char); }
        }
        if self.pos < self.src.len() { self.advance(); }
        out.push(JToken::StrLit(s));
    }

    fn read_char(&mut self, out: &mut Vec<JToken>) {
        self.advance(); // opening '
        let ch = if self.pos < self.src.len() { self.advance() as char } else { '\0' };
        if self.pos < self.src.len() { self.advance(); } // closing '
        out.push(JToken::CharLit(ch));
    }

    fn read_ident_or_kw(&mut self, out: &mut Vec<JToken>) {
        let start = self.pos;
        while self.pos < self.src.len() && (self.peek().is_ascii_alphanumeric() || self.peek() == b'_' || self.peek() == b'$') {
            self.pos += 1;
        }
        let name = core::str::from_utf8(&self.src[start..self.pos]).unwrap_or("").to_string();
        let tok = match name.as_str() {
            "class" => JToken::Class,
            "interface" => JToken::Interface,
            "extends" => JToken::Extends,
            "implements" => JToken::Implements,
            "public" => JToken::Public,
            "private" => JToken::Private,
            "protected" => JToken::Protected,
            "static" => JToken::Static,
            "final" => JToken::Final,
            "abstract" => JToken::Abstract,
            "void" => JToken::Void,
            "boolean" => JToken::Boolean,
            "byte" => JToken::Byte,
            "short" => JToken::Short,
            "int" => JToken::Int,
            "long" => JToken::Long,
            "float" => JToken::Float,
            "double" => JToken::Double,
            "char" => JToken::Char,
            "if" => JToken::If,
            "else" => JToken::Else,
            "while" => JToken::While,
            "for" => JToken::For,
            "do" => JToken::Do,
            "return" => JToken::Return,
            "break" => JToken::Break,
            "continue" => JToken::Continue,
            "new" => JToken::New,
            "this" => JToken::This,
            "null" => JToken::Null,
            "true" => JToken::True,
            "false" => JToken::False,
            "instanceof" => JToken::Instanceof,
            "try" => JToken::Try,
            "catch" => JToken::Catch,
            "finally" => JToken::Finally,
            "throw" => JToken::Throw,
            _ => JToken::Ident(name),
        };
        out.push(tok);
    }
}

