//! C Parser — recursive descent parser for C source code.

#![allow(dead_code)]

extern crate alloc;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use crate::barex::{BxError, BxResult};
use super::lexer::CToken;
use super::ast::{CType, CExpr, CBinOp, CUnaryOp, CStmt, CParam, CItem, CAst};

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
        let is_static = if self.check(&CToken::Static) { self.advance(); true } else { false };
        let is_extern = if self.check(&CToken::Extern) { self.advance(); true } else { false };

        match self.peek() {
            CToken::Struct => self.parse_struct(),
            CToken::Typedef => self.parse_typedef(),
            _ => {
                let ty = self.parse_type()?;
                let name = self.expect_ident()?;

                if self.check(&CToken::LParen) {
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
