//! Java Parser — recursive-descent for the essential subset.

#![allow(dead_code)]

extern crate alloc;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;

use crate::bmo_gpu::BxResult;
use super::ast::*;
use super::lexer::JToken;

pub struct JParser {
    tokens: Vec<JToken>,
    pos: usize,
}

impl JParser {
    pub fn new(tokens: Vec<JToken>) -> Self { Self { tokens, pos: 0 } }

    pub fn parse(&mut self) -> BxResult<JAst> {
        let mut items = Vec::new();
        while !matches!(self.peek(), JToken::Eof) {
            if let Some(item) = self.parse_item()? { items.push(item); }
        }
        Ok(JAst { items })
    }

    fn parse_item(&mut self) -> BxResult<Option<JItem>> {
        // skip modifiers
        let mods = self.parse_mods()?;
        if matches!(self.peek(), JToken::Class | JToken::Interface) {
            let cls = self.parse_class(mods)?;
            return Ok(Some(JItem::Class(cls)));
        }
        Ok(None)
    }

    fn parse_mods(&mut self) -> BxResult<Vec<JMod>> {
        let mut mods = Vec::new();
        loop {
            let m = match self.peek() {
                JToken::Public => Some(JMod::Public),
                JToken::Private => Some(JMod::Private),
                JToken::Protected => Some(JMod::Protected),
                JToken::Static => Some(JMod::Static),
                JToken::Final => Some(JMod::Final),
                JToken::Abstract => Some(JMod::Abstract),
                _ => None,
            };
            match m {
                Some(modifier) => { self.advance(); mods.push(modifier); }
                None => break,
            }
        }
        Ok(mods)
    }

    fn parse_class(&mut self, mods: Vec<JMod>) -> BxResult<JClass> {
        let is_interface = matches!(self.peek(), JToken::Interface);
        self.advance();
        let name = self.expect_ident()?;
        let mut parent = None;
        let mut implements = Vec::new();
        if matches!(self.peek(), JToken::Extends) {
            self.advance();
            parent = Some(self.expect_ident()?);
        }
        if matches!(self.peek(), JToken::Implements) {
            self.advance();
            loop {
                implements.push(self.expect_ident()?);
                if !matches!(self.peek(), JToken::Comma) { break; }
                self.advance();
            }
        }
        self.expect(JToken::LBrace)?;
        let mut members = Vec::new();
        while !matches!(self.peek(), JToken::RBrace | JToken::Eof) {
            if let Some(m) = self.parse_member()? { members.push(m); }
        }
        self.expect(JToken::RBrace)?;
        Ok(JClass { mods, name, parent, implements, is_interface, members })
    }

    fn parse_member(&mut self) -> BxResult<Option<JMember>> {
        let mods = self.parse_mods()?;
        let ty = self.parse_type()?;
        let name = self.expect_ident()?;
        if matches!(self.peek(), JToken::LParen) {
            // method or constructor
            self.advance();
            let mut params = Vec::new();
            if !matches!(self.peek(), JToken::RParen) {
                loop {
                    let pty = self.parse_type()?;
                    let pname = self.expect_ident()?;
                    params.push(JParam { ty: pty, name: pname });
                    if !matches!(self.peek(), JToken::Comma) { break; }
                    self.advance();
                }
            }
            self.expect(JToken::RParen)?;
            if name == "<init>" || /* heuristic: same name as class */ false {
                // Constructor
                self.expect(JToken::LBrace)?;
                let body = self.parse_block_body()?;
                self.expect(JToken::RBrace)?;
                return Ok(Some(JMember { mods, kind: JMemberKind::Constructor { params, body } }));
            } else {
                let is_abstract = mods.contains(&JMod::Abstract);
                if matches!(self.peek(), JToken::Semi) {
                    self.advance();
                    return Ok(Some(JMember { mods, kind: JMemberKind::Method { ret: ty, name, params, body: Vec::new(), is_abstract } }));
                }
                self.expect(JToken::LBrace)?;
                let body = self.parse_block_body()?;
                self.expect(JToken::RBrace)?;
                return Ok(Some(JMember { mods, kind: JMemberKind::Method { ret: ty, name, params, body, is_abstract } }));
            }
        } else {
            // field
            let init = if matches!(self.peek(), JToken::Eq) {
                self.advance();
                Some(self.parse_expr()?)
            } else { None };
            self.expect(JToken::Semi)?;
            return Ok(Some(JMember { mods, kind: JMemberKind::Field { ty, name, init } }));
        }
    }

