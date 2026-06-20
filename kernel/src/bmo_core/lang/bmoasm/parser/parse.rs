//! Parser recursive-descent para BMO Simple v0.3.0.
//! Soporta: incluye, cuando, atomico, volatil, acquire, release, barr.
//! Retorna `ParseError` con línea/columna.

extern crate alloc;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use super::ast::{Ast, Stmt, Expr, Type, BinOp, CpuFlag, MemOrder};
use super::error::{ParseError, ParseResult};
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

    fn tok_name(kind: TokenKind) -> &'static str {
        match kind {
            TokenKind::KwDef => "'def'", TokenKind::KwLet => "'let'",
            TokenKind::KwSi => "'si'", TokenKind::KwSino => "'sino'",
            TokenKind::KwMientras => "'mientras'", TokenKind::KwRetorna => "'retorna'",
            TokenKind::KwReg => "'reg'", TokenKind::KwEmit => "'emit'",
            TokenKind::KwAloc => "'aloc'", TokenKind::KwLibre => "'libre'",
            TokenKind::KwRompe => "'rompe'", TokenKind::KwContinua => "'continua'",
            TokenKind::KwMatch => "'match'", TokenKind::KwCaso => "'caso'",
            TokenKind::KwDefecto => "'defecto'", TokenKind::KwPara => "'para'",
            TokenKind::KwDesde => "'desde'", TokenKind::KwHasta => "'hasta'",
            TokenKind::KwPaso => "'paso'", TokenKind::KwBucle => "'bucle'",
            TokenKind::KwEtiqueta => "'etiqueta'", TokenKind::KwSalto => "'salto'",
            TokenKind::KwSyscall => "'syscall'", TokenKind::KwIncluye => "'incluye'",
            TokenKind::KwCuando => "'cuando'", TokenKind::KwAtomico => "'atomico'",
            TokenKind::KwVolatil => "'volatil'", TokenKind::KwAcquire => "'acquire'",
            TokenKind::KwRelease => "'release'", TokenKind::KwBarr => "'barr'",
            TokenKind::LBrace => "'{'", TokenKind::RBrace => "'}'",
            TokenKind::LParen => "'('", TokenKind::RParen => "')'",
            TokenKind::LBracket => "'['", TokenKind::RBracket => "']'",
            TokenKind::Comma => "','", TokenKind::Colon => "':'",
            TokenKind::Semicolon => "';'", TokenKind::Arrow => "'->'",
            TokenKind::Assign => "'='", TokenKind::Dot => "'.'",
            TokenKind::Ident => "identifier", TokenKind::LitInt => "integer",
            TokenKind::LitHex => "hex literal", TokenKind::LitBin => "binary literal",
            TokenKind::LitStr => "string literal", TokenKind::LitByte => "byte literal",
            TokenKind::LitNulo => "'nulo'", TokenKind::Eof => "end of file",
            TokenKind::TyByte => "'byte'", TokenKind::TyNum => "'num'",
            TokenKind::TyPtr => "'ptr'", TokenKind::TyArr => "'arr'",
            TokenKind::TyRef => "'ref'",
            TokenKind::OpSuma => "'suma'", TokenKind::OpResta => "'resta'",
            TokenKind::OpMult => "'mult'", TokenKind::OpDiv => "'div'",
            TokenKind::OpMod => "'mod'", TokenKind::OpY => "'y'",
            TokenKind::OpO => "'o'", TokenKind::OpXor => "'xor'",
            TokenKind::OpShl => "'shl'", TokenKind::OpShr => "'shr'",
            TokenKind::OpIgual => "'igual'", TokenKind::OpMayor => "'mayor'",
            TokenKind::OpMenor => "'menor'", TokenKind::OpNo => "'no'",
            TokenKind::FlagCf => "'cf'", TokenKind::FlagZf => "'zf'",
            TokenKind::FlagSf => "'sf'", TokenKind::FlagOf => "'of'",
            TokenKind::FlagPf => "'pf'", TokenKind::FlagDf => "'df'",
            _ => "token",
        }
    }

    fn error(&self, msg: &'static str) -> ParseError {
        let (line, col) = self.scanner.current_loc();
        ParseError::new(line, col, msg)
            .with_found(Self::tok_name(self.current.kind))
    }

    fn expect(&mut self, kind: TokenKind) -> ParseResult<()> {
        if self.current.kind == kind {
            self.advance();
            Ok(())
        } else {
            let (line, col) = self.scanner.current_loc();
            Err(ParseError::new(line, col, "unexpected token")
                .with_expected(Self::tok_name(kind))
                .with_found(Self::tok_name(self.current.kind)))
        }
    }

    fn expect_ident(&mut self) -> ParseResult<String> {
        if self.current.kind == TokenKind::Ident {
            let lexeme = self.lexeme(self.current)?;
            let name = String::from_utf8(lexeme.to_vec()).unwrap_or_default();
            self.advance();
            Ok(name)
        } else {
            let (line, col) = self.scanner.current_loc();
            Err(ParseError::new(line, col, "expected identifier")
                .with_found(Self::tok_name(self.current.kind)))
        }
    }

    fn lexeme(&self, tok: Token) -> ParseResult<&'a [u8]> {
        let end = tok.start as usize + tok.len as usize;
        if end <= self.src.len() {
            Ok(&self.src[tok.start as usize..end])
        } else {
            Err(self.error("unexpected end of file"))
        }
    }

    fn expect_string(&mut self) -> ParseResult<String> {
        let lex = self.lexeme(self.current)?;
        let s = if lex.len() >= 2 && lex[0] == b'"' && lex[lex.len() - 1] == b'"' {
            String::from_utf8(lex[1..lex.len() - 1].to_vec()).unwrap_or_default()
        } else {
            String::from_utf8(lex.to_vec()).unwrap_or_default()
        };
        self.advance();
        Ok(s)
    }

    fn parse_flag(&mut self) -> ParseResult<CpuFlag> {
        let flag = match self.current.kind {
            TokenKind::FlagCf => CpuFlag::Cf,
            TokenKind::FlagZf => CpuFlag::Zf,
            TokenKind::FlagSf => CpuFlag::Sf,
            TokenKind::FlagOf => CpuFlag::Of,
            TokenKind::FlagPf => CpuFlag::Pf,
            TokenKind::FlagDf => CpuFlag::Df,
            _ => return Err(self.error("expected CPU flag (cf, zf, sf, of, pf, df)")),
        };
        self.advance();
        Ok(flag)
    }

    fn parse_block_body(&mut self) -> ParseResult<Vec<Stmt>> {
        self.expect(TokenKind::LBrace)?;
        let mut body = Vec::new();
        while self.current.kind != TokenKind::RBrace && self.current.kind != TokenKind::Eof {
            body.push(self.parse_stmt()?);
        }
        self.expect(TokenKind::RBrace)?;
        Ok(body)
    }

    /// Parsea el código fuente completo en un AST.
    pub fn parse(&mut self) -> ParseResult<Ast> {
        let mut items = Vec::new();
        while self.current.kind != TokenKind::Eof {
            match self.current.kind {
                TokenKind::KwAlign => {
                    self.advance();
                    self.expect(TokenKind::LitInt)?;
                }
                TokenKind::KwDef => {
                    items.push(self.parse_def()?);
                }
                TokenKind::KwIncluye => {
                    self.advance();
                    let path = self.expect_string()?;
                    items.push(Stmt::Incluye(path));
                }
                TokenKind::KwFnForward => {
                    items.push(self.parse_fn_forward()?);
                }
                TokenKind::Comment => {
                    self.advance();
                }
                _ => {
                    self.advance();
                }
            }
        }
        Ok(Ast { items })
    }

    fn parse_def(&mut self) -> ParseResult<Stmt> {
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

        let body = self.parse_block_body()?;

        Ok(Stmt::Def { name, params, ret, body })
    }

    fn parse_fn_forward(&mut self) -> ParseResult<Stmt> {
        self.advance(); // consume KwFnForward (or use 'fn' keyword)
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
        Ok(Stmt::FnForward { name, params, ret })
    }

    fn parse_type(&mut self) -> ParseResult<Type> {
        let ty = match self.current.kind {
            TokenKind::TyByte => Type::Byte,
            TokenKind::TyNum => Type::Num,
            TokenKind::TyPtr => Type::Ptr,
            TokenKind::TyArr => Type::Arr,
            TokenKind::TyRef => Type::Ref,
            _ => return Err(self.error("expected type (byte, num, ptr, arr, ref)")),
        };
        self.advance();
        Ok(ty)
    }

    fn parse_stmt(&mut self) -> ParseResult<Stmt> {
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
                self.advance();
                let reg_name = self.expect_ident()?;
                self.expect(TokenKind::Assign)?;
                let value = self.parse_expr(0)?;
                Ok(Stmt::RegAssign { reg: reg_name, value })
            }
            TokenKind::KwRetorna => {
                self.advance();
                let mut expr = None;
                if self.current.kind != TokenKind::RBrace
                    && self.current.kind != TokenKind::Semicolon
                    && self.current.kind != TokenKind::Eof
                {
                    expr = Some(self.parse_expr(0)?);
                }
                Ok(Stmt::Retorna(expr))
            }
            TokenKind::KwSi => {
                self.advance();
                let cond = self.parse_expr(0)?;
                let then_body = self.parse_block_body()?;
                let mut else_body = None;
                if self.current.kind == TokenKind::KwSino {
                    self.advance();
                    else_body = Some(self.parse_block_body()?);
                }
                Ok(Stmt::Si { cond, then_body, else_body })
            }
            TokenKind::KwMientras => {
                self.advance();
                let cond = self.parse_expr(0)?;
                let body = self.parse_block_body()?;
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
            TokenKind::KwRompe => { self.advance(); Ok(Stmt::Rompe) }
            TokenKind::KwContinua => { self.advance(); Ok(Stmt::Continua) }
            TokenKind::KwBarr => { self.advance(); Ok(Stmt::Barr) }
            TokenKind::KwIncluye => {
                self.advance();
                let path = self.expect_string()?;
                Ok(Stmt::Incluye(path))
            }
            TokenKind::KwCuando => {
                self.advance();
                let flag = self.parse_flag()?;
                let then_body = self.parse_block_body()?;
                let mut else_body = None;
                if self.current.kind == TokenKind::KwSino {
                    self.advance();
                    else_body = Some(self.parse_block_body()?);
                }
                Ok(Stmt::CuandoSino { flag, then_body, else_body })
            }
            TokenKind::KwAtomico => {
                self.advance();
                let body = self.parse_block_body()?;
                Ok(Stmt::Atomico(body))
            }
            TokenKind::KwVolatil => {
                self.advance();
                let expr = self.parse_expr(4)?;
                Ok(Stmt::Volatil(expr))
            }
            TokenKind::KwMatch => {
                self.advance();
                let expr = self.parse_expr(0)?;
                self.expect(TokenKind::LBrace)?;
                let mut arms = Vec::new();
                let mut default = None;
                while self.current.kind != TokenKind::RBrace && self.current.kind != TokenKind::Eof {
                    if self.current.kind == TokenKind::KwDefecto {
                        self.advance();
                        self.expect(TokenKind::Arrow)?;
                        let body = self.parse_block_body()?;
                        default = Some(body);
                    } else if self.current.kind == TokenKind::KwCaso {
                        self.advance();
                        let pattern = self.parse_expr(0)?;
                        self.expect(TokenKind::Arrow)?;
                        let body = self.parse_block_body()?;
                        arms.push((pattern, body));
                    } else {
                        self.advance();
                    }
                }
                self.expect(TokenKind::RBrace)?;
                Ok(Stmt::Match { expr, arms, default })
            }
            TokenKind::KwPara => {
                self.advance();
                let var = self.expect_ident()?;
                self.expect(TokenKind::KwDesde)?;
                let desde = self.parse_expr(0)?;
                self.expect(TokenKind::KwHasta)?;
                let hasta = self.parse_expr(0)?;
                let mut paso = None;
                if self.current.kind == TokenKind::KwPaso {
                    self.advance();
                    paso = Some(self.parse_expr(0)?);
                }
                let body = self.parse_block_body()?;
                Ok(Stmt::Para { var, desde, hasta, paso, body })
            }
            TokenKind::KwBucle => {
                self.advance();
                let body = self.parse_block_body()?;
                Ok(Stmt::Bucle(body))
            }
            TokenKind::KwEtiqueta => {
                self.advance();
                let name = self.expect_ident()?;
                Ok(Stmt::Etiqueta(name))
            }
            TokenKind::KwSalto => {
                self.advance();
                let name = self.expect_ident()?;
                Ok(Stmt::Salto(name))
            }
            TokenKind::KwSyscall => {
                self.advance();
                Ok(Stmt::ExprStmt(Expr::Reg(String::from("syscall"))))
            }
            TokenKind::KwNop | TokenKind::KwPausa | TokenKind::KwInt3
            | TokenKind::KwHlt | TokenKind::KwCli | TokenKind::KwSti
            | TokenKind::KwRdtsc | TokenKind::KwCpuid
            | TokenKind::KwLfence | TokenKind::KwMfence | TokenKind::KwSfence =>
            {
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

    fn parse_expr(&mut self, min_prec: i32) -> ParseResult<Expr> {
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

    fn parse_primary(&mut self) -> ParseResult<Expr> {
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
            TokenKind::FlagCf | TokenKind::FlagZf | TokenKind::FlagSf
            | TokenKind::FlagOf | TokenKind::FlagPf | TokenKind::FlagDf => {
                let flag = self.parse_flag()?;
                Ok(Expr::Flag(flag))
            }
            TokenKind::KwVolatil => {
                self.advance();
                let expr = self.parse_expr(4)?;
                Ok(Expr::MemOrder(MemOrder::Volatil, Box::new(expr)))
            }
            TokenKind::KwAcquire => {
                self.advance();
                let expr = self.parse_expr(4)?;
                Ok(Expr::MemOrder(MemOrder::Acquire, Box::new(expr)))
            }
            TokenKind::KwRelease => {
                self.advance();
                let expr = self.parse_expr(4)?;
                Ok(Expr::MemOrder(MemOrder::Release, Box::new(expr)))
            }
            TokenKind::Ident => {
                let name = self.expect_ident()?;
                if self.current.kind == TokenKind::LParen {
                    self.advance();
                    let mut args = Vec::new();
                    if self.current.kind != TokenKind::RParen {
                        loop {
                            args.push(self.parse_expr(0)?);
                            if self.current.kind == TokenKind::Comma {
                                self.advance();
                            } else {
                                break;
                            }
                        }
                    }
                    self.expect(TokenKind::RParen)?;
                    Ok(Expr::Call { name, args })
                } else {
                    Ok(Expr::Ident(name))
                }
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
            _ => Err(self.error("expected expression")),
        }
    }
}

fn get_precedence(kind: TokenKind) -> i32 {
    match kind {
        TokenKind::OpY | TokenKind::OpO => 1,
        TokenKind::OpIgual | TokenKind::OpMayor | TokenKind::OpMenor => 2,
        TokenKind::OpXor | TokenKind::OpShl | TokenKind::OpShr
        | TokenKind::OpRol | TokenKind::OpRor => 3,
        TokenKind::OpSuma | TokenKind::OpResta => 4,
        TokenKind::OpMult | TokenKind::OpDiv | TokenKind::OpMod => 5,
        _ => -1,
    }
}

fn to_bin_op(kind: TokenKind) -> ParseResult<BinOp> {
    Ok(match kind {
        TokenKind::OpSuma => BinOp::Suma,
        TokenKind::OpResta => BinOp::Resta,
        TokenKind::OpMult => BinOp::Mult,
        TokenKind::OpDiv => BinOp::Div,
        TokenKind::OpMod => BinOp::Mod,
        TokenKind::OpY => BinOp::Y,
        TokenKind::OpO => BinOp::O,
        TokenKind::OpXor => BinOp::Xor,
        TokenKind::OpShl => BinOp::Shl,
        TokenKind::OpShr => BinOp::Shr,
        TokenKind::OpIgual => BinOp::Igual,
        TokenKind::OpMayor => BinOp::Mayor,
        TokenKind::OpMenor => BinOp::Menor,
        _ => return Err(ParseError::new(0, 0, "unknown operator")),
    })
}
