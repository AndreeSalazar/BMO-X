//! `bmo_abi` â€” BMO ABI: la convenciÃ³n y el "stdlib mÃ­nimo" nativo de BMO.
//!
//! **Reemplaza al C ABI** (cdecl/stdcall/Win64/SysV AMD64) y a su stdlib
//! (`<stdint.h>`, `<stddef.h>`, `<string.h>`, `<errno.h>`, `<time.h>`, etc).
//!
//! # Estructura
//!
//! ```text
//! bmo_abi/
//! â”œâ”€â”€ fundamentals/   â€” Tipos que TODO cÃ³digo usa
//! â”‚   â”œâ”€â”€ primitives/ â€” int, bool, float (bx_u8..u64, bx_i*, bx_f*)
//! â”‚   â”œâ”€â”€ status/      â€” BmoStatus 16-byte, StatusFlags
//! â”‚   â”œâ”€â”€ handle/      â€” BmoHandle 64-bit + ops (dup, close, wait)
//! â”‚   â”œâ”€â”€ capability/  â€” BmoCap, BmoCapSet (bitset de permisos)
//! â”‚   â”œâ”€â”€ option/      â€” BmoOption<T> FFI-safe
//! â”‚   â”œâ”€â”€ result/      â€” BmoResult<T, E> FFI-safe
//! â”‚   â”œâ”€â”€ error/       â€” BmoError 16-byte unificado
//! â”‚   â”œâ”€â”€ convert/     â€” BmoStatus â†” BmoError â†” ErrorCode
//! â”‚   â”œâ”€â”€ string/      â€” BmoStr (borrowed), BmoString (owned)
//! â”‚   â”œâ”€â”€ memory/      â€” BmoSlice, BmoRange, BmoAligned
//! â”‚   â”œâ”€â”€ buffer/      â€” BmoBuffer shared memory descriptor
//! â”‚   â”œâ”€â”€ allocator/   â€” BmoAllocator trait + Global wrapper
//! â”‚   â”œâ”€â”€ io/          â€” BmoRead, BmoWrite, BmoSeek, BmoPipe
//! â”‚   â”œâ”€â”€ fmt/         â€” BmoFormatter stack-allocated
//! â”‚   â””â”€â”€ sync/        â€” BmoAtomicU32/U64/Bool, MemOrder, BmoSpinLock
//! â”‚
//! â”œâ”€â”€ values/         â€” Tipos valor con semÃ¡ntica propia
//! â”‚   â”œâ”€â”€ time/        â€” BmoInstant, BmoDuration
//! â”‚   â”œâ”€â”€ clock/       â€” BmoClockId, sleep, sleep_until
//! â”‚   â”œâ”€â”€ uuid/        â€” BmoUuid 128-bit (RFC 4122)
//! â”‚   â”œâ”€â”€ version/     â€” BmoVersion semver (major.minor.patch)
//! â”‚   â”œâ”€â”€ math/        â€” sqrt, sin, cos, pow
//! â”‚   â”œâ”€â”€ hash/        â€” FNV-1a, CRC32c, CRC32
//! â”‚   â”œâ”€â”€ net/         â€” BmoIpv4Addr, BmoIpv6Addr, BmoSocketAddr
//! â”‚   â””â”€â”€ reflect/     â€” ReflectQuery
//! â”‚
//! â”œâ”€â”€ runtime/        â€” TypeRegistry, VTableStore, LangBridge
//! â”œâ”€â”€ windowing/      â€” Contrato de ventanas
//! â”œâ”€â”€ fs/             â€” File/Dir handles, OpenFlags, Stat
//! â”œâ”€â”€ surface/        â€” Formatos de pixel, surfaces CPU/GPU
//! â”œâ”€â”€ error_code/     â€” BmoErrorCode enum, BmoErrorSeverity, constants
//! â”œâ”€â”€ bef/            â€” Formato BEF (header, secciones, relocs)
//! â”œâ”€â”€ syscalls/       â€” Syscall numbers 0x100..0x1FF
//! â””â”€â”€ profile/        â€” BmoLanguageProfile + ALL_PROFILES
//! ```
//!
//! Ver `SPEC.md` para la especificaciÃ³n completa.
#![no_std]
#![allow(dead_code)]
extern crate alloc;
pub mod fundamentals;
pub mod values;
pub mod runtime;
pub mod types;
pub mod ir;
pub mod windowing;
pub mod fs;
pub mod surface;
pub mod error_code;
pub mod bef;
pub mod bex;
pub mod syscalls;
pub mod profile;
pub mod cpu_profiles;
pub mod asm;
pub mod standards;

// â”€â”€â”€ Re-exports planos para uso ergonÃ³mico â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

pub use fundamentals::primitives;
pub use fundamentals::status;
pub use fundamentals::handle;
pub use fundamentals::sync as sync_re;

pub use values::time as values_time;

// â”€â”€â”€ VersiÃ³n + magic â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

/// VersiÃ³n del BMO ABI implementada por este kernel.
pub const BMO_ABI_VERSION: (u8, u8) = (2, 0);
pub const BMO_ABI_LEGACY_VERSION: (u8, u8) = (1, 0);

/// Returns whether an artifact using `required` can run on this ABI.
/// Major versions are incompatible; minor versions are additive.
pub const fn supports_abi(required: (u8, u8)) -> bool {
    (required.0 == BMO_ABI_VERSION.0 && required.1 <= BMO_ABI_VERSION.1)
        || (required.0 == BMO_ABI_LEGACY_VERSION.0
            && required.1 <= BMO_ABI_LEGACY_VERSION.1)
}

/// Magic constant en headers BEF para identificar BMO ABI.
pub const BMO_ABI_MAGIC: u32 = u32::from_le_bytes(*b"BMO1");

/// The CPU contract selected when this ABI crate was compiled.
///
/// A BEF producer can record this contract in its manifest; the BMO loader
/// must reject a binary whose required profile is not available at boot.
pub const BMO_CPU_PROFILE: cpu_profiles::CpuProfile = cpu_profiles::ACTIVE;

pub use crate as bmo_abi;
