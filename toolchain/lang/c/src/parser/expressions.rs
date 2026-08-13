//! **EXPRESSIONS** -- the precedence ladder, from comma down to primary.
//!
//! === Why this is a file of its own ===
//!
//! Because the seventeen methods in here are **one algorithm**, not seventeen
//! decisions. Recursive descent encodes precedence as call depth: `parse_expr`
//! calls `parse_comma` calls `parse_assign` ... down to `parse_primary`, and
//! the order of that chain **is** C's precedence table. Get one link out of
//! order and the language quietly changes meaning.
//!
//! Spread through a 2,700-line file, the chain was a chain you had to
//! reconstruct by searching. Here it reads top to bottom in the order it runs.
//!
//! === ** The one that has already been paid for ===
//!
//! `parse_assign` is where the compound forms are desugared, and it holds an
//! open defect that `probe_assignment` records: `a[i++] += 1` clones the index
//! into both the read and the write, so `i` advances **twice**. C11 6.5.16.2p3
//! says the lvalue is evaluated exactly once.
//!
//! It is left open deliberately --measured: DOOM does not use that shape-- and
//! fixing it means evaluating the lvalue's ADDRESS once, which is a new AST
//! node and six arms. The row in the census carries the symptom.

use super::*;

impl Parser {
    // ---- Expressions (precedence climbing) ----
    pub(super) fn parse_expr(&mut self) -> Result<Expr, CError> {
        self.parse_comma()
    }

    pub(super) fn parse_comma(&mut self) -> Result<Expr, CError> {
        let mut exprs = vec![self.parse_assign()?];
        while *self.peek() == Token::Comma { self.advance(); exprs.push(self.parse_assign()?); }
        if exprs.len() == 1 { Ok(exprs.into_iter().next().unwrap()) } else { Ok(Expr::Comma(exprs)) }
    }