    fn parse_type(&mut self) -> BxResult<JType> {
        // Primitive?
        let prim = match self.peek() {
            JToken::Void => Some(JPrim::Void),
            JToken::Boolean => Some(JPrim::Boolean),
            JToken::Byte => Some(JPrim::Byte),
            JToken::Short => Some(JPrim::Short),
            JToken::Int => Some(JPrim::Int),
            JToken::Long => Some(JPrim::Long),
            JToken::Float => Some(JPrim::Float),
            JToken::Double => Some(JPrim::Double),
            JToken::Char => Some(JPrim::Char),
            _ => None,
        };
        if let Some(p) = prim {
            self.advance();
            return Ok(JType::Prim(p));
        }
        // Class type
        let name = self.expect_ident()?;
        let mut ty = JType::Class(name);
        while matches!(self.peek(), JToken::LBracket) {
            self.advance();
            self.expect(JToken::RBracket)?;
            ty = JType::Array(Box::new(ty));
        }
        Ok(ty)
    }

    fn parse_block_body(&mut self) -> BxResult<Vec<JStmt>> {
        let mut stmts = Vec::new();
        while !matches!(self.peek(), JToken::RBrace | JToken::Eof) {
            if let Some(s) = self.parse_stmt()? { stmts.push(s); }
        }
        Ok(stmts)
    }

    fn parse_stmt(&mut self) -> BxResult<Option<JStmt>> {
        match self.peek() {
            JToken::Semi => { self.advance(); Ok(None) }
            JToken::LBrace => { self.advance(); let b = self.parse_block_body()?; self.expect(JToken::RBrace)?; Ok(Some(JStmt::Block(b))) }
            JToken::If => { let s = self.parse_if()?; Ok(Some(s)) }
            JToken::While => { let s = self.parse_while()?; Ok(Some(s)) }
            JToken::For => { let s = self.parse_for()?; Ok(Some(s)) }
            JToken::Return => { let s = self.parse_return()?; Ok(Some(s)) }
            JToken::Break => { self.advance(); self.expect(JToken::Semi)?; Ok(Some(JStmt::Break)) }
            JToken::Continue => { self.advance(); self.expect(JToken::Semi)?; Ok(Some(JStmt::Continue)) }
            JToken::Throw => { self.advance(); let e = self.parse_expr()?; self.expect(JToken::Semi)?; Ok(Some(JStmt::Throw(e))) }
            JToken::Try => { let s = self.parse_try()?; Ok(Some(s)) }
            _ => {
                // Could be local decl or expression statement
                // Heuristic: if next-next-next is LParen, it's a method call (statement)
                // Otherwise try local decl
                let ty_or_expr_start = self.peek().clone();
                if let JToken::Ident(_) = ty_or_expr_start {
                    // Try local decl: type ident (= expr)? ;
                    let saved_pos = self.pos;
                    if let Ok(ty) = self.parse_type() {
                        if matches!(self.peek(), JToken::Ident(_)) {
                            let name = self.expect_ident()?;
                            let init = if matches!(self.peek(), JToken::Eq) {
                                self.advance();
                                Some(self.parse_expr()?)
                            } else { None };
                            self.expect(JToken::Semi)?;
                            return Ok(Some(JStmt::LocalDecl { ty, name, init }));
                        }
                    }
                    self.pos = saved_pos;
                }
                let e = self.parse_expr()?;
                self.expect(JToken::Semi)?;
                Ok(Some(JStmt::Expr(e)))
            }
        }
    }

    fn parse_if(&mut self) -> BxResult<JStmt> {
        self.advance(); // if
        let cond = self.parse_expr()?;
        self.expect(JToken::LBrace)?;
        let then_body = self.parse_block_body()?;
        self.expect(JToken::RBrace)?;
        let else_body = if matches!(self.peek(), JToken::Else) {
            self.advance();
            self.expect(JToken::LBrace)?;
            let b = self.parse_block_body()?;
            self.expect(JToken::RBrace)?;
            Some(b)
        } else { None };
        Ok(JStmt::If { cond, then_body, else_body })
    }

    fn parse_while(&mut self) -> BxResult<JStmt> {
        self.advance();
        let cond = self.parse_expr()?;
        self.expect(JToken::LBrace)?;
        let body = self.parse_block_body()?;
        self.expect(JToken::RBrace)?;
        Ok(JStmt::While { cond, body })
    }

