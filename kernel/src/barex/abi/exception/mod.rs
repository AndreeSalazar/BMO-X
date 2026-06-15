//! `exception` — modelo unificado de unwinding del BMO ABI.
//!
//! Reemplaza:
//!   - C++ Itanium EH (DWARF .eh_frame, personality functions, `__cxa_throw`)
//!   - Win64 SEH (RtlAddFunctionTable, `_C_specific_handler`)
//!   - .NET / Java exceptions (managed stack walks)
//!   - Swift `throws`, Kotlin exceptions, Python `raise`
//!
//! ## Filosofía BMO
//!
//! - **No hay tablas DWARF.** El unwind se describe en `SectionKind::Unwind`
//!   con un formato compacto BMO (un solo CIE/FDE simplificado).
//! - **Personality function única:** `bmo_personality` recorre la tabla.
//! - **Tipos de payload:** `BmoStatus` para errores recuperables, `BmoPanic`
//!   para fatales. No hay polimorfismo de excepciones a la C++.
//! - **Cualquier lenguaje** que produzca BEF puede catch/throw entre módulos
//!   sin saber del lenguaje origen — los payloads viajan como `BmoStatus`.

#![allow(dead_code)]

pub mod unwind;
pub mod panic;
pub mod resume;
pub mod table;

pub use table::UnwindTable;