    pub(super) fn parse_assign(&mut self) -> Result<Expr, CError> {
        let expr = self.parse_conditional()?;
        let assign_op = |n: String, val: Expr, op: fn(Box<Expr>, Box<Expr>) -> Expr| {
            let n2 = n.clone(); Expr::Assign(n, Box::new(op(Box::new(Expr::Var(n2)), Box::new(val))))
        };
        let field_assign_op = |e: Expr, f: String, off: u32, ft: TypeSpec, val: Expr, op: fn(Box<Expr>, Box<Expr>) -> Expr| {
            let lhs = Expr::Field(Box::new(e.clone()), f.clone(), off, ft.clone());
            Expr::AssignField(Box::new(e), f, off, ft, Box::new(op(Box::new(lhs), Box::new(val))))
        };
        let arrow_assign_op = |e: Box<Expr>, f: String, off: u32, ft: TypeSpec, val: Expr, op: fn(Box<Expr>, Box<Expr>) -> Expr| {
            Expr::AssignArrow(e.clone(), f.clone(), off, ft.clone(), Box::new(op(Box::new(Expr::Arrow(e, f, off, ft)), Box::new(val))))
        };
        let sub_assign_op = |n: String, idx: Box<Expr>, sc: u32, val: Expr, op: fn(Box<Expr>, Box<Expr>) -> Expr| {
            let lhs = Expr::Subscript(n.clone(), idx.clone(), sc);
            Expr::AssignSubscript(n, idx, sc, Box::new(op(Box::new(lhs), Box::new(val))))
        };
        let idxptr_assign_op = |b: Box<Expr>, idx: Box<Expr>, ty: TypeSpec, val: Expr, op: fn(Box<Expr>, Box<Expr>) -> Expr| {
            let lhs = Expr::IndexPtr(b.clone(), idx.clone(), ty.clone());
            Expr::AssignIndexPtr(b, idx, ty, Box::new(op(Box::new(lhs), Box::new(val))))
        };
        match self.peek() {
            Token::Assign => { self.advance(); let val = self.parse_assign()?; match expr {
                Expr::Var(n) => Ok(Expr::Assign(n, Box::new(val))),
                Expr::Deref(a) => Ok(Expr::AssignDeref(a, Box::new(val))),
                Expr::Field(e, f, off, ft) => Ok(Expr::AssignField(e, f, off, ft, Box::new(val))),
                Expr::Arrow(e, f, off, ft) => Ok(Expr::AssignArrow(e, f, off, ft, Box::new(val))),
                Expr::Subscript(n, idx, sc) => Ok(Expr::AssignSubscript(n, idx, sc, Box::new(val))),
                Expr::IndexPtr(b, idx, ty) => Ok(Expr::AssignIndexPtr(b, idx, ty, Box::new(val))),
                _ => Ok(val),
            }}
            Token::AddAssign => { self.advance(); let val = self.parse_assign()?; match expr {
                Expr::Var(n) => Ok(assign_op(n, val, Expr::Add)),
                Expr::Field(e, f, off, ft) => Ok(field_assign_op(*e, f, off, ft, val, Expr::Add)),
                Expr::Arrow(e, f, off, ft) => Ok(arrow_assign_op(e, f, off, ft, val, Expr::Add)),
                Expr::Subscript(n, idx, sc) => Ok(sub_assign_op(n, idx, sc, val, Expr::Add)),
                Expr::IndexPtr(b, idx, ty) => Ok(idxptr_assign_op(b, idx, ty, val, Expr::Add)),
                _ => Ok(val),
            }}
            Token::SubAssign => { self.advance(); let val = self.parse_assign()?; match expr {
                Expr::Var(n) => Ok(assign_op(n, val, Expr::Sub)),
                Expr::Field(e, f, off, ft) => Ok(field_assign_op(*e, f, off, ft, val, Expr::Sub)),
                Expr::Arrow(e, f, off, ft) => Ok(arrow_assign_op(e, f, off, ft, val, Expr::Sub)),
                Expr::Subscript(n, idx, sc) => Ok(sub_assign_op(n, idx, sc, val, Expr::Sub)),
                Expr::IndexPtr(b, idx, ty) => Ok(idxptr_assign_op(b, idx, ty, val, Expr::Sub)),
                _ => Ok(val),
            }}
            Token::MulAssign => { self.advance(); let val = self.parse_assign()?; match expr {
                Expr::Var(n) => Ok(assign_op(n, val, Expr::Mul)),
                Expr::Field(e, f, off, ft) => Ok(field_assign_op(*e, f, off, ft, val, Expr::Mul)),
                Expr::Arrow(e, f, off, ft) => Ok(arrow_assign_op(e, f, off, ft, val, Expr::Mul)),
                Expr::Subscript(n, idx, sc) => Ok(sub_assign_op(n, idx, sc, val, Expr::Mul)),
                Expr::IndexPtr(b, idx, ty) => Ok(idxptr_assign_op(b, idx, ty, val, Expr::Mul)),
                _ => Ok(val),
            }}
            Token::DivAssign => { self.advance(); let val = self.parse_assign()?; match expr {
                Expr::Var(n) => Ok(assign_op(n, val, Expr::Div)),
                Expr::Field(e, f, off, ft) => Ok(field_assign_op(*e, f, off, ft, val, Expr::Div)),
                Expr::Arrow(e, f, off, ft) => Ok(arrow_assign_op(e, f, off, ft, val, Expr::Div)),
                Expr::Subscript(n, idx, sc) => Ok(sub_assign_op(n, idx, sc, val, Expr::Div)),
                Expr::IndexPtr(b, idx, ty) => Ok(idxptr_assign_op(b, idx, ty, val, Expr::Div)),
                _ => Ok(val),
            }}
            Token::ModAssign => { self.advance(); let val = self.parse_assign()?; match expr {
                Expr::Var(n) => Ok(assign_op(n, val, Expr::Mod)),
                Expr::Field(e, f, off, ft) => Ok(field_assign_op(*e, f, off, ft, val, Expr::Mod)),
                Expr::Arrow(e, f, off, ft) => Ok(arrow_assign_op(e, f, off, ft, val, Expr::Mod)),
                Expr::Subscript(n, idx, sc) => Ok(sub_assign_op(n, idx, sc, val, Expr::Mod)),
                Expr::IndexPtr(b, idx, ty) => Ok(idxptr_assign_op(b, idx, ty, val, Expr::Mod)),
                _ => Ok(val),
            }}
            Token::ShlAssign => { self.advance(); let val = self.parse_assign()?; match expr {
                Expr::Var(n) => Ok(assign_op(n, val, Expr::Shl)),
                Expr::Field(e, f, off, ft) => Ok(field_assign_op(*e, f, off, ft, val, Expr::Shl)),
                Expr::Arrow(e, f, off, ft) => Ok(arrow_assign_op(e, f, off, ft, val, Expr::Shl)),
                Expr::Subscript(n, idx, sc) => Ok(sub_assign_op(n, idx, sc, val, Expr::Shl)),
                Expr::IndexPtr(b, idx, ty) => Ok(idxptr_assign_op(b, idx, ty, val, Expr::Shl)),
                _ => Ok(val),
            }}
            Token::ShrAssign => { self.advance(); let val = self.parse_assign()?; match expr {
                Expr::Var(n) => Ok(assign_op(n, val, Expr::Shr)),
                Expr::Field(e, f, off, ft) => Ok(field_assign_op(*e, f, off, ft, val, Expr::Shr)),
                Expr::Arrow(e, f, off, ft) => Ok(arrow_assign_op(e, f, off, ft, val, Expr::Shr)),
                Expr::Subscript(n, idx, sc) => Ok(sub_assign_op(n, idx, sc, val, Expr::Shr)),
                Expr::IndexPtr(b, idx, ty) => Ok(idxptr_assign_op(b, idx, ty, val, Expr::Shr)),
                _ => Ok(val),
            }}
            Token::AndAssign => { self.advance(); let val = self.parse_assign()?; match expr {
                Expr::Var(n) => Ok(assign_op(n, val, Expr::BitAnd)),
                Expr::Field(e, f, off, ft) => Ok(field_assign_op(*e, f, off, ft, val, Expr::BitAnd)),
                Expr::Arrow(e, f, off, ft) => Ok(arrow_assign_op(e, f, off, ft, val, Expr::BitAnd)),
                Expr::Subscript(n, idx, sc) => Ok(sub_assign_op(n, idx, sc, val, Expr::BitAnd)),
                Expr::IndexPtr(b, idx, ty) => Ok(idxptr_assign_op(b, idx, ty, val, Expr::BitAnd)),
                _ => Ok(val),
            }}
            Token::XorAssign => { self.advance(); let val = self.parse_assign()?; match expr {
                Expr::Var(n) => Ok(assign_op(n, val, Expr::BitXor)),
                Expr::Field(e, f, off, ft) => Ok(field_assign_op(*e, f, off, ft, val, Expr::BitXor)),
                Expr::Arrow(e, f, off, ft) => Ok(arrow_assign_op(e, f, off, ft, val, Expr::BitXor)),
                Expr::Subscript(n, idx, sc) => Ok(sub_assign_op(n, idx, sc, val, Expr::BitXor)),
                Expr::IndexPtr(b, idx, ty) => Ok(idxptr_assign_op(b, idx, ty, val, Expr::BitXor)),
                _ => Ok(val),
            }}
            Token::OrAssign => { self.advance(); let val = self.parse_assign()?; match expr {
                Expr::Var(n) => Ok(assign_op(n, val, Expr::BitOr)),
                Expr::Field(e, f, off, ft) => Ok(field_assign_op(*e, f, off, ft, val, Expr::BitOr)),
                Expr::Arrow(e, f, off, ft) => Ok(arrow_assign_op(e, f, off, ft, val, Expr::BitOr)),
                Expr::Subscript(n, idx, sc) => Ok(sub_assign_op(n, idx, sc, val, Expr::BitOr)),
                Expr::IndexPtr(b, idx, ty) => Ok(idxptr_assign_op(b, idx, ty, val, Expr::BitOr)),
                _ => Ok(val),
            }}
            _ => Ok(expr),
        }
    }

