//! [isa] NINGUNA -- el FRONTEND de C++: lexer, parser, ast, decorado y el
//! descenso al arbol de C. No nombra una maquina: baja al arbol del FRONTEND de
//! C (`bmo-c-front`), y lo que emite x86-64 vive en `emisor-x86_64/` (crate
//! `bmo-cpp-x86-64`). Partido el 2026-09-18 (`toolchain/tools/isa`).
//!
//! **BMO C++** -- C++ acotado que baja sobre el AST de BMO C.
//!
//! ```text
//! fuente .cpp -> [lexer -> parser + tabla de simbolos] -> descenso
//!                                                        |
//!                      bmo_c_front::ast::Program  <--------+   (LA FRONTERA)
//!                                 |
//!                     bmo_c_x86_64::codegen  ->  bytes del BEF   (en `emisor-x86_64/`)
//! ```
//!
//! C++ hereda el **descenso** de BMO C, no su frontend. La frontera es un tipo
//! de datos --un formato, no un cerebro-- y la flecha apunta en un solo sentido:
//! `lang/c` no sabe que este crate existe. Las cuatro reglas de *"no se
//! combinan"* estan en `HERENCIA.md`.
//!
//! === Donde esta esto: PASOS 0 y 1 HECHOS ===
//!
//! **Paso 0 -- que emita un byte.** Antes, este frontend no producia bytes para
//! NINGUNA entrada: emitia un `IrModule` de 12,12 MB --construido en la pila, lo
//! que desbordaba hasta con un fichero vacio-- que ademas **no tenia un solo
//! consumidor en el repo**. No habia emisor porque no se habia enchufado
//! ninguno. Hoy sale un BEF que corre, y el BEF de C++ es **byte a byte
//! identico** al de BMO C para la misma fuente.
//!
//! **Paso 1 -- lexer y parser de verdad.** El anterior miraba la fuente caracter
//! a caracter: leia el identificador `x` como un numero hexadecimal, no tenia
//! precedencia de operadores, y su `parse_body` **se tragaba en silencio** todo
//! lo que no reconocia. Ahora hay tokens con linea real, la escalera completa de
//! precedencia, ambitos anidados, y **ninguna rama que descarte tokens**.
//!
//! Lo que aun no baja se **rechaza diciendo en que paso llega**, nunca en
//! silencio. El orden completo esta en `BRECHA.md`.
//!
//! Falta de este paso el **preprocesador** (`#include`, `#define`): se rechaza
//! con motivo, y es la otra mitad del paso 1.

pub mod ast;
pub mod descenso;
pub mod lexer;
pub mod mangling;
pub mod parser;

use ast::*;

pub fn parse(source: &str) -> Result<Program, CppError> {
    parser::parse(source)
}

#[derive(Debug, Clone)]
pub struct CppError {
    pub line: usize,
    pub message: String,
}

impl CppError {
    pub fn new(line: usize, msg: impl Into<String>) -> Self {
        Self { line, message: msg.into() }
    }
}
