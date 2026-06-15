//! ÑEXO Parser — Tokenstream → AST.
//!
//! Parser recursivo descendente para ÑEXO.

#![allow(dead_code)]

extern crate alloc;
use alloc::boxed::Box;
use alloc::vec::Vec;
use alloc::string::String;

use crate::barex::{BxError, BxResult};
use super::lexer::Token;

/// AST nodes for ÑEXO.
#[derive(Debug, Clone)]
pub enum Expr {
    LitInt(u64),
    LitFloat(f64),
    LitStr(String),
    LitBool(bool),
    LitNull,
    Ident(String),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    Unary(UnaryOp, Box<Expr>),
    Call(Box<Expr>, Vec<Expr>),
    Index(Box<Expr>, Box<Expr>),
    Field(Box<Expr>, String),
    Syscall(u64, Vec<Expr>),
}

#[derive(Debug, Clone, Copy)]
pub enum BinOp {
    Add, Sub, Mul, Div, Mod,
    And, Or, Xor,
    Eq, Ne, Lt, Gt, Le, Ge,
    Shl, Shr,
    Land, Lor,
}

#[derive(Debug, Clone, Copy)]
pub enum UnaryOp {
    Neg, Not, Deref, Ref,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Let { name: String, ty: Option<TypeAnnotation>, value: Option<Expr> },
    Assign(String, Expr),
    Return(Option<Expr>),
    If { cond: Expr, then_body: Vec<Stmt>, else_body: Option<Vec<Stmt>> },
    While { cond: Expr, body: Vec<Stmt> },
    For { var: String, iter: Expr, body: Vec<Stmt> },
    Block(Vec<Stmt>),
    ExprStmt(Expr),
    FnDecl { name: String, params: Vec<Param>, ret: Option<TypeAnnotation>, body: Vec<Stmt> },
    StructDecl { name: String, fields: Vec<(String, TypeAnnotation)> },
    Syscall { nr: u64, args: Vec<Expr> },
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
    Array(Box<TypeAnnotation>, usize),
}

/// Top-level AST.
#[derive(Debug, Clone, Default)]
pub struct Ast {
    pub items: Vec<Stmt>,
}