    pub(super) fn parse_conditional(&mut self) -> Result<Expr, CError> {
        let mut expr = self.parse_lor()?;
        if *self.peek() == Token::Question {
            self.advance();
            let t = self.parse_expr()?;
            self.expect(&Token::Colon)?;
            let f = self.parse_conditional()?;
            expr = Expr::Conditional(Box::new(expr), Box::new(t), Box::new(f));
        }
        Ok(expr)
    }

    pub(super) fn parse_lor(&mut self) -> Result<Expr, CError> { let mut l = self.parse_land()?; while *self.peek() == Token::LOr { self.advance(); let r = self.parse_land()?; l = Expr::LOr(Box::new(l), Box::new(r)); } Ok(l) }
    pub(super) fn parse_land(&mut self) -> Result<Expr, CError> { let mut l = self.parse_bitor()?; while *self.peek() == Token::LAnd { self.advance(); let r = self.parse_bitor()?; l = Expr::LAnd(Box::new(l), Box::new(r)); } Ok(l) }
    pub(super) fn parse_bitor(&mut self) -> Result<Expr, CError> { let mut l = self.parse_bitxor()?; while *self.peek() == Token::Or { self.advance(); let r = self.parse_bitxor()?; l = Expr::BitOr(Box::new(l), Box::new(r)); } Ok(l) }
    pub(super) fn parse_bitxor(&mut self) -> Result<Expr, CError> { let mut l = self.parse_bitand()?; while *self.peek() == Token::Xor { self.advance(); let r = self.parse_bitand()?; l = Expr::BitXor(Box::new(l), Box::new(r)); } Ok(l) }
    pub(super) fn parse_bitand(&mut self) -> Result<Expr, CError> { let mut l = self.parse_equality()?; while *self.peek() == Token::And { self.advance(); let r = self.parse_equality()?; l = Expr::BitAnd(Box::new(l), Box::new(r)); } Ok(l) }

