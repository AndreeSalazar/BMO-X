//! Parser recursive-descent para BMO Simple.
//! Parsea declaraciones de funciones, let, si/sino, mientras, reg, emit, match, etc.

extern crate alloc;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;

use crate::barex::{BxError, BxResult};
use super::ast::{Ast, Stmt, Expr, Type, BinOp};
use super::super::lexer::{Token, TokenKind, Scanner};

pub struct Parser<'a> {
    scanner: Scanner<'a>,
    current: Token,
    peek: Token,
    src: &'a [u8],
}

impl<'a> Parser<'a> {
    pub fn new(src: &'a [u8]) -> Self {
        let mut scanner = Scanner::new(src);
        let current = scanner.next_token();
        let peek = scanner.next_token();
        Self { scanner, current, peek, src }
    }

    fn advance(&mut self) {
        self.current = self.peek;
        self.peek = self.scanner.next_token();
    }

    fn expect(&mut self, kind: TokenKind) -> BxResult<()> {
        if self.current.kind == kind {
            self.advance();
            Ok(())
        } else {
            Err(BxError::InvalidArgument)
        }
    }

    fn expect_ident(&mut self) -> BxResult<String> {
        if self.current.kind == TokenKind::Ident {
            let lexeme = self.lexeme(self.current)?;
            let name = String::from_utf8(lexeme.to_vec()).unwrap_or_default();
            self.advance();
            Ok(name)
        } else {
            Err(BxError::InvalidArgument)
        }
    }

