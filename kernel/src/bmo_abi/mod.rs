//! `bmo_abi` — BMO ABI: la convención y el "stdlib mínimo" nativo de FastOS.
//!
//! **Reemplaza al C ABI** (cdecl/stdcall/Win64/SysV AMD64) y a su stdlib
//! (`<stdint.h>`, `<stddef.h>`, `<string.h>`, `<errno.h>`, `<time.h>`, etc).
//!
//! Spec maestra: `combo_Window_Extractor/MAPA de Window/02_BEF_Format/BMO_ABI_Spec.md`.
//! Mapa visual: `README.md` en esta carpeta.
//!
//! ## Organización
//!
//! 5 categorías semánticas que separan "lo que todo el mundo usa" de
//! "cómo se compone" y "cómo se habla con otros lenguajes":
//!
//! ```text
//!   runtime/         BmoRuntime agregador (handle único)
//!   fundamentals/    Tipos que TODO el código importa
//!                    (primitives, status, handle, option, result, memory)
//!   values/          Tipos valor con semántica propia
//!                    (string, time, reflect)
//!   machinery/       Cómo se COMPONE el código
//!                    (calling, sync, type_system, vtable, closure,
//!                     exception, async_io)
//!   interop/         Cómo se HABLA con otros lenguajes y otros ABIs
//!                    (lang_bridge, marshal, compat)
//! ```

#![allow(dead_code)]

pub mod fundamentals;
pub mod values;
pub mod machinery;
pub mod interop;
pub mod runtime;

// ─── Re-exports planos para uso ergonómico ────────────────────────────
//
// Apps Rust pueden hacer `use crate::bmo_abi::*;` y obtener los tipos
// esenciales sin navegar sub-módulos.

pub use fundamentals::primitives;
pub use fundamentals::status;
pub use fundamentals::handle;

pub use values::string;
pub use values::time;
pub use values::reflect;

pub use machinery::calling;
pub use machinery::sync;
pub use machinery::type_system;
pub use machinery::vtable;
pub use machinery::exception;

pub use interop::lang_bridge;


/// Versión del BMO ABI implementada por este kernel.
pub const BMO_ABI_VERSION: (u8, u8) = (1, 0);

/// Magic constant en headers BEF para identificar BMO ABI.
pub const BMO_ABI_MAGIC: u32 = u32::from_le_bytes(*b"BMO1");
