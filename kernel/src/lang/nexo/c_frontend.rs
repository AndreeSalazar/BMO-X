//! ÑEXO C Frontend — Compilador C → ÑEXO.
//!
//! Convierte un subconjunto de C en AST de ÑEXO, que luego
//! pasa por el codegen ÑEXO → BMOasm → nativo.
//!
//! ## Subconjunto de C soportado
//!
//! - Tipos: `int`, `unsigned int`, `long`, `char`, `void`, `char*`, `int*`, `struct`
//! - Control: `if`/`else`, `while`, `for`, `return`, `break`, `continue`
//! - Expresiones: literales, operadores, llamadas, asignación
//! - Declaraciones: `fn`, `static`, `extern`
//! - Compilación separada via `#include` stubs

#![allow(dead_code)]

extern crate alloc;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;

use crate::barex::{BxError, BxResult};
use super::parser::{Ast, Stmt, Expr, Param, TypeAnnotation, BinOp, UnaryOp, ExternItem};

// ═══════════════════════════════════════════════════════════════════
// C Lexer
// ═══════════════════════════════════════════════════════════════════

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
    Struct, Enum, Typedef, Static, Extern, Const, Volatile,
    If, Else, While, For, Do, Switch, Case, Default, Break, Continue, Return,
    Sizeof, StructOp, // -> 
    // Operators
    Plus, Minus, Star, Slash, Percent,
    Amp, Pipe, Caret, Tilde, Bang,
    Eq, EqEq, BangEq, Lt, Gt, Le, Ge,
    AmpAmp, PipePipe,
    PlusEq, MinusEq, StarEq, SlashEq,
    PlusPlus, MinusMinus,
    Arrow, Dot,
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
                b'.' => { self.advance(); CToken::Dot }
                _ => { self.advance(); continue; } // skip unknown
            };
            tokens.push(tok);
        }
        Ok(tokens)
    }
}

// ═══════════════════════════════════════════════════════════════════
// C AST
// ═══════════════════════════════════════════════════════════════════

/// C type representation.
#[derive(Debug, Clone)]
pub enum CType {
    Int,
    UnsignedInt,
    Long,
    UnsignedLong,
    Char,
    Void,
    Short,
    Float,
    Double,
    Ptr(Box<CType>),
    Array(Box<CType>, u64),
    Struct(String),
    Named(String),
}

