//! `bmo_abi` — BMO ABI: la convención y el "stdlib mínimo" nativo de FastOS.
//!
//! **Reemplaza al C ABI** (cdecl/stdcall/Win64/SysV AMD64) y a su stdlib
//! (`<stdint.h>`, `<stddef.h>`, `<string.h>`, `<errno.h>`, `<time.h>`, etc).
//!
//! # v1.7.9 — Rediseño
//!
//! BMO ABI es ahora un ABI **modular**. Cada carpeta es un módulo
//! autocontenido. Apps pueden importar solo lo que necesitan:
//!
//! ```ignore
//! use crate::bmo_core::bmo_abi::fundamentals::primitives::*;
//! use crate::bmo_core::bmo_abi::fundamentals::status::*;
//! ```
//!
//! # Estructura
//!
//! ```text
//! bmo_abi/
//! ├── fundamentals/   — Tipos que TODO código usa
//! │   ├── primitives/ — int, bool, float
//! │   ├── status/      — BmoStatus, BmoError
//! │   ├── handle/      — Handle table
//! │   ├── option/      — Option<T>
//! │   ├── result/      — Result<T, E>
//! │   └── memory/      — slice, range, align
//! ├── values/         — Tipos valor
//! │   ├── string/      — bx_str, ascii
//! │   ├── time/        — Instant, Duration
//! │   └── reflect/     — type info (mínimo)
//! └── runtime/        — BmoRuntime agregador (handle único)
//! ```

#![allow(dead_code)]

pub mod fundamentals;
pub mod values;
pub mod runtime;

// ─── Re-exports planos para uso ergonómico ────────────────────────────

pub use fundamentals::primitives;
pub use fundamentals::status;
pub use fundamentals::handle;
pub use fundamentals::sync as sync_re;

pub use values::string;
pub use values::time;
pub use values::reflect;

/// Versión del BMO ABI implementada por este kernel.
pub const BMO_ABI_VERSION: (u8, u8) = (1, 0);

/// Magic constant en headers BEF para identificar BMO ABI.
pub const BMO_ABI_MAGIC: u32 = u32::from_le_bytes(*b"BMO1");
