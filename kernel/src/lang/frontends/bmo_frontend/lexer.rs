//! BMO Lexer — convierte texto a tokens.
//!
//! El lexer es **stateless**: solo conoce el texto fuente. Reporta
//! errores con `Span` para que el parser pueda reconstruir la posición.

#![allow(dead_code)]

use crate::lang::common::source::{Pos, Span};
use alloc::string::String;
use alloc::vec::Vec;

/// Token del lenguaje BMO.
#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    pub kind: TokKind,
    pub text: String,
    pub span: Span,
}

impl Token {
    pub fn new(kind: TokKind, text: String, span: Span) -> Self {
        Self { kind, text, span }
    }
}

/// Tipo del token.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TokKind {
    // ─── Literales ────────────────────────────────────────────
    IntLit,
    FloatLit,
    StrLit,
    CharLit,
    BoolLit,

    // ─── Identificadores y keywords ───────────────────────────
    Ident,
    KwFn, KwLet, KwIf, KwElse, KwWhile, KwFor, KwReturn,
    KwBreak, KwContinue, KwLoop, KwSwitch, KwCase, KwDefault,
    KwStruct, KwUnion, KwEnum, KwType, KwTrue, KwFalse,
    KwAs, KwUse, KwExtern, KwStatic, KwConst, KwMut,
    KwNull, KwSizeOf, KwLabel, KwGoto,

    // ─── Puntuación ───────────────────────────────────────────
    LParen, RParen,     // ( )
    LBrace, RBrace,     // { }
    LBracket, RBracket, // [ ]
    Comma, Semi, Colon, Dot, Arrow, FatArrow, // , ; : . -> =>
    At, Hash, Dollar, Question, // @ # $ ?

    // ─── Operadores ───────────────────────────────────────────
    Plus, Minus, Star, Slash, Percent,
    Amp, Pipe, Caret, Tilde, Shl, Shr,
    Eq, EqEq, NotEq, Lt, LtEq, Gt, GtEq,
    And, Or, Bang,
    PlusEq, MinusEq, StarEq, SlashEq, PercentEq,
    AmpEq, PipeEq, CaretEq, ShlEq, ShrEq,

    // ─── Especiales ───────────────────────────────────────────
    Whitespace,
    Newline,
    Comment,
    Eof,
}

impl TokKind {
    pub fn is_trivia(self) -> bool {
        matches!(self, Self::Whitespace | Self::Newline | Self::Comment)
    }
}

/// Error de lexing.
#[derive(Clone, Debug)]
pub enum LexError {
    UnexpectedChar(char, Pos),
    UnterminatedString(Pos),
    UnterminatedChar(Pos),
    InvalidEscape(char, Pos),
    NumberTooLarge(Pos),
    InvalidUtf8(Pos),
}

impl core::fmt::Display for LexError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnexpectedChar(c, p) => write!(f, "unexpected char '{}' at {}", c, p),
            Self::UnterminatedString(p) => write!(f, "unterminated string at {}", p),
            Self::UnterminatedChar(p) => write!(f, "unterminated char literal at {}", p),
            Self::InvalidEscape(c, p) => write!(f, "invalid escape '\\{}' at {}", c, p),
            Self::NumberTooLarge(p) => write!(f, "number too large at {}", p),
            Self::InvalidUtf8(p) => write!(f, "invalid UTF-8 at {}", p),
        }
    }
}

/// El lexer.
pub struct Lexer<'src> {
    src: &'src [u8],
    pos: usize,
    line: u32,
    column: u32,
}

impl<'src> Lexer<'src> {
    pub fn new(src: &'src [u8]) -> Self {
        Self { src, pos: 0, line: 1, column: 1 }
    }

