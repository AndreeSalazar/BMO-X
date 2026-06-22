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
//! │   ├── option/      — Option<T>
//! │   ├── result/      — Result<T, E>
//! │   ├── memory/      — slice, range, align
//! │   ├── sync/        — atomics, SpinLock
//! │   ├── error/       — BmoError unificado
//! │   ├── convert/     — BmoError↔BmoStatus↔ErrorCode
//! │   ├── fmt/         — BmoFormatter, write!
//! │   └── io/          — BmoFileHandle, BmoPipe, Read/Write/Seek
//! │
//! ├── values/         — Tipos valor
//! │   ├── string/      — BmoStr, ascii
//! │   ├── time/        — Instant, Duration (BmoClock re-exporta)
//! │   ├── reflect/     — TypeDescriptor, Mirror, ReflectQuery
//! │   ├── net/         — IPv4/IPv6, SocketAddr, Protocol
//! │   ├── math/        — sqrt, sin, cos, pow (f64)
//! │   └── hash/        — FNV-1a, CRC32
//! │
//! ├── windowing/      — Contrato de ventanas
//! ├── drawing/        — Color, Rect, Point, Font
//! ├── input/          — Key/Mouse/Gamepad state + event
//! ├── fs/             — File/Dir handles, OpenFlags, Stat
//! ├── clock/          — Re-export de Instant/Duration + helpers
//! ├── ipc/            — Ports, Messages, Rights
//! ├── surface/        — Formatos de pixel, surfaces CPU/GPU
//! ├── process/        — Procesos, threads, info
//! ├── memory/         — Allocator interface
//! ├── error_code/     — Códigos extendidos (21 codes)
//! ├── gpu/            — Contratos RDNA4 (skeleton)
//! ├── bef/            — Formato BEF (header, secciones)
//! ├── entry/          — Punto de entrada, stack, args
//! ├── befcore/        — Protocolo BEFCore (app ↔ BMO CORE)
//! ├── syscalls/       — Tabla de syscall numbers 0x100..0x1FF
//! ├── runtime/        — TypeRegistry, VTableStore, LangBridge
//! └── profile/        — BmoLanguageProfile + ALL_PROFILES
//! ```
//!
//! Ver `SPEC.md` para la especificación completa.

#![allow(dead_code)]

pub mod fundamentals;
pub mod values;
pub mod windowing;
pub mod drawing;
pub mod input;
pub mod fs;
pub mod clock;
pub mod ipc;
pub mod surface;
pub mod process;
pub mod memory;
pub mod error_code;
pub mod gpu;
pub mod bef;
pub mod entry;
pub mod befcore;
pub mod syscalls;
pub mod runtime;
pub mod profile;

// ─── Re-exports planos para uso ergonómico ─────────────────────────

pub use fundamentals::primitives;
pub use fundamentals::status;
pub use fundamentals::handle;
pub use fundamentals::sync as sync_re;
pub use fundamentals::error;
pub use fundamentals::convert;
pub use fundamentals::fmt;
pub use fundamentals::io as abi_io;

pub use values::string;
pub use values::time as values_time;
pub use values::reflect;
pub use values::net;
pub use values::math;
pub use values::hash;

// ─── Versión + magic ──────────────────────────────────────────────

/// Versión del BMO ABI implementada por este kernel.
pub const BMO_ABI_VERSION: (u8, u8) = (1, 0);

/// Magic constant en headers BEF para identificar BMO ABI.
pub const BMO_ABI_MAGIC: u32 = u32::from_le_bytes(*b"BMO1");
