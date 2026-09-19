//! **El enlace de un nombre: que simbolo sale, y `extern "C"`** -- E10 de
//! `docs/plan/PLAN_EL_ENLAZADOR.md` (2026-09-18).
//!
//! Llamar a C desde C++ ya funcionaba: un nombre que no esta en la tabla de
//! C++ se emite tal cual (`CPP_ABI.md`, seccion 4). Lo que no funcionaba era
//! lo contrario: una funcion de C++ sale DECORADA (`cobrar#i.i`), y un `.bo`
//! de C que pide `cobrar` no la encuentra. `extern "C"` es la palabra que el
//! lenguaje tiene para eso: el simbolo es el nombre, sin decorar.
//!
//! Y con el nombre sin decorar, la sobrecarga deja de existir -- C no la
//! tiene. Dos `extern "C"` con el mismo nombre y otros parametros serian UN
//! simbolo con dos significados; aqui es un error, no el primero que gane.
//!
//! `declarar_funcion` vino con esto desde `parser.rs`: decidir que simbolo
//! lleva una funcion ES el enlace. Y aquel fichero solo puede encoger (L6a).

use super::*;

impl Parser {
    /// `extern "C" <declaracion>` o `extern "C" { ... }`. Tambien `"C++"`, que
    /// es el enlace de siempre.
    pub(super) fn extern_c(&mut self, p: &mut Program) -> Result<(), CppError> {
        self.exige(&Token::Extern)?;
        let de_c = match self.avanzar() {
            Token::StringLit(s) if s == "C" => true,
            Token::StringLit(s) if s == "C++" => false,
            Token::StringLit(s) => return Err(self.err(format!(
                "`extern \"{s}\"`: los enlaces que existen son \"C\" y \"C++\""))),
            _ => return Err(self.pendiente("`extern` sin \"C\" (una global que vive en otra unidad)", 4)),
        };
        let antes = self.enlace_c;
        self.enlace_c = de_c;
        let r = if self.come(&Token::OpenBrace) {
            loop {
                if self.come(&Token::CloseBrace) { break Ok(()); }
                if *self.peek() == Token::Eof {
                    break Err(self.err("el bloque `extern \"C\" {` no se cierra"));
                }
                if let Err(e) = self.declaracion_de_fichero(p) { break Err(e); }
            }
        } else {
            self.declaracion_de_fichero(p)
        };
        self.enlace_c = antes;
        r
    }

    /// Registra una funcion y devuelve su simbolo.
    ///
    /// Rechaza redeclarar la MISMA firma con otro retorno, que es lo que C++
    /// prohibe: no se puede sobrecargar por retorno, asi que dos `f(int)` con
    /// retornos distintos son la misma funcion declarada dos veces mal.
    pub(super) fn declarar_funcion(&mut self, name: &str, params: &[Param], ret: &TypeSpec)
        -> Result<String, CppError>
    {
        let tipos: Vec<TypeSpec> = params.iter().map(|p| p.typ.clone()).collect();
        let simbolo = if (name == "main" && self.espacios.is_empty()) || self.enlace_c {
            name.to_string()
        } else {
            crate::mangling::funcion(&self.espacios, name, &tipos)
        };
        let de_c = self.enlace_c;
        let lista = self.funciones.entry(name.to_string()).or_default();
        if let Some(ya) = lista.iter().find(|f| f.simbolo == simbolo) {
            if de_c && ya.params != tipos {
                return Err(self.err(format!(
                    "`extern \"C\" {name}` ya esta declarada con otros parametros: un nombre de \
                     C no admite sobrecarga, es UN simbolo")));
            }
            if &ya.ret != ret {
                return Err(self.err(format!(
                    "`{name}` ya esta declarada con los mismos parametros y otro retorno: \
                     C++ no permite sobrecargar por el tipo de retorno")));
            }
        } else {
            lista.push(Firma { params: tipos, ret: ret.clone(), simbolo: simbolo.clone() });
        }
        self.retornos.insert(simbolo.clone(), ret.clone());
        Ok(simbolo)
    }
}