    fn here(&self) -> Pos {
        Pos::new(self.pos as u32, self.line, self.column)
    }

    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }
    fn peek_at(&self, off: usize) -> Option<u8> {
        self.src.get(self.pos + off).copied()
    }
    fn advance(&mut self) -> Option<u8> {
        let b = self.peek()?;
        self.pos += 1;
        if b == b'\n' { self.line += 1; self.column = 1; }
        else { self.column += 1; }
        Some(b)
    }

    /// Tokeniza todo el input. Trivia (whitespace, comentarios) se descarta.
    pub fn tokenize(&mut self) -> Result<Vec<Token>, LexError> {
        let mut tokens = Vec::new();
        while let Some(b) = self.peek() {
            // Whitespace
            if b.is_ascii_whitespace() {
                self.advance();
                continue;
            }
            // Comentarios
            if b == b'/' && self.peek_at(1) == Some(b'/') {
                while self.peek().map_or(false, |c| c != b'\n') { self.advance(); }
                continue;
            }
            if b == b'/' && self.peek_at(1) == Some(b'*') {
                self.advance(); self.advance();
                loop {
                    match (self.peek(), self.peek_at(1)) {
                        (Some(b'*'), Some(b'/')) => { self.advance(); self.advance(); break; }
                        (None, _) => return Err(LexError::UnterminatedString(self.here())),
                        _ => { self.advance(); }
                    }
                }
                continue;
            }
            // Identifiers y keywords
            if b.is_ascii_alphabetic() || b == b'_' {
                tokens.push(self.lex_ident_or_keyword()?);
                continue;
            }
            // Numbers
            if b.is_ascii_digit() {
                tokens.push(self.lex_number()?);
                continue;
            }
            // String
            if b == b'"' { tokens.push(self.lex_string()?); continue; }
            // Char
            if b == b'\'' { tokens.push(self.lex_char()?); continue; }
            // Operators / punctuation
            tokens.push(self.lex_punct()?);
        }
        tokens.push(Token::new(TokKind::Eof, String::new(), Span::point(self.here())));
        Ok(tokens)
    }

    fn lex_ident_or_keyword(&mut self) -> Result<Token, LexError> {
        let start = self.here();
        let mut s = String::new();
        while let Some(b) = self.peek() {
            if b.is_ascii_alphanumeric() || b == b'_' { s.push(b as char); self.advance(); }
            else { break; }
        }
        let kind = match s.as_str() {
            "fn" => TokKind::KwFn, "let" => TokKind::KwLet,
            "if" => TokKind::KwIf, "else" => TokKind::KwElse,
            "while" => TokKind::KwWhile, "for" => TokKind::KwFor,
            "return" => TokKind::KwReturn, "break" => TokKind::KwBreak,
            "continue" => TokKind::KwContinue, "loop" => TokKind::KwLoop,
            "switch" => TokKind::KwSwitch, "case" => TokKind::KwCase,
            "default" => TokKind::KwDefault, "struct" => TokKind::KwStruct,
            "union" => TokKind::KwUnion, "enum" => TokKind::KwEnum,
            "type" => TokKind::KwType,
            "true" => TokKind::BoolLit, "false" => TokKind::BoolLit,
            "as" => TokKind::KwAs, "use" => TokKind::KwUse,
            "extern" => TokKind::KwExtern, "static" => TokKind::KwStatic,
            "const" => TokKind::KwConst, "mut" => TokKind::KwMut,
            "null" => TokKind::KwNull, "sizeof" => TokKind::KwSizeOf,
            "label" => TokKind::KwLabel, "goto" => TokKind::KwGoto,
            _ => TokKind::Ident,
        };
        Ok(Token::new(kind, s, Span::new(start, self.here())))
    }

    fn lex_number(&mut self) -> Result<Token, LexError> {
        let start = self.here();
        let mut s = String::new();
        let mut is_float = false;
        while let Some(b) = self.peek() {
            if b.is_ascii_digit() { s.push(b as char); self.advance(); }
            else if b == b'.' && self.peek_at(1).map_or(false, |c| c.is_ascii_digit()) {
                is_float = true;
                s.push(b as char); self.advance();
            } else { break; }
        }
        // Sufijo opcional (u32, i64, f64)
        if let Some(b) = self.peek() {
            if b == b'u' || b == b'i' || b == b'f' {
                s.push(b as char); self.advance();
                while let Some(c) = self.peek() {
                    if c.is_ascii_digit() { s.push(c as char); self.advance(); }
                    else { break; }
                }
            }
        }
        let kind = if is_float { TokKind::FloatLit } else { TokKind::IntLit };
        Ok(Token::new(kind, s, Span::new(start, self.here())))
    }

    fn lex_string(&mut self) -> Result<Token, LexError> {
        let start = self.here();
        self.advance(); // consume "
        let mut s = String::new();
        loop {
            match self.advance() {
                Some(b'"') => break,
                Some(b'\\') => {
                    match self.advance() {
                        Some(b'n') => s.push('\n'),
                        Some(b't') => s.push('\t'),
                        Some(b'r') => s.push('\r'),
                        Some(b'\\') => s.push('\\'),
                        Some(b'"') => s.push('"'),
                        Some(b'\'') => s.push('\''),
                        Some(b'0') => s.push('\0'),
                        Some(c) => return Err(LexError::InvalidEscape(c as char, self.here())),
                        None => return Err(LexError::UnterminatedString(start)),
                    }
                }
                Some(b) => s.push(b as char),
                None => return Err(LexError::UnterminatedString(start)),
            }
        }
        Ok(Token::new(TokKind::StrLit, s, Span::new(start, self.here())))
    }

    fn lex_char(&mut self) -> Result<Token, LexError> {
        let start = self.here();
        self.advance(); // consume '
        let _v = match self.advance() {
            Some(b'\\') => match self.advance() {
                Some(b'n') => b'\n' as u32,
                Some(b't') => b'\t' as u32,
                Some(b'r') => b'\r' as u32,
                Some(b'\\') => b'\\' as u32,
                Some(b'\'') => b'\'' as u32,
                Some(b'0') => 0,
                Some(c) => return Err(LexError::InvalidEscape(c as char, self.here())),
                None => return Err(LexError::UnterminatedChar(start)),
            },
            Some(b) => b as u32,
            None => return Err(LexError::UnterminatedChar(start)),
        };
        if self.advance() != Some(b'\'') {
            return Err(LexError::UnterminatedChar(start));
        }
        Ok(Token::new(TokKind::CharLit, String::new(), Span::new(start, self.here())))
    }

    fn lex_punct(&mut self) -> Result<Token, LexError> {
        let start = self.here();
        let b = self.advance().unwrap();
        let (kind, text) = match b {
            b'(' => (TokKind::LParen, "("),
            b')' => (TokKind::RParen, ")"),
            b'{' => (TokKind::LBrace, "{"),
            b'}' => (TokKind::RBrace, "}"),
            b'[' => (TokKind::LBracket, "["),
            b']' => (TokKind::RBracket, "]"),
            b',' => (TokKind::Comma, ","),
            b';' => (TokKind::Semi, ";"),
            b'.' => (TokKind::Dot, "."),
            b':' => (TokKind::Colon, ":"),
            b'@' => (TokKind::At, "@"),
            b'#' => (TokKind::Hash, "#"),
            b'$' => (TokKind::Dollar, "$"),
            b'?' => (TokKind::Question, "?"),
            b'+' => match self.peek() {
                Some(b'=') => { self.advance(); (TokKind::PlusEq, "+=") }
                _ => (TokKind::Plus, "+"),
            },
            b'-' => match self.peek() {
                Some(b'>') => { self.advance(); (TokKind::Arrow, "->") }
                Some(b'=') => { self.advance(); (TokKind::MinusEq, "-=") }
                _ => (TokKind::Minus, "-"),
            },
            b'*' => match self.peek() {
                Some(b'=') => { self.advance(); (TokKind::StarEq, "*=") }
                _ => (TokKind::Star, "*"),
            },
            b'/' => match self.peek() {
                Some(b'=') => { self.advance(); (TokKind::SlashEq, "/=") }
                _ => (TokKind::Slash, "/"),
            },
            b'%' => match self.peek() {
                Some(b'=') => { self.advance(); (TokKind::PercentEq, "%=") }
                _ => (TokKind::Percent, "%"),
            },
            b'&' => match self.peek() {
                Some(b'&') => { self.advance(); (TokKind::And, "&&") }
                Some(b'=') => { self.advance(); (TokKind::AmpEq, "&=") }
                _ => (TokKind::Amp, "&"),
            },
            b'|' => match self.peek() {
                Some(b'|') => { self.advance(); (TokKind::Or, "||") }
                Some(b'=') => { self.advance(); (TokKind::PipeEq, "|=") }
                _ => (TokKind::Pipe, "|"),
            },
            b'^' => match self.peek() {
                Some(b'=') => { self.advance(); (TokKind::CaretEq, "^=") }
                _ => (TokKind::Caret, "^"),
            },
            b'~' => (TokKind::Tilde, "~"),
            b'!' => match self.peek() {
                Some(b'=') => { self.advance(); (TokKind::NotEq, "!=") }
                _ => (TokKind::Bang, "!"),
            },
            b'=' => match self.peek() {
                Some(b'=') => { self.advance(); (TokKind::EqEq, "==") }
                Some(b'>') => { self.advance(); (TokKind::FatArrow, "=>") }
                _ => (TokKind::Eq, "="),
            },
            b'<' => match self.peek() {
                Some(b'=') => { self.advance(); (TokKind::LtEq, "<=") }
                Some(b'<') => {
                    self.advance();
                    match self.peek() {
                        Some(b'=') => { self.advance(); (TokKind::ShlEq, "<<=") }
                        _ => (TokKind::Shl, "<<"),
                    }
                }
                _ => (TokKind::Lt, "<"),
            },
            b'>' => match self.peek() {
                Some(b'=') => { self.advance(); (TokKind::GtEq, ">=") }
                Some(b'>') => {
                    self.advance();
                    match self.peek() {
                        Some(b'=') => { self.advance(); (TokKind::ShrEq, ">>=") }
                        _ => (TokKind::Shr, ">>"),
                    }
                }
                _ => (TokKind::Gt, ">"),
            },
            _ => return Err(LexError::UnexpectedChar(b as char, start)),
        };
        Ok(Token::new(kind, text.into(), Span::new(start, self.here())))
    }
}
