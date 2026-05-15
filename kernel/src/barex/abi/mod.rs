//! BMO ABI — la convención y el "stdlib mínimo" nativo de FastOS.
//!
//! **Reemplaza al C ABI** (cdecl/stdcall/Win64/SysV AMD64) y a su stdlib
//! (`<stdint.h>`, `<stddef.h>`, `<string.h>`, `<errno.h>`, `<time.h>`, etc.).
//!
//! Spec maestra: `combo_Window_Extractor/MAPA de Window/02_BEF_Format/BMO_ABI_Spec.md`.
//! Mapa visual: `_README.md` en esta carpeta.
//!
//! ## Sub-módulos
//!
//! - [`primitives`] — tipos numéricos (`bx_u8..u64`, `bx_i*`, `bx_f16/32/64`, `bx_bool`).
//! - [`memory`]     — slices, ranges, alignment helpers (sustituye `void*` + `size_t`).
//! - [`string`]     — `BmoStr`, `BmoString`, ASCII helpers (sustituye `char*` + `wchar_t*`).
//! - [`handle`]     — `BmoHandle` 64-bit con generación (sustituye `HANDLE`/`fd`/`IUnknown*`).
//! - [`status`]     — `BmoStatus` + `BxError` (sustituye `HRESULT`/`errno`/`GetLastError`).
//! - [`calling`]    — convención de llamada (registros, stack, red zone).
//! - [`async_io`]   — Submission/Completion Queues (sustituye `OVERLAPPED`/IOCP/callbacks).
//! - [`time`]       — `BmoInstant`, `BmoDuration` (sustituye `time_t`/`timespec`/`GetTickCount`).
//! - [`compat`]     — thunks Win64 / SysV → BMO ABI para FFI con código C heredado.
//!
//! ### Sub-módulos genéricos multi-lenguaje (Sesión 7)
//!
//! - [`type_system`]  — descriptores universales (sustituye RTTI / `Type` / `class`).
//! - [`vtable`]       — despacho dinámico (sustituye vtables C++ / COM / dyn Trait).
//! - [`closure`]      — closures de primera clase (C ABI no los tiene).
//! - [`exception`]    — modelo unificado de unwinding (sustituye Itanium EH / SEH).
//! - [`reflect`]      — reflection runtime sobre cualquier BEF cargado.
//! - [`lang_bridge`]  — registro de lenguajes (Rust, C++, Java, Swift, Python, etc).
//! - [`marshal`]      — conversiones Lang ↔ BMO ↔ Lang.

#![allow(dead_code)]

pub mod primitives;
pub mod memory;
pub mod string;
pub mod handle;
pub mod status;
pub mod calling;
pub mod async_io;
pub mod time;
pub mod compat;
pub mod sync;
pub mod option;
pub mod result;

// ─── Sesión 7: arquitectura genérica multi-lenguaje ───────────────────
pub mod type_system;
pub mod vtable;
pub mod closure;
pub mod exception;
pub mod reflect;
pub mod lang_bridge;
pub mod marshal;

// ─── Sesión 8: agregador único ────────────────────────────────────────
pub mod runtime;

// ─── Re-exports planos para uso ergonómico ────────────────────────────
//   Apps Rust pueden hacer `use crate::barex::abi::*;` y obtener todo lo
//   esencial sin navegar sub-módulos.

pub use primitives::*;
pub use memory::{BmoSlice, BmoMutSlice, BmoRange, align_up, align_down};
pub use string::{BmoStr, BmoString};
pub use handle::{BmoHandle, HandleKind};
pub use status::{BmoStatus, StatusFlags};
pub use calling::{STACK_ALIGNMENT, SHADOW_SPACE, RED_ZONE_SIZE, ARG_GPRS, ARG_XMMS};
pub use time::{BmoInstant, BmoDuration};
pub use sync::{BmoAtomicU32, BmoAtomicU64, BmoAtomicBool, BmoMutex, BmoFutex, MemOrder};
pub use option::BmoOption;
pub use result::BmoResult;

// Re-exports de la capa genérica (Sesión 7-8).
pub use type_system::{TypeDescriptor, TypeId, TypeKind, TypeLayout, TypeRegistry};
pub use vtable::{BmoVTable, BmoFatPtr, VTableEntry};
pub use closure::{BmoClosure, ClosureEnv, ClosureSig, ClosureKind};
pub use exception::{BmoPanic, PanicKind, UnwindTable, UnwindEntry, ResumeToken};
pub use reflect::{Mirror, ReflectQuery};
pub use lang_bridge::{LangDescriptor, LangRegistry, LangFeatures, LangVersion};
pub use marshal::{Marshaller, MarshalError};
pub use runtime::{BmoRuntime, RuntimeStats};

/// Versión del BMO ABI implementada por este kernel.
pub const BMO_ABI_VERSION: (u8, u8) = (1, 0);

/// Magic constant en headers BEF para identificar BMO ABI.
pub const BMO_ABI_MAGIC: u32 = u32::from_le_bytes(*b"BMO1");