    fn parse_for(&mut self) -> BxResult<JStmt> {
        self.advance();
        self.expect(JToken::LParen)?;
        let init = if matches!(self.peek(), JToken::Semi) { self.advance(); None }
                   else { let s = self.parse_stmt()?; s };
        let cond = if matches!(self.peek(), JToken::Semi) { self.advance(); None }
                   else { let e = self.parse_expr()?; self.expect(JToken::Semi)?; Some(e) };
        let update = if matches!(self.peek(), JToken::RParen) { None }
                    else { let e = self.parse_expr()?; Some(e) };
        self.expect(JToken::RParen)?;
        self.expect(JToken::LBrace)?;
        let body = self.parse_block_body()?;
        self.expect(JToken::RBrace)?;
        Ok(JStmt::For { init: init.map(Box::new), cond, update, body })
    }

    fn parse_return(&mut self) -> BxResult<JStmt> {
        self.advance();
        let val = if matches!(self.peek(), JToken::Semi) { None } else { Some(self.parse_expr()?) };
        self.expect(JToken::Semi)?;
        Ok(JStmt::Return(val))
    }

    fn parse_try(&mut self) -> BxResult<JStmt> {
        self.advance();
        self.expect(JToken::LBrace)?;
        let body = self.parse_block_body()?;
        self.expect(JToken::RBrace)?;
        let mut catches = Vec::new();
        while matches!(self.peek(), JToken::Catch) {
            self.advance();
            self.expect(JToken::LParen)?;
            let ct = if matches!(self.peek(), JToken::RParen) { None } else { Some(self.expect_ident()?) };
            let name = if matches!(self.peek(), JToken::RParen) { "e".to_string() } else { self.expect_ident()? };
            self.expect(JToken::RParen)?;
            self.expect(JToken::LBrace)?;
            let cb = self.parse_block_body()?;
            self.expect(JToken::RBrace)?;
            catches.push(JCatch { catch_type: ct, name, body: cb });
        }
        let finally = if matches!(self.peek(), JToken::Finally) {
            self.advance();
            self.expect(JToken::LBrace)?;
            let fb = self.parse_block_body()?;
            self.expect(JToken::RBrace)?;
            Some(fb)
        } else { None };
        Ok(JStmt::Try { body, catches, finally })
    }

    fn parse_expr(&mut self) -> BxResult<JExpr> {
        self.parse_binop_rhs(JBinOp::Or)
    }

    fn parse_binop_rhs(&mut self, _min_prec: JBinOp) -> BxResult<JExpr> {
        // Stub: just parse one primary for now.
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> BxResult<JExpr> {
        match self.peek().clone() {
            JToken::IntLit(n) => { self.advance(); Ok(JExpr::IntLit(n)) }
            JToken::FloatLit(b) => { self.advance(); Ok(JExpr::FloatLit(b)) }
            JToken::StrLit(s) => { self.advance(); Ok(JExpr::StrLit(s)) }
            JToken::CharLit(c) => { self.advance(); Ok(JExpr::IntLit(c as i64)) }
            JToken::True => { self.advance(); Ok(JExpr::BoolLit(true)) }
            JToken::False => { self.advance(); Ok(JExpr::BoolLit(false)) }
            JToken::Null => { self.advance(); Ok(JExpr::Null) }
            JToken::This => { self.advance(); Ok(JExpr::This) }
            JToken::New => {
                self.advance();
                let name = self.expect_ident()?;
                self.expect(JToken::LParen)?;
                let mut args = Vec::new();
                if !matches!(self.peek(), JToken::RParen) {
                    loop { args.push(self.parse_expr()?); if !matches!(self.peek(), JToken::Comma) { break; } self.advance(); }
                }
                self.expect(JToken::RParen)?;
                Ok(JExpr::New(name, args))
            }
            JToken::Ident(n) => { self.advance(); Ok(JExpr::Name(n)) }
            JToken::LParen => { self.advance(); let e = self.parse_expr()?; self.expect(JToken::RParen)?; Ok(e) }
            _ => Err(crate::bmo_gpu::BxError::InvalidArgument),
        }
    }

    fn peek(&self) -> &JToken { &self.tokens[self.pos] }
    fn advance(&mut self) -> JToken {
        let t = self.tokens[self.pos].clone();
        if self.pos < self.tokens.len() - 1 { self.pos += 1; }
        t
    }
    fn expect(&mut self, want: JToken) -> BxResult<()> {
        if core::mem::discriminant(self.peek()) == core::mem::discriminant(&want) { self.advance(); Ok(()) }
        else { Err(crate::bmo_gpu::BxError::InvalidArgument) }
    }
    fn expect_ident(&mut self) -> BxResult<String> {
        if let JToken::Ident(n) = self.peek() { let n = n.clone(); self.advance(); Ok(n) }
        else { Err(crate::bmo_gpu::BxError::InvalidArgument) }
    }
}

