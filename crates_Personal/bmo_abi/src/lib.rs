//! `bmo_abi` — BMO ABI: la convención y el "stdlib mínimo" nativo de FastOS.
//!
//! **Reemplaza al C ABI** (cdecl/stdcall/Win64/SysV AMD64) y a su stdlib
//! (`<stdint.h>`, `<stddef.h>`, `<string.h>`, `<errno.h>`, `<time.h>`, etc).
//!
//! # Estructura
//!
//! ```text
//! bmo_abi/
//! ├── fundamentals/   — Tipos que TODO código usa
//! │   ├── primitives/ — int, bool, float (bx_u8..u64, bx_i*, bx_f*)
//! │   ├── status/      — BmoStatus 16-byte, StatusFlags
//! │   ├── handle/      — BmoHandle 64-bit con tag+generation
//! │   ├── option/      — BmoOption<T> FFI-safe
//! │   ├── result/      — BmoResult<T, E> FFI-safe
//! │   ├── error/       — BmoError 16-byte unificado
//! │   ├── convert/     — BmoStatus ↔ BmoError ↔ ErrorCode
//! │   ├── string/      — BmoStr (borrowed), BmoString (owned)
//! │   ├── memory/      — BmoSlice, BmoRange, BmoAligned
//! │   ├── io/          — BmoRead, BmoWrite, BmoSeek, BmoPipe
//! │   ├── fmt/         — BmoFormatter stack-allocated
//! │   └── sync/        — BmoAtomicU32/U64/Bool, MemOrder, BmoSpinLock
//! │
//! ├── values/         — Tipos valor con semántica propia
//! │   ├── time/        — BmoInstant, BmoDuration
//! │   ├── math/        — sqrt, sin, cos, pow
//! │   ├── hash/        — FNV-1a, CRC32c, CRC32
//! │   ├── net/         — BmoIpv4Addr, BmoIpv6Addr, BmoSocketAddr
//! │   └── reflect/     — ReflectQuery
//! │
//! ├── runtime/        — Agregador de runtime: TypeRegistry, VTableStore, LangBridge
//! ├── windowing/      — Contrato de ventanas
//! ├── fs/             — File/Dir handles, OpenFlags, Stat
//! ├── surface/        — Formatos de pixel, surfaces CPU/GPU
//! ├── error_code/     — BmoErrorCode enum, BmoErrorSeverity, raw constants
//! ├── bef/            — Formato BEF (header, secciones, relocs)
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
pub mod runtime;
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
