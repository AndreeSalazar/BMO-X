//! ÑEXO Parser — Parser recursivo descendente completo.
//!
//! Soporta: fn, let, if/else, while, for, return, break, continue,
//! struct, enum, impl, match, block, syscall, emit, expressions.

#![allow(dead_code)]

extern crate alloc;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::bmo_gpu::{BxError, BxResult};
use super::lexer::Token;

// ── AST ──────────────────────────────────────────────────────────────

/// Qualified path: `nexo::io::print` → Path(vec!["nexo", "io", "print"])
pub type Path = Vec<String>;

#[derive(Debug, Clone)]
pub enum Expr {
    LitInt(u64),
    LitFloat(u64),   // bits
    LitStr(String),
    LitByte(u8),
    LitBool(bool),
    LitNull,
    Ident(String),
    /// Qualified path expression: `modulo::func()`, `tipo::campo`
    QualifiedPath(Path),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    Unary(UnaryOp, Box<Expr>),
    Call(String, Vec<Expr>),        // func_name, args
    /// Call via qualified path: `io::print("hola")`
    QualifiedCall(Path, Vec<Expr>),
    MethodCall(Box<Expr>, String, Vec<Expr>),
    Field(Box<Expr>, String),
    Index(Box<Expr>, Box<Expr>),
    Syscall(u64, Vec<Expr>),
    Emit(Vec<u8>),
    Aloc(Box<Expr>),
    Libre(Box<Expr>),
    Reg(String),
    Block(Vec<Stmt>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add, Sub, Mul, Div, Mod,
    And, Or, Xor, Shl, Shr,
    Eq, Ne, Lt, Gt, Le, Ge,
    Land, Lor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg, Not, Deref, Ref,
}

/// Visibility of a declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Public,
    Private,
}

/// External declaration kind (for FFI / C interop).
#[derive(Debug, Clone)]
pub enum ExternItem {
    Fn { name: String, params: Vec<Param>, ret: Option<TypeAnnotation> },
    Static { name: String, ty: TypeAnnotation },
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Let { name: String, ty: Option<TypeAnnotation>, value: Option<Expr> },
    Mut { name: String, ty: Option<TypeAnnotation>, value: Expr },
    Assign(String, Expr),
    Return(Option<Expr>),
    Break,
    Continue,
    If { cond: Expr, then_body: Vec<Stmt>, else_body: Option<Vec<Stmt>> },
    While { cond: Expr, body: Vec<Stmt> },
    For { var: String, start: Expr, end: Expr, body: Vec<Stmt> },
    Block(Vec<Stmt>),
    ExprStmt(Expr),
    FnDecl { name: String, params: Vec<Param>, ret: Option<TypeAnnotation>, body: Vec<Stmt> },
    StructDecl { name: String, fields: Vec<(String, TypeAnnotation)> },
    EnumDecl { name: String, variants: Vec<String> },
    ImplDecl { type_name: String, methods: Vec<Stmt> },
    Syscall { nr: u64, args: Vec<Expr> },
    Emit(Vec<u8>),
    Aloc { size: Expr },
    Libre(Expr),
    Module { name: String, items: Vec<Stmt> },
    // ── Module system ─────────────────────────────────────────
    /// `usa nexo::io;` or `usa nexo::io::print;`
    Use { path: Path, alias: Option<String> },
    /// `usa nexo::io::*;` — import all public names from module
    UseGlob { path: Path },
    /// `pub fn ...`, `pub tipo ...`, `pub modulo ...`
    Pub { inner: Box<Stmt> },
    /// `externa { fn printf(ptr byte, ...) -> num; }`
    Extern { items: Vec<ExternItem> },
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub ty: TypeAnnotation,
}

#[derive(Debug, Clone)]
pub enum TypeAnnotation {
    Named(String),
    Ptr(Box<TypeAnnotation>),
    Ref(Box<TypeAnnotation>),
    Array(Box<TypeAnnotation>, u64),
    Optional(Box<TypeAnnotation>),
    /// Qualified type: `nexo::io::Error`
    QualifiedType(Path),
}

