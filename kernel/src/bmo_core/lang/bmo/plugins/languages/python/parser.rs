//! Python Parser — recursive-descent for the essential subset.

#![allow(dead_code)]

extern crate alloc;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::bmo_gpu::BxResult;
use super::ast::*;
use super::lexer::PyToken;

pub struct PyParser {
    tokens: Vec<PyToken>,
    pos: usize,
}

impl PyParser {
    pub fn new(tokens: Vec<PyToken>) -> Self {
        Self { tokens, pos: 0 }
    }

    pub fn parse(&mut self) -> BxResult<PyAst> {
        let mut items = Vec::new();
        while !self.is_eof() && !matches!(self.peek(), PyToken::Eof) {
            self.skip_newlines();
            if self.is_eof() { break; }
            if let Some(stmt) = self.parse_stmt()? {
                items.push(stmt);
            }
        }
        Ok(PyAst { items })
    }

    fn parse_stmt(&mut self) -> BxResult<Option<PyStmt>> {
        match self.peek() {
            PyToken::Newline => { self.advance(); Ok(None) }
            PyToken::Def => { let s = self.parse_funcdef()?; Ok(Some(s)) }
            PyToken::Class => { let s = self.parse_classdef()?; Ok(Some(s)) }
            PyToken::If => { let s = self.parse_if()?; Ok(Some(s)) }
            PyToken::While => { let s = self.parse_while()?; Ok(Some(s)) }
            PyToken::For => { let s = self.parse_for()?; Ok(Some(s)) }
            PyToken::Return => { let s = self.parse_return()?; Ok(Some(s)) }
            PyToken::Break => { self.advance(); self.expect_newline()?; Ok(Some(PyStmt::Break)) }
            PyToken::Continue => { self.advance(); self.expect_newline()?; Ok(Some(PyStmt::Continue)) }
            PyToken::Pass => { self.advance(); self.expect_newline()?; Ok(Some(PyStmt::Pass)) }
            PyToken::Import | PyToken::From => { let s = self.parse_import()?; Ok(Some(s)) }
            PyToken::Try => { let s = self.parse_try()?; Ok(Some(s)) }
            PyToken::With => { let s = self.parse_with()?; Ok(Some(s)) }
            PyToken::Indent => { self.advance(); let b = self.parse_block()?; Ok(Some(PyStmt::Block(b))) }
            _ => {
                // Expression or assignment
                let expr = self.parse_expr()?;
                if matches!(self.peek(), PyToken::Assign) {
                    self.advance();
                    let value = self.parse_expr()?;
                    self.expect_newline()?;
                    Ok(Some(PyStmt::Assign(vec![expr], value)))
                } else {
                    self.expect_newline()?;
                    Ok(Some(PyStmt::Expr(expr)))
                }
            }
        }
    }

    fn parse_funcdef(&mut self) -> BxResult<PyStmt> {
        self.expect(PyToken::Def)?;
        let name = self.expect_name()?;
        self.expect(PyToken::LParen)?;
        let mut params = Vec::new();
        if !matches!(self.peek(), PyToken::RParen) {
            loop {
                params.push(self.expect_name()?);
                if matches!(self.peek(), PyToken::Comma) {
                    self.advance();
                } else { break; }
            }
        }
        self.expect(PyToken::RParen)?;
        self.expect(PyToken::Colon)?;
        let body = self.parse_block()?;
        Ok(PyStmt::FuncDef { name, params, body })
    }

    fn parse_classdef(&mut self) -> BxResult<PyStmt> {
        self.expect(PyToken::Class)?;
        let name = self.expect_name()?;
        let parent = if matches!(self.peek(), PyToken::LParen) {
            self.advance();
            let p = self.expect_name()?;
            self.expect(PyToken::RParen)?;
            Some(p)
        } else { None };
        self.expect(PyToken::Colon)?;
        let body = self.parse_block()?;
        Ok(PyStmt::ClassDef { name, parent, body })
    }

    fn parse_if(&mut self) -> BxResult<PyStmt> {
        self.expect(PyToken::If)?;
        let cond = self.parse_expr()?;
        self.expect(PyToken::Colon)?;
        let then_body = self.parse_block()?;
        let mut elif_branches = Vec::new();
        while matches!(self.peek(), PyToken::Elif) {
            self.advance();
            let c = self.parse_expr()?;
            self.expect(PyToken::Colon)?;
            let b = self.parse_block()?;
            elif_branches.push((c, b));
        }
        let else_body = if matches!(self.peek(), PyToken::Else) {
            self.advance();
            self.expect(PyToken::Colon)?;
            Some(self.parse_block()?)
        } else { None };
        Ok(PyStmt::If { cond, then_body, elif_branches, else_body })
    }

    fn parse_while(&mut self) -> BxResult<PyStmt> {
        self.expect(PyToken::While)?;
        let cond = self.parse_expr()?;
        self.expect(PyToken::Colon)?;
        let body = self.parse_block()?;
        Ok(PyStmt::While { cond, body })
    }

    fn parse_for(&mut self) -> BxResult<PyStmt> {
        self.expect(PyToken::For)?;
        let var = self.expect_name()?;
        self.expect(PyToken::In)?;
        let iter = self.parse_expr()?;
        self.expect(PyToken::Colon)?;
        let body = self.parse_block()?;
        Ok(PyStmt::For { var, iter, body })
    }

