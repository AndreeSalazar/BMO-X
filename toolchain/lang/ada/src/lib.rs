//! [isa] NINGUNA -- este crate es el FRONTEND de Ada: lexer, analisis y arbol.
//! No nombra una maquina y no depende de nada. Lo que emite x86-64 vive en
//! `emisor-x86_64/` (crate `bmo-ada-x86-64`), igual que INTI. Partido el
//! 2026-09-18: en BMO-X todo es x86-64 MENOS los frontends de los
//! compiladores, que son lo unico agnostico -- y lo son por el arbol de
//! directorios, no por una convencion. Ver `toolchain/tools/isa`.
//!
//! **BMO Ada** -- Ada compilado a nativo, sin runtime y sin biblioteca.
//!
//! ## Por que Ada, y por que salio barato
//!
//! El objetivo de BMO es banca, y para banca hacen falta dos cosas: decimal
//! exacto y un compilador del que uno se pueda fiar. Ada trae las dos de
//! fabrica -- y trae una tercera que casi nadie menciona:
//!
//! **El Annex F de Ada (Information Systems) copio las reglas de COBOL.** Los
//! tipos decimales (`delta 0.01 digits 12`) y la edicion con `PICTURE` de
//! `Ada.Text_IO.Editing` estan definidos sobre ANSI X3.23-1985, que es COBOL.
//! Por eso este frontend nace con el decimal ya resuelto: es la misma
//! aritmetica de escala entera que ya estaba escrita y probada.
//!
//! ## Que es y que NO es
//!
//! Es un frontend **completo y propio**: lexer, analisis y emisor de bytes,
//! sin pasar por ningun cerebro compartido. No depende de `lang/cobol` -- la
//! regla del proyecto es que cada lenguaje mantiene su esencia de principio a
//! fin, y lo unico compartido son contratos (el contenedor BEF, los 3
//! syscalls) y librerias **opcionales** (`bmo-lower`).
//!
//! **No es GNAT.** Lo que compila hoy esta en la matriz de conformidad de
//! abajo, y todo lo demas se rechaza **con su motivo**: `package`, genericos,
//! tareas, `for`, `elsif`, y cualquier `with` que no sea `Ada.Text_IO`.
//! Prometer Ada entero seria mentir en la primera linea.
//!
//! ## El limite que importa: un fichero, una unidad
//!
//! Ada de verdad son especificacion y cuerpo con **orden de elaboracion**
//! (RM 10.2.1). Eso es semantica del lenguaje y no se puede fingir, asi que
//! aqui se acota a un `procedure` suelto --forma que el estandar permite-- y se
//! rechaza lo que pida el modelo de unidades. Es la misma decision que en los
//! otros frontends: alcance acotado y dicho en voz alta.

pub mod ast;
pub mod lexer;
pub mod parser;

pub use ast::AdaError;

/// Analiza el fuente.
pub fn analizar(fuente: &str) -> Result<ast::Programa, AdaError> {
    parser::Parser::nuevo(fuente).programa()
}
