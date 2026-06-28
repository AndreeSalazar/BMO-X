//! `bmo_abi` — BMO ABI: la convención y el "stdlib mínimo" nativo de FastOS.
//!
//! **Reemplaza al C ABI** (cdecl/stdcall/Win64/SysV AMD64) y a su stdlib
//! (`<stdint.h>`, `<stddef.h>`, `<string.h>`, `<errno.h>`, `<time.h>`, etc).
//!
//! # Estructura (v1.8.8)
//!
//! ```text
//! bmo_abi/
//! ├── fundamentals/   — Tipos que TODO código usa
//! │   ├── primitives/ — int, bool, float
//! │   ├── status/      — BmoStatus, ErrorCode
//! │   ├── handle/      — Handle table
//! │   └── sync/        — BmoAtomicU64, MemOrder
//! │
//! ├── values/         — Tipos valor
//! │   └── time/        — Instant, Duration
//! │
//! ├── windowing/      — Contrato de ventanas
//! ├── fs/             — File/Dir handles, OpenFlags, Stat
//! ├── surface/        — Formatos de pixel, surfaces CPU/GPU
//! ├── error_code/     — Códigos extendidos (21 codes)
//! ├── bef/            — Formato BEF (header, secciones)
//! ├── syscalls/       — Tabla de syscall numbers 0x100..0x1FF
//! └── profile/        — BmoLanguageProfile + ALL_PROFILES
//! ```
//!
//! Ver `SPEC.md` para la especificación completa.
#![no_std]
#![allow(dead_code)]
extern crate alloc;
pub mod fundamentals;
pub mod values;
pub mod windowing;
pub mod fs;
pub mod surface;
pub mod error_code;
pub mod bef;
pub mod syscalls;
pub mod profile;

// ─── Re-exports planos para uso ergonómico ─────────────────────────

pub use fundamentals::primitives;
pub use fundamentals::status;
pub use fundamentals::handle;
pub use fundamentals::sync as sync_re;

pub use values::time as values_time;

// ─── Versión + magic ──────────────────────────────────────────────

/// Versión del BMO ABI implementada por este kernel.
pub const BMO_ABI_VERSION: (u8, u8) = (1, 0);

/// Magic constant en headers BEF para identificar BMO ABI.
pub const BMO_ABI_MAGIC: u32 = u32::from_le_bytes(*b"BMO1");

pub use crate as bmo_abi;