    fn parse_return(&mut self) -> BxResult<PyStmt> {
        self.expect(PyToken::Return)?;
        let val = if matches!(self.peek(), PyToken::Newline | PyToken::Eof) { None } else { Some(self.parse_expr()?) };
        self.expect_newline()?;
        Ok(PyStmt::Return(val))
    }

    fn parse_import(&mut self) -> BxResult<PyStmt> {
        // v0.1.0: simplified — supports `import foo`
        if matches!(self.peek(), PyToken::Import) {
            self.advance();
            let module = self.expect_name()?;
            self.expect_newline()?;
            Ok(PyStmt::Import(PyImport::Module(module)))
        } else if matches!(self.peek(), PyToken::From) {
            self.advance();
            let module = self.expect_name()?;
            self.expect(PyToken::Import)?;
            let mut names = Vec::new();
            loop { names.push(self.expect_name()?); if !matches!(self.peek(), PyToken::Comma) { break; } self.advance(); }
            self.expect_newline()?;
            Ok(PyStmt::Import(PyImport::From(module, names)))
        } else {
            Err(crate::bmo_gpu::BxError::InvalidArgument)
        }
    }

    fn parse_try(&mut self) -> BxResult<PyStmt> {
        self.expect(PyToken::Try)?;
        self.expect(PyToken::Colon)?;
        let body = self.parse_block()?;
        let mut except_name = None;
        let mut except_body = Vec::new();
        if matches!(self.peek(), PyToken::Except) {
            self.advance();
            except_name = Some(self.expect_name()?);
            self.expect(PyToken::Colon)?;
            except_body = self.parse_block()?;
        }
        let finally_body = if matches!(self.peek(), PyToken::Finally) {
            self.advance();
            self.expect(PyToken::Colon)?;
            Some(self.parse_block()?)
        } else { None };
        self.expect_newline()?;
        Ok(PyStmt::Try { body, except_name, except_body, finally_body })
    }

    fn parse_with(&mut self) -> BxResult<PyStmt> {
        self.expect(PyToken::With)?;
        let ctx = self.parse_expr()?;
        self.expect(PyToken::Colon)?;
        let body = self.parse_block()?;
        self.expect_newline()?;
        Ok(PyStmt::With { ctx, body })
    }

    fn parse_block(&mut self) -> BxResult<Vec<PyStmt>> {
        self.expect(PyToken::Colon)?;
        self.expect(PyToken::Newline)?;
        self.expect(PyToken::Indent)?;
        let mut body = Vec::new();
        while !matches!(self.peek(), PyToken::Dedent | PyToken::Eof) {
            self.skip_newlines();
            if matches!(self.peek(), PyToken::Dedent | PyToken::Eof) { break; }
            if let Some(s) = self.parse_stmt()? { body.push(s); }
        }
        if matches!(self.peek(), PyToken::Dedent) { self.advance(); }
        Ok(body)
    }

    fn parse_expr(&mut self) -> BxResult<PyExpr> {
        // Clone the token so we can advance after.
        let tok = self.peek().clone();
        match tok {
            PyToken::IntLit(n) => { self.advance(); Ok(PyExpr::Literal(PyLiteral::Int(n))) }
            PyToken::FloatLit(b) => { self.advance(); Ok(PyExpr::Literal(PyLiteral::Float(b))) }
            PyToken::StrLit(s) => { self.advance(); Ok(PyExpr::Literal(PyLiteral::Str(s))) }
            PyToken::True => { self.advance(); Ok(PyExpr::Literal(PyLiteral::Bool(true))) }
            PyToken::False => { self.advance(); Ok(PyExpr::Literal(PyLiteral::Bool(false))) }
            PyToken::None => { self.advance(); Ok(PyExpr::Literal(PyLiteral::None)) }
            PyToken::Name(n) => { self.advance(); Ok(PyExpr::Name(n)) }
            _ => Err(crate::bmo_gpu::BxError::InvalidArgument),
        }
    }

    fn peek(&self) -> &PyToken { &self.tokens[self.pos] }
    fn advance(&mut self) -> PyToken {
        let t = self.tokens[self.pos].clone();
        if self.pos < self.tokens.len() - 1 { self.pos += 1; }
        t
    }
    fn is_eof(&self) -> bool { matches!(self.peek(), PyToken::Eof) }
    fn skip_newlines(&mut self) {
        while matches!(self.peek(), PyToken::Newline) { self.advance(); }
    }
    fn expect(&mut self, _want: PyToken) -> BxResult<()> {
        if matches!(self.peek(), _want) { self.advance(); Ok(()) }
        else { Err(crate::bmo_gpu::BxError::InvalidArgument) }
    }
    fn expect_name(&mut self) -> BxResult<String> {
        if let PyToken::Name(n) = self.peek() { let n = n.clone(); self.advance(); Ok(n) }
        else { Err(crate::bmo_gpu::BxError::InvalidArgument) }
    }
    fn expect_newline(&mut self) -> BxResult<()> {
        match self.peek() {
            PyToken::Newline | PyToken::Semi => { self.advance(); Ok(()) }
            _ => Ok(()),
        }
    }
}