#[derive(Debug, Clone, Default)]
pub struct Ast {
    pub items: Vec<Stmt>,
}

// ── Parser ───────────────────────────────────────────────────────────

pub struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl<'a> Parser<'a> {
    pub fn new(tokens: &'a [Token]) -> Self {
        Self { tokens, pos: 0 }
    }

    pub fn parse(&mut self) -> BxResult<Ast> {
        let mut ast = Ast::default();
        while !self.is_eof() {
            ast.items.push(self.parse_stmt()?);
        }
        Ok(ast)
    }

    fn parse_stmt(&mut self) -> BxResult<Stmt> {
        match self.peek().clone() {
            Token::Fn => self.parse_fn_decl(),
            Token::Let => self.parse_let(),
            Token::Mut => self.parse_mut(),
            Token::Return => { self.advance(); let v = if !self.check(Token::Semi) { Some(self.parse_expr()?) } else { None }; self.expect(Token::Semi)?; Ok(Stmt::Return(v)) }
            Token::Break => { self.advance(); self.expect(Token::Semi)?; Ok(Stmt::Break) }
            Token::Continue => { self.advance(); self.expect(Token::Semi)?; Ok(Stmt::Continue) }
            Token::If => self.parse_if(),
            Token::While => self.parse_while(),
            Token::For => self.parse_for(),
            Token::LBrace => self.parse_block(),
            Token::Struct => self.parse_struct_decl(),
            Token::Enum => self.parse_enum_decl(),
            Token::Impl => self.parse_impl_decl(),
            Token::Syscall => self.parse_syscall_stmt(),
            Token::Emit => self.parse_emit_stmt(),
            Token::Aloc => self.parse_aloc_stmt(),
            Token::Libre => self.parse_libre_stmt(),
            Token::Use => self.parse_use(),
            Token::Pub => self.parse_pub(),
            Token::Module => self.parse_module(),
            _ => {
                let expr = self.parse_expr()?;
                if self.check(Token::Eq) {
                    self.advance();
                    let val = self.parse_expr()?;
                    self.expect(Token::Semi)?;
                    // Extract name from expression
                    if let Expr::Ident(name) = expr {
                        Ok(Stmt::Assign(name, val))
                    } else {
                        Err(BxError::InvalidArgument)
                    }
                } else {
                    self.expect(Token::Semi)?;
                    Ok(Stmt::ExprStmt(expr))
                }
            }
        }
    }

    fn parse_fn_decl(&mut self) -> BxResult<Stmt> {
        self.expect(Token::Fn)?;
        let name = self.expect_ident()?;
        self.expect(Token::LParen)?;
        let params = self.parse_params()?;
        self.expect(Token::RParen)?;
        let ret = if self.check(Token::Arrow) { self.advance(); Some(self.parse_type()?) } else { None };
        let body = self.parse_block_body()?;
        Ok(Stmt::FnDecl { name, params, ret, body })
    }

    fn parse_let(&mut self) -> BxResult<Stmt> {
        self.expect(Token::Let)?;
        let name = self.expect_ident()?;
        let ty = if self.check(Token::Colon) { self.advance(); Some(self.parse_type()?) } else { None };
        let value = if self.check(Token::Eq) { self.advance(); Some(self.parse_expr()?) } else { None };
        self.expect(Token::Semi)?;
        Ok(Stmt::Let { name, ty, value })
    }

    fn parse_mut(&mut self) -> BxResult<Stmt> {
        self.expect(Token::Mut)?;
        let name = self.expect_ident()?;
        let ty = if self.check(Token::Colon) { self.advance(); Some(self.parse_type()?) } else { None };
        self.expect(Token::Eq)?;
        let value = self.parse_expr()?;
        self.expect(Token::Semi)?;
        Ok(Stmt::Mut { name, ty, value })
    }

    fn parse_if(&mut self) -> BxResult<Stmt> {
        self.expect(Token::If)?;
        let cond = self.parse_expr()?;
        let then_body = self.parse_block_body()?;
        let else_body = if self.check(Token::Else) {
            self.advance();
            if self.check(Token::If) {
                Some(vec![self.parse_if()?])
            } else {
                Some(self.parse_block_body()?)
            }
        } else { None };
        Ok(Stmt::If { cond, then_body, else_body })
    }

    fn parse_while(&mut self) -> BxResult<Stmt> {
        self.expect(Token::While)?;
        let cond = self.parse_expr()?;
        let body = self.parse_block_body()?;
        Ok(Stmt::While { cond, body })
    }

    fn parse_for(&mut self) -> BxResult<Stmt> {
        self.expect(Token::For)?;
        let var = self.expect_ident()?;
        self.expect(Token::Eq)?;
        let start = self.parse_expr()?;
        // Expect `..` or `hasta`
        if self.check(Token::Dot) { self.advance(); self.expect(Token::Dot)?; }
        let end = self.parse_expr()?;
        let body = self.parse_block_body()?;
        Ok(Stmt::For { var, start, end, body })
    }

    fn parse_struct_decl(&mut self) -> BxResult<Stmt> {
        self.expect(Token::Struct)?;
        let name = self.expect_ident()?;
        self.expect(Token::LBrace)?;
        let mut fields = Vec::new();
        while !self.check(Token::RBrace) {
            let fname = self.expect_ident()?;
            self.expect(Token::Colon)?;
            let fty = self.parse_type()?;
            fields.push((fname, fty));
            if self.check(Token::Comma) { self.advance(); }
        }
        self.expect(Token::RBrace)?;
        Ok(Stmt::StructDecl { name, fields })
    }

    fn parse_enum_decl(&mut self) -> BxResult<Stmt> {
        self.expect(Token::Enum)?;
        let name = self.expect_ident()?;
        self.expect(Token::LBrace)?;
        let mut variants = Vec::new();
        while !self.check(Token::RBrace) {
            variants.push(self.expect_ident()?);
            if self.check(Token::Comma) { self.advance(); }
        }
        self.expect(Token::RBrace)?;
        Ok(Stmt::EnumDecl { name, variants })
    }

    fn parse_impl_decl(&mut self) -> BxResult<Stmt> {
        self.expect(Token::Impl)?;
        let type_name = self.expect_ident()?;
        self.expect(Token::LBrace)?;
        let mut methods = Vec::new();
        while !self.check(Token::RBrace) {
            methods.push(self.parse_stmt()?);
        }
        self.expect(Token::RBrace)?;
        Ok(Stmt::ImplDecl { type_name, methods })
    }

    fn parse_syscall_stmt(&mut self) -> BxResult<Stmt> {
        self.expect(Token::Syscall)?;
        self.expect(Token::LParen)?;
        let nr = match self.peek().clone() {
            Token::IntLit(n) => { self.advance(); n }
            _ => return Err(BxError::InvalidArgument),
        };
        let mut args = Vec::new();
        if self.check(Token::Comma) {
            self.advance();
            while !self.check(Token::RParen) {
                args.push(self.parse_expr()?);
                if self.check(Token::Comma) { self.advance(); }
            }
        }
        self.expect(Token::RParen)?;
        self.expect(Token::Semi)?;
        Ok(Stmt::Syscall { nr, args })
    }

    fn parse_emit_stmt(&mut self) -> BxResult<Stmt> {
        self.expect(Token::Emit)?;
        let mut bytes = Vec::new();
        while !self.check(Token::Semi) && !self.is_eof() {
            if let Token::IntLit(b) = self.peek().clone() {
                bytes.push(b as u8);
                self.advance();
            } else {
                break;
            }
        }
        self.expect(Token::Semi)?;
        Ok(Stmt::Emit(bytes))
    }

    fn parse_aloc_stmt(&mut self) -> BxResult<Stmt> {
        self.expect(Token::Aloc)?;
        let size = self.parse_expr()?;
        self.expect(Token::Semi)?;
        Ok(Stmt::Aloc { size })
    }

    fn parse_libre_stmt(&mut self) -> BxResult<Stmt> {
        self.expect(Token::Libre)?;
        let ptr = self.parse_expr()?;
        self.expect(Token::Semi)?;
        Ok(Stmt::Libre(ptr))
    }

    fn parse_module(&mut self) -> BxResult<Stmt> {
        self.expect(Token::Module)?;
        let name = self.expect_ident()?;
        let items = self.parse_block_body()?;
        Ok(Stmt::Module { name, items })
    }

    /// Parse `usa path::to::module [como alias];` or `usa path::*;`
    fn parse_use(&mut self) -> BxResult<Stmt> {
        self.expect(Token::Use)?;
        let path = self.parse_path()?;
        if self.check(Token::Star) {
            self.advance();
            self.expect(Token::Semi)?;
            return Ok(Stmt::UseGlob { path });
        }
        let alias = if self.check(Token::As) {
            self.advance();
            Some(self.expect_ident()?)
        } else {
            None
        };
        self.expect(Token::Semi)?;
        Ok(Stmt::Use { path, alias })
    }

    /// Parse `pub <statement>` — wraps next statement with public visibility.
    fn parse_pub(&mut self) -> BxResult<Stmt> {
        self.expect(Token::Pub)?;
        let inner = self.parse_stmt()?;
        Ok(Stmt::Pub { inner: Box::new(inner) })
    }

    /// Parse `externa { fn name(params) -> ret; static name: tipo; }`
    fn parse_extern(&mut self) -> BxResult<Stmt> {
        // Extern keyword maps to Import token (importa)
        self.expect(Token::Import)?;
        self.expect(Token::LBrace)?;
        let mut items = Vec::new();
        while !self.check(Token::RBrace) && !self.is_eof() {
            match self.peek().clone() {
                Token::Fn => {
                    self.advance();
                    let name = self.expect_ident()?;
                    self.expect(Token::LParen)?;
                    let params = self.parse_params()?;
                    self.expect(Token::RParen)?;
                    let ret = if self.check(Token::Arrow) {
                        self.advance();
                        Some(self.parse_type()?)
                    } else { None };
                    self.expect(Token::Semi)?;
                    items.push(ExternItem::Fn { name, params, ret });
                }
                Token::Let => {
                    self.advance();
                    let name = self.expect_ident()?;
                    self.expect(Token::Colon)?;
                    let ty = self.parse_type()?;
                    self.expect(Token::Semi)?;
                    items.push(ExternItem::Static { name, ty });
                }
                _ => { self.advance(); } // skip unexpected tokens
            }
        }
        self.expect(Token::RBrace)?;
        Ok(Stmt::Extern { items })
    }

    /// Parse a qualified path: `ident::ident::ident`
    fn parse_path(&mut self) -> BxResult<Path> {
        let mut path = Vec::new();
        path.push(self.expect_ident()?);
        while self.check(Token::ColonColon) {
            self.advance();
            path.push(self.expect_ident()?);
        }
        Ok(path)
    }

    fn parse_block(&mut self) -> BxResult<Stmt> {
        let items = self.parse_block_body()?;
        Ok(Stmt::Block(items))
    }

    fn parse_block_body(&mut self) -> BxResult<Vec<Stmt>> {
        self.expect(Token::LBrace)?;
        let mut stmts = Vec::new();
        while !self.check(Token::RBrace) && !self.is_eof() {
            stmts.push(self.parse_stmt()?);
        }
        self.expect(Token::RBrace)?;
        Ok(stmts)
    }

    fn parse_params(&mut self) -> BxResult<Vec<Param>> {
        let mut params = Vec::new();
        if self.check(Token::RParen) { return Ok(params); }
        loop {
            let name = self.expect_ident()?;
            self.expect(Token::Colon)?;
            let ty = self.parse_type()?;
            params.push(Param { name, ty });
            if self.check(Token::Comma) { self.advance(); } else { break; }
        }
        Ok(params)
    }

    // ── Expressions ──────────────────────────────────────────────

    fn parse_expr(&mut self) -> BxResult<Expr> {
        self.parse_expr_prec(0)
    }

    fn parse_expr_prec(&mut self, min_prec: u32) -> BxResult<Expr> {
        let mut left = self.parse_primary()?;
        while let Some(op) = self.peek_binop() {
            let prec = op.precedence();
            if prec < min_prec { break; }
            self.advance();
            let right = self.parse_expr_prec(prec + 1)?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_primary(&mut self) -> BxResult<Expr> {
        match self.peek().clone() {
            Token::IntLit(n) => { self.advance(); Ok(Expr::LitInt(n)) }
            Token::FloatLit(b) => { self.advance(); Ok(Expr::LitFloat(b)) }
            Token::StrLit(s) => { self.advance(); Ok(Expr::LitStr(s)) }
            Token::ByteLit(b) => { self.advance(); Ok(Expr::LitByte(b)) }
            Token::True => { self.advance(); Ok(Expr::LitBool(true)) }
            Token::False => { self.advance(); Ok(Expr::LitBool(false)) }
            Token::Null => { self.advance(); Ok(Expr::LitNull) }
            Token::Ident(name) => {
                self.advance();
                // Check for qualified path: `name::name::...`
                if self.check(Token::ColonColon) {
                    let mut path = vec![name];
                    while self.check(Token::ColonColon) {
                        self.advance();
                        path.push(self.expect_ident()?);
                    }
                    // Check for qualified call: `path::to::func(args)`
                    if self.check(Token::LParen) {
                        self.advance();
                        let mut args = Vec::new();
                        if !self.check(Token::RParen) {
                            loop {
                                args.push(self.parse_expr()?);
                                if self.check(Token::Comma) { self.advance(); } else { break; }
                            }
                        }
                        self.expect(Token::RParen)?;
                        return Ok(Expr::QualifiedCall(path, args));
                    }
                    return Ok(Expr::QualifiedPath(path));
                }
                // Check for function call
                if self.check(Token::LParen) {
                    self.advance();
                    let mut args = Vec::new();
                    if !self.check(Token::RParen) {
                        loop {
                            args.push(self.parse_expr()?);
                            if self.check(Token::Comma) { self.advance(); } else { break; }
                        }
                    }
                    self.expect(Token::RParen)?;
                    Ok(Expr::Call(name, args))
                } else {
                    Ok(Expr::Ident(name))
                }
            }
            Token::LParen => {
                self.advance();
                let expr = self.parse_expr()?;
                self.expect(Token::RParen)?;
                Ok(expr)
            }
            Token::Bang => { self.advance(); let e = self.parse_primary()?; Ok(Expr::Unary(UnaryOp::Not, Box::new(e))) }
            Token::Minus => { self.advance(); let e = self.parse_primary()?; Ok(Expr::Unary(UnaryOp::Neg, Box::new(e))) }
            Token::Star => { self.advance(); let e = self.parse_primary()?; Ok(Expr::Unary(UnaryOp::Deref, Box::new(e))) }
            Token::Amp => { self.advance(); let e = self.parse_primary()?; Ok(Expr::Unary(UnaryOp::Ref, Box::new(e))) }
            Token::Syscall => {
                self.advance();
                self.expect(Token::LParen)?;
                let nr = match self.peek().clone() {
                    Token::IntLit(n) => { self.advance(); n }
                    _ => return Err(BxError::InvalidArgument),
                };
                let mut args = Vec::new();
                if self.check(Token::Comma) {
                    self.advance();
                    while !self.check(Token::RParen) {
                        args.push(self.parse_expr()?);
                        if self.check(Token::Comma) { self.advance(); }
                    }
                }
                self.expect(Token::RParen)?;
                Ok(Expr::Syscall(nr, args))
            }
            Token::Emit => {
                self.advance();
                let mut bytes = Vec::new();
                while !self.check(Token::Semi) && !self.is_eof() {
                    if let Token::IntLit(b) = self.peek().clone() {
                        bytes.push(b as u8);
                        self.advance();
                    } else { break; }
                }
                Ok(Expr::Emit(bytes))
            }
            Token::Aloc => {
                self.advance();
                let size = self.parse_expr()?;
                Ok(Expr::Aloc(Box::new(size)))
            }
            Token::Libre => {
                self.advance();
                let ptr = self.parse_expr()?;
                Ok(Expr::Libre(Box::new(ptr)))
            }
            Token::Reg => {
                self.advance();
                let name = self.expect_ident()?;
                Ok(Expr::Reg(name))
            }
            Token::LBrace => {
                self.advance();
                let mut stmts = Vec::new();
                while !self.check(Token::RBrace) && !self.is_eof() {
                    stmts.push(self.parse_stmt()?);
                }
                self.expect(Token::RBrace)?;
                Ok(Expr::Block(stmts))
            }
            _ => Err(BxError::InvalidArgument),
        }
    }

    fn parse_type(&mut self) -> BxResult<TypeAnnotation> {
        let name = self.expect_ident()?;
        if self.check(Token::Star) {
            self.advance();
            Ok(TypeAnnotation::Ptr(Box::new(TypeAnnotation::Named(name))))
        } else if self.check(Token::Amp) {
            self.advance();
            Ok(TypeAnnotation::Ref(Box::new(TypeAnnotation::Named(name))))
        } else if self.check(Token::LBracket) {
            self.advance();
            let size = match self.peek().clone() {
                Token::IntLit(n) => { self.advance(); n }
                _ => return Err(BxError::InvalidArgument),
            };
            self.expect(Token::RBracket)?;
            Ok(TypeAnnotation::Array(Box::new(TypeAnnotation::Named(name)), size))
        } else {
            Ok(TypeAnnotation::Named(name))
        }
    }

    fn peek_binop(&self) -> Option<BinOp> {
        match self.peek() {
            Token::Plus => Some(BinOp::Add),
            Token::Minus => Some(BinOp::Sub),
            Token::Star => Some(BinOp::Mul),
            Token::Slash => Some(BinOp::Div),
            Token::Percent => Some(BinOp::Mod),
            Token::Amp if !self.check_next(Token::Amp) => Some(BinOp::And),
            Token::Pipe if !self.check_next(Token::Pipe) => Some(BinOp::Or),
            Token::Caret => Some(BinOp::Xor),
            Token::Shl => Some(BinOp::Shl),
            Token::Shr => Some(BinOp::Shr),
            Token::EqEq => Some(BinOp::Eq),
            Token::Ne => Some(BinOp::Ne),
            Token::Lt => Some(BinOp::Lt),
            Token::Gt => Some(BinOp::Gt),
            Token::Le => Some(BinOp::Le),
            Token::Ge => Some(BinOp::Ge),
            Token::AmpAmp => Some(BinOp::Land),
            Token::PipePipe => Some(BinOp::Lor),
            _ => None,
        }
    }

    fn peek(&self) -> &Token {
        if self.pos < self.tokens.len() { &self.tokens[self.pos] } else { &Token::Eof }
    }

    fn check(&self, expected: Token) -> bool {
        core::mem::discriminant(self.peek()) == core::mem::discriminant(&expected)
    }

    fn check_next(&self, expected: Token) -> bool {
        if self.pos + 1 < self.tokens.len() {
            core::mem::discriminant(&self.tokens[self.pos + 1]) == core::mem::discriminant(&expected)
        } else { false }
    }

    fn advance(&mut self) {
        if self.pos < self.tokens.len() { self.pos += 1; }
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.tokens.len() || matches!(self.peek(), Token::Eof)
    }

    fn expect(&mut self, expected: Token) -> BxResult<()> {
        if core::mem::discriminant(self.peek()) == core::mem::discriminant(&expected) {
            self.advance();
            Ok(())
        } else {
            Err(BxError::InvalidArgument)
        }
    }

    fn expect_ident(&mut self) -> BxResult<String> {
        match self.peek().clone() {
            Token::Ident(name) => { self.advance(); Ok(name) }
            _ => Err(BxError::InvalidArgument),
        }
    }
}

impl BinOp {
    pub fn precedence(&self) -> u32 {
        match self {
            BinOp::Lor => 1,
            BinOp::Land => 2,
            BinOp::Or => 3,
            BinOp::Xor => 4,
            BinOp::And => 5,
            BinOp::Eq | BinOp::Ne => 6,
            BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => 7,
            BinOp::Shl | BinOp::Shr => 8,
            BinOp::Add | BinOp::Sub => 9,
            BinOp::Mul | BinOp::Div | BinOp::Mod => 10,
        }
    }
}
