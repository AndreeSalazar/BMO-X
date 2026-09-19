//! **`new` y `delete`** -- paso 3 de `BRECHA.md` (2026-09-18).
//!
//! `new P(args)` es literalmente `malloc` mas el constructor, y `delete p` el
//! destructor mas `free` -- lo que Cfront escribia. El parser solo resuelve
//! QUE constructor (la misma sobrecarga que `P p(args);`) y apunta que la
//! unidad necesita el monton; lo emite el descenso (`descenso.rs`, `nuevo`).
//!
//! === Lo que NO entra, y lo dice ===
//!
//! - `new int` / `new P[n]` / `delete[]`: un array de objetos pide la cuenta de
//!   elementos delante (BRECHA.md, `new[] / delete[]`). Hasta el 18-09
//!   `delete[] p` se tragaba los corchetes y hacia un `delete p` -- en
//!   silencio y mal.
//! - el destructor virtual: sin el, `delete` sobre un puntero a la base llama
//!   al destructor de la BASE, que es lo que dice el estandar cuando no es
//!   virtual. Se rechaza `virtual ~P()` en vez de aceptarlo a medias.
//!
//! Vive aqui y no en `parser.rs` por L6a: aquel solo puede encoger.

use super::*;

impl Parser {
    /// `new P(a, b)`, `new P()` o `new P`.
    pub(super) fn expr_new(&mut self) -> Result<Expr, CppError> {
        self.exige(&Token::New)?;
        let cls = match self.avanzar() {
            Token::Ident(n) if self.clases.contains_key(&n) => n,
            Token::Ident(n) => return Err(self.err(format!("`new {n}`: `{n}` no es una clase conocida"))),
            _ => return Err(self.pendiente("`new` de un tipo que no es una clase (`new int`)", 3)),
        };
        if *self.peek() == Token::OpenBracket {
            return Err(self.pendiente("`new[]` (un array de objetos)", 3));
        }
        let mut args = Vec::new();
        if self.come(&Token::OpenParen) {
            if *self.peek() != Token::CloseParen {
                loop {
                    args.push(self.asignacion()?);
                    if !self.come(&Token::Comma) { break; }
                }
            }
            self.exige(&Token::CloseParen)?;
        }
        let ctor = self.resolver_ctor(&cls, &args)?;
        let clave = (cls.clone(), ctor.clone());
        if !self.nuevos.contains(&clave) {
            self.nuevos.push(clave);
        }
        self.monton = true;
        Ok(Expr::New(cls, ctor, args))
    }

    /// `delete p;` -- `p` es una variable puntero.
    pub(super) fn borrar(&mut self) -> Result<Stmt, CppError> {
        self.exige(&Token::Delete)?;
        if *self.peek() == Token::OpenBracket {
            return Err(self.pendiente("`delete[]` (un array de objetos)", 3));
        }
        let n = match self.avanzar() {
            Token::Ident(n) => n,
            otro => return Err(self.err(format!("`delete` de {otro:?}: se borra una variable puntero"))),
        };
        self.exige(&Token::Semicolon)?;
        let clase = match self.ambitos.tipo(&n) {
            Some(TypeSpec::Ptr(t)) => match &**t {
                TypeSpec::ClassRef(c) => Some(c.clone()),
                _ => None,
            },
            Some(_) => return Err(self.err(format!("`delete {n}`: `{n}` no es un puntero"))),
            None => return Err(self.err(format!("`delete {n}`: `{n}` no esta declarada"))),
        };
        self.monton = true;
        Ok(Stmt::Delete(n, clase))
    }
}