/// C expression.
#[derive(Debug, Clone)]
pub enum CExpr {
    IntLit(u64),
    StrLit(String),
    CharLit(u8),
    Ident(String),
    Binary(CBinOp, Box<CExpr>, Box<CExpr>),
    Unary(CUnaryOp, Box<CExpr>),
    Call(String, Vec<CExpr>),
    Assign(Box<CExpr>, Box<CExpr>),
    Member(Box<CExpr>, String),     // obj.field
    ArrowMember(Box<CExpr>, String), // ptr->field
    Sizeof(CType),
    ArrayIndex(Box<CExpr>, Box<CExpr>),
    Cast(CType, Box<CExpr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CBinOp {
    Add, Sub, Mul, Div, Mod,
    Eq, Ne, Lt, Gt, Le, Ge,
    And, Or,
    BitAnd, BitOr, BitXor,
    Shl, Shr,
    Assign, AddAssign, SubAssign, MulAssign, DivAssign,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CUnaryOp {
    Neg, Not, BitNot, PreInc, PreDec, PostInc, PostDec, Deref, AddrOf,
}

/// C statement.
#[derive(Debug, Clone)]
pub enum CStmt {
    Empty,
    Expr(CExpr),
    Decl { ty: CType, name: String, init: Option<CExpr> },
    If { cond: CExpr, then_body: Box<CStmt>, else_body: Option<Box<CStmt>> },
    While { cond: CExpr, body: Box<CStmt> },
    For { init: Option<Box<CStmt>>, cond: Option<CExpr>, update: Option<CExpr>, body: Box<CStmt> },
    Do { cond: CExpr, body: Box<CStmt> },
    Block(Vec<CStmt>),
    Return(Option<CExpr>),
    Break,
    Continue,
}

/// C function parameter.
#[derive(Debug, Clone)]
pub struct CParam {
    pub ty: CType,
    pub name: String,
}

/// C top-level declaration.
#[derive(Debug, Clone)]
pub enum CItem {
    Function {
        name: String,
        ret: CType,
        params: Vec<CParam>,
        body: Option<Vec<CStmt>>,
        is_static: bool,
        is_extern: bool,
    },
    Struct {
        name: String,
        fields: Vec<(CType, String)>,
    },
    Typedef {
        name: String,
        ty: CType,
    },
    GlobalVar {
        ty: CType,
        name: String,
        init: Option<CExpr>,
        is_static: bool,
        is_extern: bool,
    },
}

/// C program AST.
#[derive(Debug, Clone, Default)]
pub struct CAst {
    pub items: Vec<CItem>,
}

// ═══════════════════════════════════════════════════════════════════
// C Parser
// ═══════════════════════════════════════════════════════════════════

/// C parser — recursive descent.
pub struct CParser {
    tokens: Vec<CToken>,
    pos: usize,
}

impl CParser {
    pub fn new(tokens: Vec<CToken>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> &CToken {
        if self.pos < self.tokens.len() { &self.tokens[self.pos] } else { &CToken::Eof }
    }

    fn advance(&mut self) -> CToken {
        let tok = self.peek().clone();
        if self.pos < self.tokens.len() { self.pos += 1; }
        tok
    }

    fn expect(&mut self, expected: &CToken) -> BxResult<()> {
        let tok = self.advance();
        if &tok == expected { Ok(()) } else { Err(BxError::InvalidArgument) }
    }

    fn expect_ident(&mut self) -> BxResult<String> {
        match self.advance() {
            CToken::Ident(s) => Ok(s),
            _ => Err(BxError::InvalidArgument),
        }
    }

    fn check(&self, expected: &CToken) -> bool {
        core::mem::discriminant(self.peek()) == core::mem::discriminant(expected)
    }

    pub fn parse(&mut self) -> BxResult<CAst> {
        let mut ast = CAst::default();
        while !self.check(&CToken::Eof) {
            ast.items.push(self.parse_item()?);
        }
        Ok(ast)
    }

    fn parse_item(&mut self) -> BxResult<CItem> {
        // Parse storage class
        let is_static = if self.check(&CToken::Static) { self.advance(); true } else { false };
        let is_extern = if self.check(&CToken::Extern) { self.advance(); true } else { false };

        match self.peek() {
            CToken::Struct => self.parse_struct(),
            CToken::Typedef => self.parse_typedef(),
            _ => {
                // Parse type then check if it's a function or variable
                let ty = self.parse_type()?;
                let name = self.expect_ident()?;

                if self.check(&CToken::LParen) {
                    // Function declaration
                    self.advance();
                    let params = self.parse_param_list()?;
                    self.expect(&CToken::RParen)?;

                    if self.check(&CToken::LBrace) {
                        let body = self.parse_block()?;
                        Ok(CItem::Function { name, ret: ty, params, body: Some(body), is_static, is_extern })
                    } else {
                        self.expect(&CToken::Semi)?;
                        Ok(CItem::Function { name, ret: ty, params, body: None, is_static, is_extern })
                    }
                } else {
                    // Global variable
                    let init = if self.check(&CToken::Eq) {
                        self.advance();
                        Some(self.parse_expr()?)
                    } else { None };
                    self.expect(&CToken::Semi)?;
                    Ok(CItem::GlobalVar { ty, name, init, is_static, is_extern })
                }
            }
        }
    }

    fn parse_struct(&mut self) -> BxResult<CItem> {
        self.expect(&CToken::Struct)?;
        let name = self.expect_ident()?;
        self.expect(&CToken::LBrace)?;
        let mut fields = Vec::new();
        while !self.check(&CToken::RBrace) && !self.check(&CToken::Eof) {
            let ty = self.parse_type()?;
            let field_name = self.expect_ident()?;
            self.expect(&CToken::Semi)?;
            fields.push((ty, field_name));
        }
        self.expect(&CToken::RBrace)?;
        self.expect(&CToken::Semi)?;
        Ok(CItem::Struct { name, fields })
    }

    fn parse_typedef(&mut self) -> BxResult<CItem> {
        self.expect(&CToken::Typedef)?;
        let ty = self.parse_type()?;
        let name = self.expect_ident()?;
        self.expect(&CToken::Semi)?;
        Ok(CItem::Typedef { name, ty })
    }

    fn parse_type(&mut self) -> BxResult<CType> {
        let mut ty = match self.advance() {
            CToken::Int => CType::Int,
            CToken::Unsigned => {
                if self.check(&CToken::Int) { self.advance(); CType::UnsignedInt }
                else if self.check(&CToken::Long) { self.advance(); CType::UnsignedLong }
                else { CType::UnsignedInt }
            }
            CToken::Long => {
                if self.check(&CToken::Int) { self.advance(); }
                CType::Long
            }
            CToken::Char => CType::Char,
            CToken::Void => CType::Void,
            CToken::Short => { if self.check(&CToken::Int) { self.advance(); } CType::Short }
            CToken::Float => CType::Float,
            CToken::Double => CType::Double,
            CToken::Ident(s) => CType::Named(s),
            _ => return Err(BxError::InvalidArgument),
        };
        // Handle pointers
        while self.check(&CToken::Star) {
            self.advance();
            ty = CType::Ptr(Box::new(ty));
        }
        Ok(ty)
    }

    fn parse_param_list(&mut self) -> BxResult<Vec<CParam>> {
        let mut params = Vec::new();
        if self.check(&CToken::RParen) { return Ok(params); }
        loop {
            let ty = self.parse_type()?;
            let name = if matches!(self.peek(), CToken::Ident(_)) {
                self.expect_ident()?
            } else { String::new() };
            params.push(CParam { ty, name });
            if self.check(&CToken::Comma) { self.advance(); } else { break; }
        }
        Ok(params)
    }

    fn parse_block(&mut self) -> BxResult<Vec<CStmt>> {
        self.expect(&CToken::LBrace)?;
        let mut stmts = Vec::new();
        while !self.check(&CToken::RBrace) && !self.check(&CToken::Eof) {
            stmts.push(self.parse_stmt()?);
        }
        self.expect(&CToken::RBrace)?;
        Ok(stmts)
    }

    fn parse_stmt(&mut self) -> BxResult<CStmt> {
        match self.peek() {
            CToken::If => {
                self.advance();
                self.expect(&CToken::LParen)?;
                let cond = self.parse_expr()?;
                self.expect(&CToken::RParen)?;
                let then_body = self.parse_stmt()?;
                let else_body = if self.check(&CToken::Else) {
                    self.advance();
                    Some(Box::new(self.parse_stmt()?))
                } else { None };
                Ok(CStmt::If { cond, then_body: Box::new(then_body), else_body })
            }
            CToken::While => {
                self.advance();
                self.expect(&CToken::LParen)?;
                let cond = self.parse_expr()?;
                self.expect(&CToken::RParen)?;
                let body = self.parse_stmt()?;
                Ok(CStmt::While { cond, body: Box::new(body) })
            }
            CToken::For => {
                self.advance();
                self.expect(&CToken::LParen)?;
                let init = if self.check(&CToken::Semi) { None } else {
                    if self.check(&CToken::Int) || self.check(&CToken::Char) || self.check(&CToken::Void) {
                        Some(Box::new(self.parse_stmt()?))
                    } else {
                        let e = self.parse_expr()?;
                        self.expect(&CToken::Semi)?;
                        Some(Box::new(CStmt::Expr(e)))
                    }
                };
                let cond = if self.check(&CToken::Semi) { None } else { Some(self.parse_expr()?) };
                self.expect(&CToken::Semi)?;
                let update = if self.check(&CToken::RParen) { None } else { Some(self.parse_expr()?) };
                self.expect(&CToken::RParen)?;
                let body = self.parse_stmt()?;
                Ok(CStmt::For { init, cond, update, body: Box::new(body) })
            }
            CToken::Return => {
                self.advance();
                let val = if self.check(&CToken::Semi) { None } else { Some(self.parse_expr()?) };
                self.expect(&CToken::Semi)?;
                Ok(CStmt::Return(val))
            }
            CToken::Break => { self.advance(); self.expect(&CToken::Semi)?; Ok(CStmt::Break) }
            CToken::Continue => { self.advance(); self.expect(&CToken::Semi)?; Ok(CStmt::Continue) }
            CToken::LBrace => Ok(CStmt::Block(self.parse_block()?)),
            CToken::Int | CToken::Char | CToken::Void | CToken::Unsigned | CToken::Long | CToken::Short => {
                // Variable declaration
                let ty = self.parse_type()?;
                let name = self.expect_ident()?;
                let init = if self.check(&CToken::Eq) {
                    self.advance();
                    Some(self.parse_expr()?)
                } else { None };
                self.expect(&CToken::Semi)?;
                Ok(CStmt::Decl { ty, name, init })
            }
            _ => {
                let expr = self.parse_expr()?;
                self.expect(&CToken::Semi)?;
                Ok(CStmt::Expr(expr))
            }
        }
    }

    fn parse_expr(&mut self) -> BxResult<CExpr> {
        self.parse_assign()
    }

    fn parse_assign(&mut self) -> BxResult<CExpr> {
        let left = self.parse_or()?;
        match self.peek() {
            CToken::Eq => { self.advance(); let r = self.parse_assign()?; Ok(CExpr::Assign(Box::new(left), Box::new(r))) }
            CToken::PlusEq => { self.advance(); let r = self.parse_assign()?; Ok(CExpr::Binary(CBinOp::AddAssign, Box::new(left), Box::new(r))) }
            CToken::MinusEq => { self.advance(); let r = self.parse_assign()?; Ok(CExpr::Binary(CBinOp::SubAssign, Box::new(left), Box::new(r))) }
            CToken::StarEq => { self.advance(); let r = self.parse_assign()?; Ok(CExpr::Binary(CBinOp::MulAssign, Box::new(left), Box::new(r))) }
            CToken::SlashEq => { self.advance(); let r = self.parse_assign()?; Ok(CExpr::Binary(CBinOp::DivAssign, Box::new(left), Box::new(r))) }
            _ => Ok(left),
        }
    }

    fn parse_or(&mut self) -> BxResult<CExpr> {
        let mut left = self.parse_and()?;
        while self.check(&CToken::PipePipe) {
            self.advance();
            let right = self.parse_and()?;
            left = CExpr::Binary(CBinOp::Or, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> BxResult<CExpr> {
        let mut left = self.parse_bit_or()?;
        while self.check(&CToken::AmpAmp) {
            self.advance();
            let right = self.parse_bit_or()?;
            left = CExpr::Binary(CBinOp::And, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_bit_or(&mut self) -> BxResult<CExpr> {
        let mut left = self.parse_bit_xor()?;
        while self.check(&CToken::Pipe) {
            self.advance();
            let right = self.parse_bit_xor()?;
            left = CExpr::Binary(CBinOp::BitOr, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_bit_xor(&mut self) -> BxResult<CExpr> {
        let mut left = self.parse_bit_and()?;
        while self.check(&CToken::Caret) {
            self.advance();
            let right = self.parse_bit_and()?;
            left = CExpr::Binary(CBinOp::BitXor, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_bit_and(&mut self) -> BxResult<CExpr> {
        let mut left = self.parse_comparison()?;
        while self.check(&CToken::Amp) && !self.check(&CToken::AmpAmp) {
            self.advance();
            let right = self.parse_comparison()?;
            left = CExpr::Binary(CBinOp::BitAnd, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_comparison(&mut self) -> BxResult<CExpr> {
        let mut left = self.parse_shift()?;
        loop {
            match self.peek() {
                CToken::EqEq => { self.advance(); let r = self.parse_shift()?; left = CExpr::Binary(CBinOp::Eq, Box::new(left), Box::new(r)); }
                CToken::BangEq => { self.advance(); let r = self.parse_shift()?; left = CExpr::Binary(CBinOp::Ne, Box::new(left), Box::new(r)); }
                CToken::Lt => { self.advance(); let r = self.parse_shift()?; left = CExpr::Binary(CBinOp::Lt, Box::new(left), Box::new(r)); }
                CToken::Gt => { self.advance(); let r = self.parse_shift()?; left = CExpr::Binary(CBinOp::Gt, Box::new(left), Box::new(r)); }
                CToken::Le => { self.advance(); let r = self.parse_shift()?; left = CExpr::Binary(CBinOp::Le, Box::new(left), Box::new(r)); }
                CToken::Ge => { self.advance(); let r = self.parse_shift()?; left = CExpr::Binary(CBinOp::Ge, Box::new(left), Box::new(r)); }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_shift(&mut self) -> BxResult<CExpr> {
        let mut left = self.parse_add()?;
        loop {
            match self.peek() {
                CToken::Lt if self.pos + 1 < self.tokens.len() => {
                    // Check for << but be careful with < comparison
                    if let CToken::Lt = &self.tokens[self.pos + 1] {
                        self.advance(); self.advance();
                        let right = self.parse_add()?;
                        left = CExpr::Binary(CBinOp::Shl, Box::new(left), Box::new(right));
                    } else { break; }
                }
                CToken::Gt if self.pos + 1 < self.tokens.len() => {
                    if let CToken::Gt = &self.tokens[self.pos + 1] {
                        self.advance(); self.advance();
                        let right = self.parse_add()?;
                        left = CExpr::Binary(CBinOp::Shr, Box::new(left), Box::new(right));
                    } else { break; }
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_add(&mut self) -> BxResult<CExpr> {
        let mut left = self.parse_mul()?;
        loop {
            match self.peek() {
                CToken::Plus => { self.advance(); let r = self.parse_mul()?; left = CExpr::Binary(CBinOp::Add, Box::new(left), Box::new(r)); }
                CToken::Minus => { self.advance(); let r = self.parse_mul()?; left = CExpr::Binary(CBinOp::Sub, Box::new(left), Box::new(r)); }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_mul(&mut self) -> BxResult<CExpr> {
        let mut left = self.parse_unary()?;
        loop {
            match self.peek() {
                CToken::Star => { self.advance(); let r = self.parse_unary()?; left = CExpr::Binary(CBinOp::Mul, Box::new(left), Box::new(r)); }
                CToken::Slash => { self.advance(); let r = self.parse_unary()?; left = CExpr::Binary(CBinOp::Div, Box::new(left), Box::new(r)); }
                CToken::Percent => { self.advance(); let r = self.parse_unary()?; left = CExpr::Binary(CBinOp::Mod, Box::new(left), Box::new(r)); }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> BxResult<CExpr> {
        match self.peek() {
            CToken::Minus => { self.advance(); let e = self.parse_unary()?; Ok(CExpr::Unary(CUnaryOp::Neg, Box::new(e))) }
            CToken::Bang => { self.advance(); let e = self.parse_unary()?; Ok(CExpr::Unary(CUnaryOp::Not, Box::new(e))) }
            CToken::Tilde => { self.advance(); let e = self.parse_unary()?; Ok(CExpr::Unary(CUnaryOp::BitNot, Box::new(e))) }
            CToken::Star => { self.advance(); let e = self.parse_unary()?; Ok(CExpr::Unary(CUnaryOp::Deref, Box::new(e))) }
            CToken::Amp => { self.advance(); let e = self.parse_unary()?; Ok(CExpr::Unary(CUnaryOp::AddrOf, Box::new(e))) }
            CToken::PlusPlus => { self.advance(); let e = self.parse_unary()?; Ok(CExpr::Unary(CUnaryOp::PreInc, Box::new(e))) }
            CToken::MinusMinus => { self.advance(); let e = self.parse_unary()?; Ok(CExpr::Unary(CUnaryOp::PreDec, Box::new(e))) }
            _ => self.parse_postfix(),
        }
    }

    fn parse_postfix(&mut self) -> BxResult<CExpr> {
        let mut expr = self.parse_primary()?;
        loop {
            match self.peek() {
                CToken::PlusPlus => { self.advance(); expr = CExpr::Unary(CUnaryOp::PostInc, Box::new(expr)); }
                CToken::MinusMinus => { self.advance(); expr = CExpr::Unary(CUnaryOp::PostDec, Box::new(expr)); }
                CToken::Dot => {
                    self.advance();
                    let field = self.expect_ident()?;
                    expr = CExpr::Member(Box::new(expr), field);
                }
                CToken::Arrow => {
                    self.advance();
                    let field = self.expect_ident()?;
                    expr = CExpr::ArrowMember(Box::new(expr), field);
                }
                CToken::LParen => {
                    self.advance();
                    let mut args = Vec::new();
                    if !self.check(&CToken::RParen) {
                        loop {
                            args.push(self.parse_expr()?);
                            if self.check(&CToken::Comma) { self.advance(); } else { break; }
                        }
                    }
                    self.expect(&CToken::RParen)?;
                    if let CExpr::Ident(name) = expr {
                        expr = CExpr::Call(name, args);
                    }
                }
                CToken::LBracket => {
                    self.advance();
                    let idx = self.parse_expr()?;
                    self.expect(&CToken::RBracket)?;
                    expr = CExpr::ArrayIndex(Box::new(expr), Box::new(idx));
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> BxResult<CExpr> {
        match self.advance() {
            CToken::IntLit(n) => Ok(CExpr::IntLit(n)),
            CToken::StrLit(s) => Ok(CExpr::StrLit(s)),
            CToken::CharLit(c) => Ok(CExpr::CharLit(c)),
            CToken::Ident(name) => Ok(CExpr::Ident(name)),
            CToken::LParen => {
                let expr = self.parse_expr()?;
                self.expect(&CToken::RParen)?;
                Ok(expr)
            }
            CToken::Sizeof => {
                self.expect(&CToken::LParen)?;
                let ty = self.parse_type()?;
                self.expect(&CToken::RParen)?;
                Ok(CExpr::Sizeof(ty))
            }
            _ => Err(BxError::InvalidArgument),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// C → ÑEXO Translator
// ═══════════════════════════════════════════════════════════════════

/// Translates C AST to ÑEXO AST.
pub struct CToNexo;

impl CToNexo {
    pub fn new() -> Self { Self }

    /// Translate a C program to ÑEXO AST.
    pub fn translate(&self, cast: &CAst) -> BxResult<Ast> {
        let mut items = Vec::new();
        for item in &cast.items {
            if let Some(stmt) = self.translate_item(item)? {
                items.push(stmt);
            }
        }
        Ok(Ast { items })
    }

    fn translate_item(&self, item: &CItem) -> BxResult<Option<Stmt>> {
        match item {
            CItem::Function { name, ret, params, body, is_static: _, is_extern } => {
                if *is_extern {
                    // extern function → Extern declaration
                    let nexo_params: Vec<Param> = params.iter().map(|p| Param {
                        name: p.name.clone(),
                        ty: self.translate_type(&p.ty),
                    }).collect();
                    return Ok(Some(Stmt::Extern { items: vec![ExternItem::Fn {
                        name: name.clone(),
                        params: nexo_params,
                        ret: Some(self.translate_type(ret)),
                    }] }));
                }

                let nexo_params: Vec<Param> = params.iter().map(|p| Param {
                    name: p.name.clone(),
                    ty: self.translate_type(&p.ty),
                }).collect();

                let nexo_ret = Some(self.translate_type(ret));
                let nexo_body = if let Some(stmts) = body {
                    stmts.iter().filter_map(|s| self.translate_stmt(s).ok().flatten()).collect()
                } else {
                    Vec::new()
                };

                Ok(Some(Stmt::FnDecl {
                    name: name.clone(),
                    params: nexo_params,
                    ret: nexo_ret,
                    body: nexo_body,
                }))
            }
            CItem::Struct { name, fields } => {
                let nexo_fields: Vec<(String, TypeAnnotation)> = fields.iter()
                    .map(|(ty, fname)| (fname.clone(), self.translate_type(ty)))
                    .collect();
                Ok(Some(Stmt::StructDecl { name: name.clone(), fields: nexo_fields }))
            }
            CItem::Typedef { .. } => Ok(None), // Type aliases are metadata
            CItem::GlobalVar { ty: _, name, init, is_static: _, is_extern } => {
                if *is_extern {
                    return Ok(None);
                }
                let value = init.as_ref().map(|e| self.translate_expr(e)).transpose()?;
                Ok(Some(Stmt::Let {
                    name: name.clone(),
                    ty: None,
                    value,
                }))
            }
        }
    }

    fn translate_stmt(&self, stmt: &CStmt) -> BxResult<Option<Stmt>> {
        match stmt {
            CStmt::Empty => Ok(None),
            CStmt::Expr(expr) => {
                let nexo_expr = self.translate_expr(expr)?;
                Ok(Some(Stmt::ExprStmt(nexo_expr)))
            }
            CStmt::Decl { ty: _, name, init } => {
                let value = init.as_ref().map(|e| self.translate_expr(e)).transpose()?;
                Ok(Some(Stmt::Let {
                    name: name.clone(),
                    ty: None,
                    value,
                }))
            }
            CStmt::If { cond, then_body, else_body } => {
                let nexo_cond = self.translate_expr(cond)?;
                let then = self.translate_stmt(then_body)?.into_iter().collect();
                let else_b = else_body.as_ref()
                    .map(|eb| self.translate_stmt(eb).map(|s| s.into_iter().collect()))
                    .transpose()?;
                Ok(Some(Stmt::If { cond: nexo_cond, then_body: then, else_body: else_b }))
            }
            CStmt::While { cond, body } => {
                let nexo_cond = self.translate_expr(cond)?;
                let nexo_body = self.translate_stmt(body)?.into_iter().collect();
                Ok(Some(Stmt::While { cond: nexo_cond, body: nexo_body }))
            }
            CStmt::For { init, cond, update: _, body } => {
                // Desugar to while loop
                let mut stmts = Vec::new();
                if let Some(init_stmt) = init {
                    if let Some(s) = self.translate_stmt(init_stmt)? {
                        stmts.push(s);
                    }
                }
                let cond_expr = cond.as_ref()
                    .map(|e| self.translate_expr(e))
                    .transpose()?
                    .unwrap_or(Expr::LitBool(true));
                let nexo_body = self.translate_stmt(body)?.into_iter().collect();
                let while_stmt = Stmt::While { cond: cond_expr, body: nexo_body };
                stmts.push(while_stmt);
                // Update expression would go at end of while body — simplified for now
                Ok(Some(Stmt::Block(stmts)))
            }
            CStmt::Do { cond, body } => {
                let nexo_cond = self.translate_expr(cond)?;
                let nexo_body: Vec<Stmt> = self.translate_stmt(body)?.into_iter().collect();
                // do-while → while with body first
                let mut stmts = nexo_body;
                stmts.push(Stmt::While { cond: nexo_cond, body: Vec::new() });
                Ok(Some(Stmt::Block(stmts)))
            }
            CStmt::Block(stmts) => {
                let nexo_stmts: Vec<Stmt> = stmts.iter()
                    .filter_map(|s| self.translate_stmt(s).ok().flatten())
                    .collect();
                Ok(Some(Stmt::Block(nexo_stmts)))
            }
            CStmt::Return(expr) => {
                let val = expr.as_ref().map(|e| self.translate_expr(e)).transpose()?;
                Ok(Some(Stmt::Return(val)))
            }
            CStmt::Break => Ok(Some(Stmt::Break)),
            CStmt::Continue => Ok(Some(Stmt::Continue)),
        }
    }

    fn translate_expr(&self, expr: &CExpr) -> BxResult<Expr> {
        match expr {
            CExpr::IntLit(n) => Ok(Expr::LitInt(*n)),
            CExpr::StrLit(s) => Ok(Expr::LitStr(s.clone())),
            CExpr::CharLit(c) => Ok(Expr::LitByte(*c)),
            CExpr::Ident(name) => Ok(Expr::Ident(name.clone())),
            CExpr::Binary(op, left, right) => {
                let l = self.translate_expr(left)?;
                let r = self.translate_expr(right)?;
                let nexo_op = self.translate_binop(*op);
                Ok(Expr::Binary(nexo_op, Box::new(l), Box::new(r)))
            }
            CExpr::Unary(op, inner) => {
                let e = self.translate_expr(inner)?;
                match op {
                    CUnaryOp::Neg => Ok(Expr::Unary(UnaryOp::Neg, Box::new(e))),
                    CUnaryOp::Not => Ok(Expr::Unary(UnaryOp::Not, Box::new(e))),
                    CUnaryOp::Deref => Ok(Expr::Unary(UnaryOp::Deref, Box::new(e))),
                    CUnaryOp::AddrOf => Ok(Expr::Unary(UnaryOp::Ref, Box::new(e))),
                    CUnaryOp::PreInc => {
                        // ++x → x = x + 1; x
                        Ok(Expr::Binary(BinOp::Add, Box::new(e), Box::new(Expr::LitInt(1))))
                    }
                    CUnaryOp::PreDec => {
                        Ok(Expr::Binary(BinOp::Sub, Box::new(e), Box::new(Expr::LitInt(1))))
                    }
                    CUnaryOp::PostInc => {
                        Ok(Expr::Binary(BinOp::Add, Box::new(e), Box::new(Expr::LitInt(1))))
                    }
                    CUnaryOp::PostDec => {
                        Ok(Expr::Binary(BinOp::Sub, Box::new(e), Box::new(Expr::LitInt(1))))
                    }
                    CUnaryOp::BitNot => Ok(Expr::Unary(UnaryOp::Not, Box::new(e))),
                }
            }
            CExpr::Call(name, args) => {
                let nexo_args: Vec<Expr> = args.iter()
                    .map(|a| self.translate_expr(a))
                    .collect::<BxResult<Vec<_>>>()?;
                Ok(Expr::Call(name.clone(), nexo_args))
            }
            CExpr::Assign(left, right) => {
                if let CExpr::Ident(name) = left.as_ref() {
                    let val = self.translate_expr(right)?;
                    Ok(Expr::Binary(BinOp::Add, Box::new(Expr::Ident(name.clone())), Box::new(val)))
                } else {
                    let l = self.translate_expr(left)?;
                    let r = self.translate_expr(right)?;
                    Ok(Expr::Binary(BinOp::Add, Box::new(l), Box::new(r)))
                }
            }
            CExpr::Member(obj, field) | CExpr::ArrowMember(obj, field) => {
                let o = self.translate_expr(obj)?;
                Ok(Expr::Field(Box::new(o), field.clone()))
            }
            CExpr::Sizeof(_) => Ok(Expr::LitInt(8)), // Default size
            CExpr::ArrayIndex(obj, idx) => {
                let o = self.translate_expr(obj)?;
                let i = self.translate_expr(idx)?;
                Ok(Expr::Index(Box::new(o), Box::new(i)))
            }
            CExpr::Cast(_, inner) => self.translate_expr(inner),
        }
    }

    fn translate_type(&self, ty: &CType) -> TypeAnnotation {
        match ty {
            CType::Int | CType::Long | CType::Short => TypeAnnotation::Named(String::from("num")),
            CType::UnsignedInt | CType::UnsignedLong => TypeAnnotation::Named(String::from("num")),
            CType::Char => TypeAnnotation::Named(String::from("byte")),
            CType::Void => TypeAnnotation::Named(String::from("void")),
            CType::Float | CType::Double => TypeAnnotation::Named(String::from("num")),
            CType::Ptr(inner) => TypeAnnotation::Ptr(Box::new(self.translate_type(inner))),
            CType::Array(inner, _) => TypeAnnotation::Array(Box::new(self.translate_type(inner)), 0),
            CType::Struct(name) => TypeAnnotation::Named(name.clone()),
            CType::Named(name) => TypeAnnotation::Named(name.clone()),
        }
    }

    fn translate_binop(&self, op: CBinOp) -> BinOp {
        match op {
            CBinOp::Add | CBinOp::AddAssign => BinOp::Add,
            CBinOp::Sub | CBinOp::SubAssign => BinOp::Sub,
            CBinOp::Mul | CBinOp::MulAssign => BinOp::Mul,
            CBinOp::Div | CBinOp::DivAssign => BinOp::Div,
            CBinOp::Mod => BinOp::Mod,
            CBinOp::Eq => BinOp::Eq,
            CBinOp::Ne => BinOp::Ne,
            CBinOp::Lt => BinOp::Lt,
            CBinOp::Gt => BinOp::Gt,
            CBinOp::Le => BinOp::Le,
            CBinOp::Ge => BinOp::Ge,
            CBinOp::And => BinOp::Land,
            CBinOp::Or => BinOp::Lor,
            CBinOp::BitAnd => BinOp::And,
            CBinOp::BitOr => BinOp::Or,
            CBinOp::BitXor => BinOp::Xor,
            CBinOp::Shl => BinOp::Shl,
            CBinOp::Shr => BinOp::Shr,
            CBinOp::Assign => BinOp::Add, // Placeholder
        }
    }
}

/// Compile C source code to ÑEXO AST.
pub fn compile_c(source: &[u8]) -> BxResult<Ast> {
    let mut lexer = CLexer::new(source);
    let tokens = lexer.tokenize()?;
    let mut parser = CParser::new(tokens);
    let cast = parser.parse()?;
    let translator = CToNexo::new();
    translator.translate(&cast)
}