/// Parser state.
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
        match self.peek() {
            Token::Fn => self.parse_fn_decl(),
            Token::Let => self.parse_let(),
            Token::Return => {
                self.advance();
                let value = if *self.peek() != Token::Semi { Some(self.parse_expr()?) } else { None };
                self.expect(Token::Semi)?;
                Ok(Stmt::Return(value))
            }
            Token::If => self.parse_if(),
            Token::While => self.parse_while(),
            Token::LBrace => {
                self.advance();
                let mut stmts = Vec::new();
                while *self.peek() != Token::RBrace {
                    stmts.push(self.parse_stmt()?);
                }
                self.expect(Token::RBrace)?;
                Ok(Stmt::Block(stmts))
            }
            Token::Syscall => {
                self.advance();
                self.expect(Token::LParen)?;
                let nr = match self.peek() {
                    Token::IntLit(n) => { let v = *n; self.advance(); v }
                    _ => return Err(BxError::InvalidArgument),
                };
                self.expect(Token::RParen)?;
                self.expect(Token::Semi)?;
                Ok(Stmt::Syscall { nr, args: Vec::new() })
            }
            _ => {
                let expr = self.parse_expr()?;
                self.expect(Token::Semi)?;
                Ok(Stmt::ExprStmt(expr))
            }
        }
    }

    fn parse_fn_decl(&mut self) -> BxResult<Stmt> {
        self.expect(Token::Fn)?;
        let name = self.expect_ident()?;
        self.expect(Token::LParen)?;
        let mut params = Vec::new();
        if *self.peek() != Token::RParen {
            loop {
                let pname = self.expect_ident()?;
                self.expect(Token::Colon)?;
                let pty = self.parse_type()?;
                params.push(Param { name: pname, ty: pty });
                if *self.peek() == Token::Comma { self.advance(); } else { break; }
            }
        }
        self.expect(Token::RParen)?;
        let ret = if *self.peek() == Token::Arrow {
            self.advance();
            Some(self.parse_type()?)
        } else {
            None
        };
        self.expect(Token::LBrace)?;
        let mut body = Vec::new();
        while *self.peek() != Token::RBrace {
            body.push(self.parse_stmt()?);
        }
        self.expect(Token::RBrace)?;
        Ok(Stmt::FnDecl { name, params, ret, body })
    }

    fn parse_let(&mut self) -> BxResult<Stmt> {
        self.expect(Token::Let)?;
        let name = self.expect_ident()?;
        let ty = if *self.peek() == Token::Colon {
            self.advance();
            Some(self.parse_type()?)
        } else {
            None
        };
        let value = if *self.peek() == Token::Eq {
            self.advance();
            Some(self.parse_expr()?)
        } else {
            None
        };
        self.expect(Token::Semi)?;
        Ok(Stmt::Let { name, ty, value })
    }

    fn parse_if(&mut self) -> BxResult<Stmt> {
        self.expect(Token::If)?;
        let cond = self.parse_expr()?;
        self.expect(Token::LBrace)?;
        let mut then_body = Vec::new();
        while *self.peek() != Token::RBrace {
            then_body.push(self.parse_stmt()?);
        }
        self.expect(Token::RBrace)?;
        let else_body = if *self.peek() == Token::Else {
            self.advance();
            self.expect(Token::LBrace)?;
            let mut else_stmts = Vec::new();
            while *self.peek() != Token::RBrace {
                else_stmts.push(self.parse_stmt()?);
            }
            self.expect(Token::RBrace)?;
            Some(else_stmts)
        } else {
            None
        };
        Ok(Stmt::If { cond, then_body, else_body })
    }

    fn parse_while(&mut self) -> BxResult<Stmt> {
        self.expect(Token::While)?;
        let cond = self.parse_expr()?;
        self.expect(Token::LBrace)?;
        let mut body = Vec::new();
        while *self.peek() != Token::RBrace {
            body.push(self.parse_stmt()?);
        }
        self.expect(Token::RBrace)?;
        Ok(Stmt::While { cond, body })
    }

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
            Token::StrLit(s) => { self.advance(); Ok(Expr::LitStr(s)) }
            Token::True => { self.advance(); Ok(Expr::LitBool(true)) }
            Token::False => { self.advance(); Ok(Expr::LitBool(false)) }
            Token::Null => { self.advance(); Ok(Expr::LitNull) }
            Token::Ident(name) => {
                self.advance();
                Ok(Expr::Ident(name))
            }
            Token::LParen => {
                self.advance();
                let expr = self.parse_expr()?;
                self.expect(Token::RParen)?;
                Ok(expr)
            }
            Token::Bang => {
                self.advance();
                let expr = self.parse_primary()?;
                Ok(Expr::Unary(UnaryOp::Not, Box::new(expr)))
            }
            Token::Minus => {
                self.advance();
                let expr = self.parse_primary()?;
                Ok(Expr::Unary(UnaryOp::Neg, Box::new(expr)))
            }
            _ => Err(BxError::InvalidArgument),
        }
    }

    fn parse_type(&mut self) -> BxResult<TypeAnnotation> {
        let name = self.expect_ident()?;
        if *self.peek() == Token::Star {
            self.advance();
            Ok(TypeAnnotation::Ptr(Box::new(TypeAnnotation::Named(name))))
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
            Token::Amp if *self.peek_next() != Token::Amp => Some(BinOp::And),
            Token::Pipe if *self.peek_next() != Token::Pipe => Some(BinOp::Or),
            Token::Caret => Some(BinOp::Xor),
            Token::EqEq => Some(BinOp::Eq),
            Token::Ne => Some(BinOp::Ne),
            Token::Lt => Some(BinOp::Lt),
            Token::Gt => Some(BinOp::Gt),
            Token::Le => Some(BinOp::Le),
            Token::Ge => Some(BinOp::Ge),
            Token::Shl => Some(BinOp::Shl),
            Token::Shr => Some(BinOp::Shr),
            Token::AmpAmp => Some(BinOp::Land),
            Token::PipePipe => Some(BinOp::Lor),
            _ => None,
        }
    }

    fn peek(&self) -> &Token {
        if self.pos < self.tokens.len() { &self.tokens[self.pos] } else { &Token::Eof }
    }

    fn peek_next(&self) -> &Token {
        if self.pos + 1 < self.tokens.len() { &self.tokens[self.pos + 1] } else { &Token::Eof }
    }

    fn advance(&mut self) {
        if self.pos < self.tokens.len() { self.pos += 1; }
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.tokens.len() || *self.peek() == Token::Eof
    }

    fn expect(&mut self, expected: Token) -> BxResult<()> {
        if *self.peek() == expected {
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
    fn precedence(&self) -> u32 {
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