    fn lexeme(&self, tok: Token) -> BxResult<&'a [u8]> {
        let end = tok.start as usize + tok.len as usize;
        if end <= self.src.len() {
            Ok(&self.src[tok.start as usize..end])
        } else {
            Err(BxError::InvalidArgument)
        }
    }

    /// Parsea el código fuente completo en un AST.
    pub fn parse(&mut self) -> BxResult<Ast> {
        let mut items = Vec::new();
        // Omitir directivas iniciales o comments si los hay
        while self.current.kind != TokenKind::Eof {
            if self.current.kind == TokenKind::KwAlign {
                self.advance();
                self.expect(TokenKind::LitInt)?;
            } else if self.current.kind == TokenKind::KwDef {
                items.push(self.parse_def()?);
            } else if self.current.kind == TokenKind::Comment {
                self.advance();
            } else {
                // Si hay cosas no reconocidas a nivel top-level, avanzamos para no congelar
                self.advance();
            }
        }
        Ok(Ast { items })
    }

    // def nombre(p1: tipo, p2: tipo) -> tipo { body }
    fn parse_def(&mut self) -> BxResult<Stmt> {
        self.expect(TokenKind::KwDef)?;
        let name = self.expect_ident()?;
        self.expect(TokenKind::LParen)?;
        
        let mut params = Vec::new();
        while self.current.kind != TokenKind::RParen && self.current.kind != TokenKind::Eof {
            let p_name = self.expect_ident()?;
            self.expect(TokenKind::Colon)?;
            let p_ty = self.parse_type()?;
            params.push((p_name, p_ty));
            if self.current.kind == TokenKind::Comma {
                self.advance();
            } else {
                break;
            }
        }
        self.expect(TokenKind::RParen)?;

        let mut ret = Type::Void;
        if self.current.kind == TokenKind::Arrow {
            self.advance();
            ret = self.parse_type()?;
        }

        self.expect(TokenKind::LBrace)?;
        let mut body = Vec::new();
        while self.current.kind != TokenKind::RBrace && self.current.kind != TokenKind::Eof {
            body.push(self.parse_stmt()?);
        }
        self.expect(TokenKind::RBrace)?;

        Ok(Stmt::Def { name, params, ret, body })
    }

    fn parse_type(&mut self) -> BxResult<Type> {
        let ty = match self.current.kind {
            TokenKind::TyByte => Type::Byte,
            TokenKind::TyNum => Type::Num,
            TokenKind::TyPtr => Type::Ptr,
            TokenKind::TyArr => Type::Arr,
            TokenKind::TyRef => Type::Ref,
            _ => return Err(BxError::InvalidArgument),
        };
        self.advance();
        Ok(ty)
    }

    fn parse_stmt(&mut self) -> BxResult<Stmt> {
        match self.current.kind {
            TokenKind::KwLet => {
                self.advance();
                let name = self.expect_ident()?;
                let mut ty = None;
                if self.current.kind == TokenKind::Colon {
                    self.advance();
                    ty = Some(self.parse_type()?);
                }
                self.expect(TokenKind::Assign)?;
                let value = self.parse_expr(0)?;
                Ok(Stmt::Let { name, ty, value })
            }
            TokenKind::KwReg => {
                // reg rdi = expr o reg rdi = "hola"
                self.advance();
                let reg_name = self.expect_ident()?;
                self.expect(TokenKind::Assign)?;
                let value = self.parse_expr(0)?;
                Ok(Stmt::RegAssign { reg: reg_name, value })
            }
            TokenKind::KwRetorna => {
                self.advance();
                let mut expr = None;
                // Si no hay cierre de bloque, intenta parsear expresión de retorno
                if self.current.kind != TokenKind::RBrace && self.current.kind != TokenKind::Semicolon && self.current.kind != TokenKind::Eof {
                    expr = Some(self.parse_expr(0)?);
                }
                Ok(Stmt::Retorna(expr))
            }
            TokenKind::KwSi => {
                self.advance();
                let cond = self.parse_expr(0)?;
                self.expect(TokenKind::LBrace)?;
                let mut then_body = Vec::new();
                while self.current.kind != TokenKind::RBrace && self.current.kind != TokenKind::Eof {
                    then_body.push(self.parse_stmt()?);
                }
                self.expect(TokenKind::RBrace)?;
                
                let mut else_body = None;
                if self.current.kind == TokenKind::KwSino {
                    self.advance();
                    self.expect(TokenKind::LBrace)?;
                    let mut e_body = Vec::new();
                    while self.current.kind != TokenKind::RBrace && self.current.kind != TokenKind::Eof {
                        e_body.push(self.parse_stmt()?);
                    }
                    self.expect(TokenKind::RBrace)?;
                    else_body = Some(e_body);
                }
                Ok(Stmt::Si { cond, then_body, else_body })
            }
            TokenKind::KwMientras => {
                self.advance();
                let cond = self.parse_expr(0)?;
                self.expect(TokenKind::LBrace)?;
                let mut body = Vec::new();
                while self.current.kind != TokenKind::RBrace && self.current.kind != TokenKind::Eof {
                    body.push(self.parse_stmt()?);
                }
                self.expect(TokenKind::RBrace)?;
                Ok(Stmt::Mientras { cond, body })
            }
            TokenKind::KwEmit => {
                self.advance();
                let mut bytes = Vec::new();
                while self.current.kind == TokenKind::LitHex || self.current.kind == TokenKind::LitInt {
                    bytes.push(self.current.value as u8);
                    self.advance();
                }
                Ok(Stmt::Emit(bytes))
            }
            TokenKind::KwLibre => {
                self.advance();
                let expr = self.parse_expr(0)?;
                Ok(Stmt::Libre(expr))
            }
            TokenKind::KwRompe => {
                self.advance();
                Ok(Stmt::Rompe)
            }
            TokenKind::KwContinua => {
                self.advance();
                Ok(Stmt::Continua)
            }
            TokenKind::KwSyscall => {
                self.advance();
                Ok(Stmt::ExprStmt(Expr::Reg(String::from("syscall"))))
            }
            TokenKind::KwNop | TokenKind::KwPausa | TokenKind::KwInt3 | TokenKind::KwHlt | TokenKind::KwCli | TokenKind::KwSti | TokenKind::KwRdtsc | TokenKind::KwCpuid | TokenKind::KwLfence | TokenKind::KwMfence | TokenKind::KwSfence => {
                let name = String::from_utf8(self.lexeme(self.current)?.to_vec()).unwrap_or_default();
                self.advance();
                Ok(Stmt::ExprStmt(Expr::Reg(name)))
            }
            TokenKind::Comment => {
                self.advance();
                self.parse_stmt()
            }
            _ => {
                let expr = self.parse_expr(0)?;
                Ok(Stmt::ExprStmt(expr))
            }
        }
    }

    fn parse_expr(&mut self, min_prec: i32) -> BxResult<Expr> {
        let mut left = self.parse_primary()?;
        loop {
            let prec = get_precedence(self.current.kind);
            if prec < min_prec {
                break;
            }
            let op = self.current.kind;
            self.advance();
            let right = self.parse_expr(prec + 1)?;
            left = Expr::Bin(to_bin_op(op)?, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_primary(&mut self) -> BxResult<Expr> {
        match self.current.kind {
            TokenKind::LitInt | TokenKind::LitHex | TokenKind::LitBin => {
                let val = self.current.value;
                self.advance();
                Ok(Expr::LitInt(val))
            }
            TokenKind::LitByte => {
                let val = self.current.value as u8;
                self.advance();
                Ok(Expr::LitByte(val))
            }
            TokenKind::LitNulo => {
                self.advance();
                Ok(Expr::LitNulo)
            }
            TokenKind::LitStr => {
                let lex = self.lexeme(self.current)?;
                // Remover comillas
                let s = if lex.len() >= 2 && lex[0] == b'"' && lex[lex.len() - 1] == b'"' {
                    String::from_utf8(lex[1..lex.len() - 1].to_vec()).unwrap_or_default()
                } else {
                    String::from_utf8(lex.to_vec()).unwrap_or_default()
                };
                self.advance();
                Ok(Expr::LitStr(s))
            }
            TokenKind::KwReg => {
                self.advance();
                let r_name = self.expect_ident()?;
                Ok(Expr::Reg(r_name))
            }
            TokenKind::Ident => {
                let name = self.expect_ident()?;
                Ok(Expr::Ident(name))
            }
            TokenKind::LParen => {
                self.advance();
                let expr = self.parse_expr(0)?;
                self.expect(TokenKind::RParen)?;
                Ok(expr)
            }
            TokenKind::OpNo => {
                self.advance();
                let expr = self.parse_expr(4)?;
                Ok(Expr::No(Box::new(expr)))
            }
            TokenKind::KwAloc => {
                self.advance();
                let expr = self.parse_expr(4)?;
                Ok(Expr::Aloc(Box::new(expr)))
            }
            _ => Err(BxError::InvalidArgument),
        }
    }
}

fn get_precedence(kind: TokenKind) -> i32 {
    match kind {
        TokenKind::OpY | TokenKind::OpO => 1,
        TokenKind::OpIgual | TokenKind::OpMayor | TokenKind::OpMenor => 2,
        TokenKind::OpXor | TokenKind::OpShl | TokenKind::OpShr | TokenKind::OpRol | TokenKind::OpRor => 3,
        TokenKind::OpSuma | TokenKind::OpResta => 4,
        TokenKind::OpMult | TokenKind::OpDiv | TokenKind::OpMod => 5,
        _ => -1,
    }
}

fn to_bin_op(kind: TokenKind) -> BxResult<BinOp> {
    Ok(match kind {
        TokenKind::OpSuma => BinOp::Suma,
        TokenKind::OpResta => BinOp::Resta,
        TokenKind::OpMult => BinOp::Mult,
        TokenKind::OpDiv => BinOp::Div,
        TokenKind::OpY => BinOp::Y,
        TokenKind::OpO => BinOp::O,
        TokenKind::OpIgual => BinOp::Igual,
        TokenKind::OpMayor => BinOp::Mayor,
        TokenKind::OpMenor => BinOp::Menor,
        _ => return Err(BxError::InvalidArgument),
    })
}
