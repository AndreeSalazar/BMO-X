//! C Lexer — tokenizes C source code.

#![allow(dead_code)]

extern crate alloc;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;

use crate::bmo_gpu::BxResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CToken {
    // Literals
    IntLit(u64),
    StrLit(String),
    CharLit(u8),
    // Identifier
    Ident(String),
    // Keywords
    Int, Unsigned, Long, Char, Void, Short, Float, Double,
    Struct, Union, Enum, Typedef, Static, Extern, Const, Volatile,
    If, Else, While, For, Do, Switch, Case, Default, Break, Continue, Return,
    Sizeof, StructOp, // -> 
    Goto, Label,      // goto, label
    // Operators
    Plus, Minus, Star, Slash, Percent,
    Amp, Pipe, Caret, Tilde, Bang,
    Eq, EqEq, BangEq, Lt, Gt, Le, Ge,
    AmpAmp, PipePipe,
    PlusEq, MinusEq, StarEq, SlashEq,
    AmpEq, PipeEq, CaretEq, ShlEq, ShrEq,
    PlusPlus, MinusMinus,
    Arrow, Dot,
    Question,          // ? for ternary
    // Delimiters
    LParen, RParen, LBrace, RBrace, LBracket, RBracket,
    Semi, Comma, Colon,
    // Special
    Eof,
}

/// C Lexer — tokenizes C source code.
pub struct CLexer {
    src: Vec<u8>,
    pos: usize,
}

impl CLexer {
    pub fn new(source: &[u8]) -> Self {
        Self { src: source.to_vec(), pos: 0 }
    }

    fn peek(&self) -> u8 {
        if self.pos < self.src.len() { self.src[self.pos] } else { 0 }
    }