    pub(super) fn parse_equality(&mut self) -> Result<Expr, CError> {
        let mut l = self.parse_relational()?;
        loop {
            match self.peek() {
                Token::EqEq => { self.advance(); let r = self.parse_relational()?; l = Expr::Eq(Box::new(l), Box::new(r)); }
                Token::Neq => { self.advance(); let r = self.parse_relational()?; l = Expr::Neq(Box::new(l), Box::new(r)); }
                _ => break,
            }
        }
        Ok(l)
    }

    pub(super) fn parse_relational(&mut self) -> Result<Expr, CError> {
        let mut l = self.parse_shift()?;
        loop {
            match self.peek() {
                Token::Lt => { self.advance(); let r = self.parse_shift()?; l = Expr::Lt(Box::new(l), Box::new(r)); }
                Token::Gt => { self.advance(); let r = self.parse_shift()?; l = Expr::Gt(Box::new(l), Box::new(r)); }
                Token::Le => { self.advance(); let r = self.parse_shift()?; l = Expr::Le(Box::new(l), Box::new(r)); }
                Token::Ge => { self.advance(); let r = self.parse_shift()?; l = Expr::Ge(Box::new(l), Box::new(r)); }
                _ => break,
            }
        }
        Ok(l)
    }

    pub(super) fn parse_shift(&mut self) -> Result<Expr, CError> {
        let mut l = self.parse_add()?;
        loop {
            match self.peek() {
                Token::Shl => { self.advance(); let r = self.parse_add()?; l = Expr::Shl(Box::new(l), Box::new(r)); }
                Token::Shr => { self.advance(); let r = self.parse_add()?; l = Expr::Shr(Box::new(l), Box::new(r)); }
                _ => break,
            }
        }
        Ok(l)
    }

    pub(super) fn parse_add(&mut self) -> Result<Expr, CError> {
        let mut l = self.parse_mul()?;
        loop {
            match self.peek() {
                Token::Plus => { self.advance(); let r = self.parse_mul()?; l = Expr::Add(Box::new(l), Box::new(r)); }
                Token::Minus => { self.advance(); let r = self.parse_mul()?; l = Expr::Sub(Box::new(l), Box::new(r)); }
                _ => break,
            }
        }
        Ok(l)
    }

