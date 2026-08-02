//! **BMO C++** — C++ acotado que baja sobre el AST de BMO C.
//!
//! ```text
//! fuente .cpp → [lexer → parser + tabla de simbolos] → descenso
//!                                                        │
//!                     bmo_c_front::ast::Program  ◄────────┘   (LA FRONTERA)
//!                                 │
//!                     bmo_c_front::codegen  →  bytes del BEF
//! ```
//!
//! C++ hereda el **descenso** de BMO C, no su frontend. La frontera es un tipo
//! de datos —un formato, no un cerebro— y la flecha apunta en un solo sentido:
//! `lang/c` no sabe que este crate existe. Las cuatro reglas de *"no se
//! combinan"* están en `HERENCIA.md`.
//!
//! ═══ Dónde está esto: PASOS 0 y 1 HECHOS ═══
//!
//! **Paso 0 — que emita un byte.** Antes, este frontend no producía bytes para
//! NINGUNA entrada: emitía un `IrModule` de 12,12 MB —construido en la pila, lo
//! que desbordaba hasta con un fichero vacío— que además **no tenía un solo
//! consumidor en el repo**. No había emisor porque no se había enchufado
//! ninguno. Hoy sale un BEF que corre, y el BEF de C++ es **byte a byte
//! idéntico** al de BMO C para la misma fuente.
//!
//! **Paso 1 — lexer y parser de verdad.** El anterior miraba la fuente carácter
//! a carácter: leía el identificador `x` como un número hexadecimal, no tenía
//! precedencia de operadores, y su `parse_body` **se tragaba en silencio** todo
//! lo que no reconocía. Ahora hay tokens con línea real, la escalera completa de
//! precedencia, ámbitos anidados, y **ninguna rama que descarte tokens**.
//!
//! Lo que aún no baja se **rechaza diciendo en qué paso llega**, nunca en
//! silencio. El orden completo está en `BRECHA.md`.
//!
//! Falta de este paso el **preprocesador** (`#include`, `#define`): se rechaza
//! con motivo, y es la otra mitad del paso 1.

pub mod ast;
pub mod descenso;
pub mod lexer;
pub mod parser;

use ast::*;
use bmo_abi::profile::BmoLanguageProfile;

pub fn profile() -> BmoLanguageProfile {
    BmoLanguageProfile {
        name: "C++",
        frontend: bmo_abi::profile::FrontendKind::Cpp,
        backend: bmo_abi::profile::BackendKind::AotX86_64,
        runtime: bmo_abi::profile::RuntimeKind::CppMin,
        uses_bmo_abi: true,
        ring0_capable: true,
        standard_version: "cpp17",
    }
}

pub fn parse(source: &str) -> Result<Program, CppError> {
    parser::parse(source)
}

/// **La única salida que cuenta**: fuente de C++ → bytes del BEF.
///
/// Pasa por el AST de BMO C (`descenso`) y por SU codegen. Aquí no hay ni un
/// byte de x86-64 escrito por C++, y ése es el objetivo: el backend que se
/// hereda tiene 223 tests y está verificado en el Ryzen.
pub fn compile_source_to_bef(source: &str) -> Result<Vec<u8>, CppError> {
    let programa = parse(source)?;
    let en_c = descenso::descender(&programa)?;
    bmo_c_front::codegen::compile_to_bef_bytes(&en_c)
        .map_err(|e| CppError::new(e.line, e.message))
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

#[cfg(test)]
mod tests;