    fn advance(&mut self) -> u8 {
        let b = self.peek();
        if self.pos < self.src.len() { self.pos += 1; }
        b
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            match self.peek() {
                b' ' | b'\t' | b'\r' | b'\n' => { self.advance(); }
                b'/' if self.pos + 1 < self.src.len() && self.src[self.pos + 1] == b'/' => {
                    while self.pos < self.src.len() && self.src[self.pos] != b'\n' {
                        self.pos += 1;
                    }
                }
                b'/' if self.pos + 1 < self.src.len() && self.src[self.pos + 1] == b'*' => {
                    self.advance(); self.advance(); // skip /*
                    while self.pos + 1 < self.src.len() {
                        if self.src[self.pos] == b'*' && self.src[self.pos + 1] == b'/' {
                            self.advance(); self.advance();
                            break;
                        }
                        self.advance();
                    }
                }
                b'#' => {
                    // Skip preprocessor directives
                    while self.pos < self.src.len() && self.src[self.pos] != b'\n' {
                        self.pos += 1;
                    }
                }
                _ => break,
            }
        }
    }

    fn read_string(&mut self) -> String {
        self.advance(); // skip opening quote
        let mut s = String::new();
        while self.pos < self.src.len() && self.peek() != b'"' {
            if self.peek() == b'\\' {
                self.advance();
                match self.advance() {
                    b'n' => s.push('\n'),
                    b't' => s.push('\t'),
                    b'\\' => s.push('\\'),
                    b'0' => s.push('\0'),
                    b'"' => s.push('"'),
                    c => s.push(c as char),
                }
            } else {
                s.push(self.advance() as char);
            }
        }
        if self.pos < self.src.len() { self.advance(); } // skip closing quote
        s
    }

    fn read_number(&mut self) -> u64 {
        let start = self.pos;
        if self.peek() == b'0' && self.pos + 1 < self.src.len() {
            self.advance();
            if self.peek() == b'x' || self.peek() == b'X' {
                self.advance();
                while self.pos < self.src.len() && self.peek().is_ascii_hexdigit() {
                    self.pos += 1;
                }
                let s = core::str::from_utf8(&self.src[start..self.pos]).unwrap_or("0");
                return u64::from_str_radix(&s[2..], 16).unwrap_or(0);
            }
        }
        while self.pos < self.src.len() && self.peek().is_ascii_digit() {
            self.pos += 1;
        }
        let s = core::str::from_utf8(&self.src[start..self.pos]).unwrap_or("0");
        s.parse::<u64>().unwrap_or(0)
    }

    fn read_ident(&mut self) -> String {
        let start = self.pos;
        while self.pos < self.src.len() && (self.src[self.pos].is_ascii_alphanumeric() || self.src[self.pos] == b'_') {
            self.pos += 1;
        }
        core::str::from_utf8(&self.src[start..self.pos]).unwrap_or("").to_string()
    }

    /// Tokenize the entire C source.
    pub fn tokenize(&mut self) -> BxResult<Vec<CToken>> {
        let mut tokens = Vec::new();
        loop {
            self.skip_whitespace_and_comments();
            if self.pos >= self.src.len() {
                tokens.push(CToken::Eof);
                break;
            }
            let tok = match self.peek() {
                b'0'..=b'9' => CToken::IntLit(self.read_number()),
                b'"' => CToken::StrLit(self.read_string()),
                b'\'' => {
                    self.advance();
                    let ch = if self.peek() == b'\\' {
                        self.advance();
                        match self.advance() {
                            b'n' => b'\n',
                            b't' => b'\t',
                            b'\\' => b'\\',
                            b'0' => 0,
                            c => c,
                        }
                    } else {
                        self.advance()
                    };
                    if self.pos < self.src.len() { self.advance(); } // closing quote
                    CToken::CharLit(ch)
                }
                b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                    let ident = self.read_ident();
                    match ident.as_str() {
                        "int" => CToken::Int,
                        "unsigned" => CToken::Unsigned,
                        "long" => CToken::Long,
                        "char" => CToken::Char,
                        "void" => CToken::Void,
                        "short" => CToken::Short,
                        "float" => CToken::Float,
                        "double" => CToken::Double,
                        "struct" => CToken::Struct,
                        "union" => CToken::Union,
                        "enum" => CToken::Enum,
                        "typedef" => CToken::Typedef,
                        "static" => CToken::Static,
                        "extern" => CToken::Extern,
                        "const" => CToken::Const,
                        "volatile" => CToken::Volatile,
                        "if" => CToken::If,
                        "else" => CToken::Else,
                        "while" => CToken::While,
                        "for" => CToken::For,
                        "do" => CToken::Do,
                        "switch" => CToken::Switch,
                        "case" => CToken::Case,
                        "default" => CToken::Default,
                        "break" => CToken::Break,
                        "continue" => CToken::Continue,
                        "return" => CToken::Return,
                        "sizeof" => CToken::Sizeof,
                        "goto" => CToken::Goto,
                        _ => CToken::Ident(ident),
                    }
                }
                b'+' => { self.advance(); if self.peek() == b'+' { self.advance(); CToken::PlusPlus } else if self.peek() == b'=' { self.advance(); CToken::PlusEq } else { CToken::Plus } }
                b'-' => { self.advance(); if self.peek() == b'-' { self.advance(); CToken::MinusMinus } else if self.peek() == b'=' { self.advance(); CToken::MinusEq } else if self.peek() == b'>' { self.advance(); CToken::Arrow } else { CToken::Minus } }
                b'*' => { self.advance(); if self.peek() == b'=' { self.advance(); CToken::StarEq } else { CToken::Star } }
                b'/' => { self.advance(); if self.peek() == b'=' { self.advance(); CToken::SlashEq } else { CToken::Slash } }
                b'%' => { self.advance(); CToken::Percent }
                b'&' => { self.advance(); if self.peek() == b'&' { self.advance(); CToken::AmpAmp } else { CToken::Amp } }
                b'|' => { self.advance(); if self.peek() == b'|' { self.advance(); CToken::PipePipe } else { CToken::Pipe } }
                b'^' => { self.advance(); CToken::Caret }
                b'~' => { self.advance(); CToken::Tilde }
                b'!' => { self.advance(); if self.peek() == b'=' { self.advance(); CToken::BangEq } else { CToken::Bang } }
                b'=' => { self.advance(); if self.peek() == b'=' { self.advance(); CToken::EqEq } else { CToken::Eq } }
                b'<' => { self.advance(); if self.peek() == b'=' { self.advance(); CToken::Le } else { CToken::Lt } }
                b'>' => { self.advance(); if self.peek() == b'=' { self.advance(); CToken::Ge } else { CToken::Gt } }
                b'(' => { self.advance(); CToken::LParen }
                b')' => { self.advance(); CToken::RParen }
                b'{' => { self.advance(); CToken::LBrace }
                b'}' => { self.advance(); CToken::RBrace }
                b'[' => { self.advance(); CToken::LBracket }
                b']' => { self.advance(); CToken::RBracket }
                b';' => { self.advance(); CToken::Semi }
                b',' => { self.advance(); CToken::Comma }
                b':' => { self.advance(); CToken::Colon }
                b'?' => { self.advance(); CToken::Question }
                b'.' => { self.advance(); CToken::Dot }
                _ => { self.advance(); continue; } // skip unknown
            };
            tokens.push(tok);
        }
        Ok(tokens)
    }
}