    pub(super) fn parse_mul(&mut self) -> Result<Expr, CError> {
        let mut l = self.parse_unary()?;
        loop {
            match self.peek() {
                Token::Star => { self.advance(); let r = self.parse_unary()?; l = Expr::Mul(Box::new(l), Box::new(r)); }
                Token::Slash => { self.advance(); let r = self.parse_unary()?; l = Expr::Div(Box::new(l), Box::new(r)); }
                Token::Percent => { self.advance(); let r = self.parse_unary()?; l = Expr::Mod(Box::new(l), Box::new(r)); }
                _ => break,
            }
        }
        Ok(l)
    }

    pub(super) fn parse_unary(&mut self) -> Result<Expr, CError> {
        match self.peek() {
            Token::Minus => { self.advance(); let e = self.parse_unary()?; Ok(Expr::Neg(Box::new(e))) }
            Token::Not => { self.advance(); let e = self.parse_unary()?; Ok(Expr::Not(Box::new(e))) }
            Token::Tilde => { self.advance(); let e = self.parse_unary()?; Ok(Expr::BitNot(Box::new(e))) }
            // * `++` y `--` DELANTE, sobre cualquier lvalue y no solo un nombre.
            //
            // Solo se aceptaba `++nombre`. `--door->topcountdown` --que es como
            // `p_doors.c` cuenta los ticks de una puerta-- moria con "expected
            // CloseParen, got Arrow": el parser leia `door` como la variable
            // entera y la flecha sobraba.
            //
            // Un pre-incremento ES una asignacion: `--x` vale exactamente
            // `x = x - 1`, valor nuevo incluido. Asi que se reescribe con la
            // maquinaria de `+=`, que ya existia para las cinco formas de
            // lvalue. No hay nada nuevo en el codegen.
            Token::PlusPlus => {
                self.advance();
                if let Token::Ident(n) = self.peek().clone() {
                    if !matches!(self.tokens.get(self.pos + 1),
                        Some(Token::Arrow) | Some(Token::Dot) | Some(Token::OpenBracket))
                    {
                        self.advance();
                        return Ok(Expr::PreInc(n));
                    }
                }
                let e = self.parse_unary()?;
                asignacion_con_uno(e, Expr::Add).ok_or_else(|| {
                    CError::new(self.line(), "'++' necesita algo a lo que se pueda asignar")
                })
            }
            Token::MinusMinus => {
                self.advance();
                if let Token::Ident(n) = self.peek().clone() {
                    if !matches!(self.tokens.get(self.pos + 1),
                        Some(Token::Arrow) | Some(Token::Dot) | Some(Token::OpenBracket))
                    {
                        self.advance();
                        return Ok(Expr::PreDec(n));
                    }
                }
                let e = self.parse_unary()?;
                asignacion_con_uno(e, Expr::Sub).ok_or_else(|| {
                    CError::new(self.line(), "'--' necesita algo a lo que se pueda asignar")
                })
            }
            Token::And => { self.advance(); let expr = self.parse_unary()?; Ok(Expr::AddrOf(Box::new(expr))) }
            Token::Star => { self.advance(); let e = self.parse_unary()?; Ok(Expr::Deref(Box::new(e))) }
            // * `sizeof` de un TIPO y de una EXPRESION.
            //
            // Solo entendia el tipo, asi que `sizeof(p->campo)` moria con
            // "expected type, got Ident(p)" -- un mensaje que manda a buscar un
            // typedef que no falta. Y la forma con expresion no es un adorno:
            // es como se escribe `memset(&x, 0, sizeof(x))` sin repetir el
            // tipo, o sea la forma que NO se rompe cuando el tipo cambia.
            //
            // Se intenta primero el tipo y se vuelve atras si no cuela: los dos
            // empiezan igual y solo el intento distingue `sizeof(int)` de
            // `sizeof(x)`. La expresion no se EVALUA -- solo se le pregunta el
            // tipo, que es lo que dice el estandar.
            Token::Sizeof => {
                self.advance();
                self.expect(&Token::OpenParen)?;
                let guardado = self.pos;
                if let Ok(t) = self.parse_type_spec() {
                    if *self.peek() == Token::CloseParen {
                        self.advance();
                        return Ok(Expr::Int(self.tamano_de(&t) as i64));
                    }
                }
                self.pos = guardado;
                let e = self.parse_expr()?;
                self.expect(&Token::CloseParen)?;
                let t = self.resolve_expr_type(&e).ok_or_else(|| {
                    CError::new(
                        self.line(),
                        "sizeof: no se de que tipo es esa expresion".to_string(),
                    )
                })?;
                Ok(Expr::Int(self.tamano_de(&t) as i64))
            }
            Token::OpenParen => {
                let save = self.pos;
                self.advance();
                // Try to parse as cast: (type)expr
                let is_cast = self.peek_is_type_start();
                if is_cast {
                    if let Ok(typ) = self.parse_type_spec() {
                        if *self.peek() == Token::CloseParen {
                            self.advance();
                            let expr = self.parse_unary()?;
                            // cast REAL: codegen trunca/extiende al tamano del tipo
                            return Ok(Expr::Cast(typ, Box::new(expr)));
                        }
                    }
                }
                self.pos = save;
                self.parse_postfix()
            }
            _ => self.parse_postfix(),
        }
    }

