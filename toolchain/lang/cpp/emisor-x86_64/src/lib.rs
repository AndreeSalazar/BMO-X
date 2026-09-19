//! **BMO C++ para x86-64** -- del arbol de C++ a un `.bex` o a un `.bo`.
//!
//! [isa] x86-64. El frontend (`bmo-cpp-front`, la carpeta de arriba) baja C++
//! al arbol de C; esto le pasa ese arbol al codegen de C para x86-64
//! (`bmo-c-x86-64`) y pone el perfil. Partido el 2026-09-18
//! (`toolchain/tools/isa`). El frontend se re-exporta entero.

pub use bmo_cpp_front::*;

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

/// **La unica salida que cuenta**: fuente de C++ -> bytes del BEF.
///
/// Pasa por el AST de BMO C (`descenso`) y por SU codegen. Aqui no hay ni un
/// byte de x86-64 escrito por C++, y ese es el objetivo: el backend que se
/// hereda tiene 223 tests y esta verificado en el Ryzen.
pub fn compile_source_to_bef(source: &str) -> Result<Vec<u8>, CppError> {
    let programa = parse(source)?;
    let en_c = descenso::descender(&programa)?;
    bmo_c_x86_64::codegen::compile_to_bef_bytes(&en_c)
        .map_err(|e| CppError::new(e.line, e.message))
}

/// **Lo mismo, a un OBJETO (`.bo`)** para que `bmo-enlazar` lo junte con otros.
///
/// Es la misma frontera de arriba: quien decide si las referencias salen
/// cerradas o abiertas es el codegen de BMO C, no este frontend. Por eso esto
/// son cuatro lineas y no un compilador -- la compilacion separada de C++ la
/// pago E2 sin saberlo.
pub fn compile_source_to_object(source: &str) -> Result<Vec<u8>, CppError> {
    let programa = parse(source)?;
    let en_c = descenso::descender_unidad(&programa, true)?;
    bmo_c_x86_64::codegen::compile_to_object(&en_c)
        .map_err(|e| CppError::new(e.line, e.message))
}

#[cfg(test)]
mod tests;
