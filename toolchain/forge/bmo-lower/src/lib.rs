//! `bmo-lower` — **L1: el descenso genérico al BMO ABI**.
//!
//! # Su lugar en el pipeline
//!
//! ```text
//! L2  ESPECIALIZADA (una por lenguaje, jamás se mezclan)
//!     C: printf(fmt,…)      COBOL: DISPLAY con PIC      C++: cout <<
//!     varargs, %d/%s        edición decimal ZZ9,99      operator<<
//!               │                     │                      │
//!               └─────────────────────┼──────────────────────┘
//!                                     ▼
//! L1  GENÉRICA  ← ESTA CRATE: "escribe estos bytes", nada más
//!                                     ▼
//! L0  SUPERFICIE CONGELADA (bmo_abi::syscalls::surface)
//!     INVOKE · CHANNEL_KICK · WAIT
//! ```
//!
//! # La regla que mantiene esto modular
//!
//! > **L1 solo contiene lo expresable en la superficie congelada por valor.
//! > Todo lo que tenga semántica de lenguaje —formato `%d`, edición PIC,
//! > `operator<<`— se queda en L2.**
//!
//! Esa frontera es lo que impide que esta crate degenere en un embudo de
//! mínimo común denominador. Aquí no se sabe qué lenguaje llamó, y ese es
//! exactamente el punto: cuando entre un cuarto frontend, no se toca nada.
//!
//! # Por qué existe
//!
//! Antes de esto, `lang/c/codegen.rs` y `lang/cobol/codegen.rs` emitían cada
//! uno su propia "impresión" contra números de syscall planos (`0x1F0`,
//! `NR_DEBUG_PRINT`) que el kernel **ya no despacha**, y encima pasaban un
//! puntero, cosa que la superficie congelada rechaza por diseño. Ninguno de
//! los dos imprimía nada en hardware. Un solo emisor correcto, compartido,
//! elimina la clase entera de bug.
//!
//! # Lo que emite
//!
//! Código x86-64 crudo, apendizado a un `Vec<u8>`. No conoce secciones,
//! relocations ni el escritor BEF: el frontend que lo llama ya tiene todo
//! eso. Por eso `console::write_const` no necesita meter la cadena en
//! `.rodata` — el texto viaja **dentro de las instrucciones**, como
//! inmediatos, así que no hay fixup que parchear ni puntero que cruzar.

pub mod console;
pub mod task;

mod x86;

#[cfg(test)]
mod emu;

/// Re-export de la superficie para que un frontend que enlaza `bmo-lower`
/// no tenga que declarar además `bmo-abi` solo para nombrar una operación.
pub use bmo_abi::syscalls::surface;
