//! `bmo_abi` — BMO ABI: la convención y el "stdlib mínimo" nativo de FastOS.
//!
//! **Reemplaza al C ABI** (cdecl/stdcall/Win64/SysV AMD64) y a su stdlib
//! (`<stdint.h>`, `<stddef.h>`, `<string.h>`, `<errno.h>`, `<time.h>`, etc).
//!
//! # v2.0
//!
//! BMO ABI es ahora un ABI **modular**. Cada carpeta es un módulo
//! autocontenido. Apps pueden importar solo lo que necesitan:
//!
//! ```ignore
//! use crate::bmo_abi::fundamentals::primitives::*;
//! use crate::bmo_abi::fundamentals::status::*;
//! ```
//!
//! # Estructura
//!
//! ```text
//! bmo_abi/
//! ├── fundamentals/   — Tipos que TODO código usa
//! │   ├── primitives/ — int, bool, float
//! │   ├── status/      — BmoStatus, ErrorCode
//! │   ├── handle/      — Handle table
//! │   ├── option/      — Option<T>
//! │   ├── result/      — Result<T, E>
//! │   ├── memory/      — slice, range, align
//! │   ├── sync/        — atomics, SpinLock
//! │   ├── error/       — BmoError unificado
//! │   ├── convert/     — BmoError↔BmoStatus↔ErrorCode
//! │   ├── fmt/         — BmoFormatter, write!
//! │   └── io/          — BmoFileHandle, BmoPipe, Read/Write/Seek
//! ├── values/         — Tipos valor
//! │   ├── string/      — BmoStr, ascii
//! │   ├── time/        — Instant, Duration
//! │   ├── reflect/     — TypeDescriptor, Mirror, ReflectQuery
//! │   ├── net/         — IPv4/IPv6, SocketAddr, Protocol
//! │   ├── math/        — sqrt, sin, cos, pow (f64)
//! │   └── hash/        — FNV-1a, CRC32
//! ├── befcore/        — Protocolo BEFCore: mensajes app ↔ BMO CORE
//! │   └── mod.rs       — BefcoreMessage (CreateWindow/DrawText/...)
//! │                      + BefcoreEvent (Paint/KeyDown/MouseMove/...)
//! │                      + NR_BEFCORE_SEND/RECV/POLL (0x190..0x192)
//! ├── syscalls/       — Tabla de syscall numbers 0x100..0x1FF
//! └── runtime/        — BmoRuntime agregador
//!     ├── types/       — TypeRegistry (256 slots)
//!     ├── vtable/      — VTableStore (64 slots)
//!     └── lang_bridge/ — LangBridge (8 languages)
//! ```

#![allow(dead_code)]

pub mod fundamentals;
pub mod values;
pub mod befcore;
pub mod syscalls;
pub mod runtime;

// ─── Re-exports planos para uso ergonómico ────────────────────────────

pub use fundamentals::primitives;
pub use fundamentals::status;
pub use fundamentals::handle;
pub use fundamentals::sync as sync_re;
pub use fundamentals::error;
pub use fundamentals::convert;
pub use fundamentals::fmt;
pub use fundamentals::io as abi_io;

pub use values::string;
pub use values::time;
pub use values::reflect;
pub use values::net;
pub use values::math;
pub use values::hash;

// `befcore` y `syscalls` se usan directamente como `crate::bmo_abi::befcore::*`
// y `crate::bmo_abi::syscalls::*` (ya son `pub mod`).

/// Versión del BMO ABI implementada por este kernel.
pub const BMO_ABI_VERSION: (u8, u8) = (1, 0);

/// Magic constant en headers BEF para identificar BMO ABI.
pub const BMO_ABI_MAGIC: u32 = u32::from_le_bytes(*b"BMO1");