    pub(super) fn parse_postfix(&mut self) -> Result<Expr, CError> {
        let mut expr = self.parse_primary()?;
        loop {
            match self.peek() {
                // ** `p->x++` SE IGNORABA EN SILENCIO.
                //
                // El brazo era `_ => {}`: si el operando no era un nombre
                // suelto, el `++` se consumia y **no se emitia nada**. O sea
                // que `s->count++` compilaba, corria, y no incrementaba --
                // ningun error, ningun aviso, y un contador que no se mueve.
                // Es la peor forma de fallar que hay en este proyecto.
                //
                // Ahora se reescribe, y si no se puede, se DICE.
                //
                // * Un post-incremento vale el valor VIEJO, asi que no basta
                // con `x += 1`: se compensa con la resta de fuera. Es exacto
                // para enteros -- y por eso se rechaza sobre un puntero, donde
                // `+1` avanza un elemento y `-1` restaria un byte.
                Token::PlusPlus => {
                    self.advance();
                    match expr {
                        Expr::Var(ref n) => expr = Expr::PostInc(n.clone()),
                        otro => expr = self.post_sobre_lvalue(otro, true)?,
                    }
                }
                Token::MinusMinus => {
                    self.advance();
                    match expr {
                        Expr::Var(ref n) => expr = Expr::PostDec(n.clone()),
                        otro => expr = self.post_sobre_lvalue(otro, false)?,
                    }
                }
                Token::OpenParen => {
                    // (*fp)(args) -- llamada a traves de un puntero CALCULADO.
                    // (fp(args) con fp variable ya lo maneja parse_primary.)
                    self.advance();
                    let mut args = Vec::new();
                    while *self.peek() != Token::CloseParen && *self.peek() != Token::Eof {
                        args.push(self.parse_assign()?);
                        if *self.peek() == Token::Comma { self.advance(); }
                    }
                    self.expect(&Token::CloseParen)?;
                    expr = Expr::CallPtr(Box::new(expr), args);
                }
                Token::OpenBracket => {
                    self.advance();
                    let index = self.parse_expr()?;
                    self.expect(&Token::CloseBracket)?;
                    match &expr {
                        Expr::Var(n) => {
                            let scale = self.element_size(n);
                            let n2 = n.clone();
                            expr = Expr::Subscript(n2, Box::new(index), scale);
                        }
                        // base compuesta (p->arr[i], (a+1)[i]): el elemento sale
                        // del tipo de la base. Antes se rechazaba en seco.
                        _ => {
                            let elem = self.resolve_expr_type(&expr)
                                .and_then(|t| match t {
                                    TypeSpec::Ptr(inner) | TypeSpec::Array(inner, _) => Some(*inner),
                                    _ => None,
                                })
                                .unwrap_or(TypeSpec::Long);
                            expr = Expr::IndexPtr(Box::new(expr), Box::new(index), elem);
                        }
                    }
                }
                Token::Dot => {
                    self.advance();
                    let field = match self.advance() {
                        Token::Ident(s) => s,
                        t => return Err(CError::new(self.line(),format!("expected field name, got {:?}", t))),
                    };
                    let offset = self.resolve_field_expr_offset(&expr, &field);
                    let ftyp = self.field_type_via_value(&expr, &field);
                    expr = Expr::Field(Box::new(expr), field, offset, ftyp);
                }
                Token::Arrow => {
                    self.advance();
                    let field = match self.advance() {
                        Token::Ident(s) => s,
                        t => return Err(CError::new(self.line(),format!("expected field name, got {:?}", t))),
                    };
                    let offset = self.resolve_arrow_expr_offset(&expr, &field);
                    let ftyp = self.field_type_via_pointer(&expr, &field);
                    expr = Expr::Arrow(Box::new(expr), field, offset, ftyp);
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    pub(super) fn parse_primary(&mut self) -> Result<Expr, CError> {
        let tok_line = self.line(); // linea del token que vamos a consumir
        let tok = self.advance();
        match tok {
            Token::IntLit(n) => Ok(Expr::Int(n)),
            Token::FloatLit(f) => Ok(Expr::FloatLit(f)),
            Token::StringLit(s) => Ok(Expr::StringLit(s)),
            Token::CharLit(c) => Ok(Expr::CharLit(c)),
            Token::Ident(name) => {
                if *self.peek() == Token::OpenParen {
                    self.advance();
                    let mut args = Vec::new();
                    while *self.peek() != Token::CloseParen && *self.peek() != Token::Eof {
                        // Use parse_assign (not parse_expr) to avoid the comma operator
                        // consuming argument separators -- C grammar requires
                        // argument_expression_list: assignment_expression (',' assignment_expression)*
                        args.push(self.parse_assign()?);
                        if *self.peek() == Token::Comma { self.advance(); }
                    }
                    self.expect(&Token::CloseParen)?;
                    // Check if this function name matches a known syscall definition
                    if let Some(def) = self.syscalls.get(&name).cloned() {
                        if args.len() != def.arg_count as usize {
                            return Err(CError::new(self.line(),format!(
                                "syscall {}() expects {} arguments, got {}",
                                def.name, def.arg_count, args.len()
                            )));
                        }
                        Ok(Expr::Syscall(def, args))
                    } else if let Some(stripped) = name.strip_prefix("__") {
                        // FUSION sem-asm<->C: __hlt(), __outb(p,v), __rdtsc()... =
                        // instruccion de la tabla como funcion. El namespace __
                        // es reservado a la implementacion -- aqui ES la
                        // implementacion. La aridad la valida el codegen contra
                        // la tabla (donde vive la verdad de cada intrinseco).
                        Ok(Expr::Intrinsic(stripped.to_string(), args))
                    } else {
                        Ok(Expr::Call(name, args))
                    }
                } else if let Some(&value) = self.enum_constants.get(&name) {
                    // Una constante de enum ES su valor, no una variable: no
                    // tiene direccion ni hueco en la pila.
                    Ok(Expr::Int(value))
                } else if let Some(real) = self.static_alias.get(&name) {
                    // * El UNICO sitio donde un identificador se vuelve
                    // variable, y por eso el unico que hace falta tocar para
                    // que las `static` locales funcionen. Si hubiera dos
                    // caminos, uno se quedaria sin traducir y el bug seria
                    // "a veces la static es la global de otro".
                    Ok(Expr::Var(real.clone()))
                } else {
                    Ok(Expr::Var(name))
                }
            }
            Token::OpenParen => {
                let expr = self.parse_expr()?;
                self.expect(&Token::CloseParen)?;
                Ok(expr)
            }
            t => Err(CError::new(tok_line, format!("unexpected token: {:?}", t))),
        }
    }
}
