//! `bmo_abi` -- BMO ABI: la convencion y el "stdlib minimo" nativo de BMO.
//!
//! **Reemplaza al C ABI** (cdecl/stdcall/Win64/SysV AMD64) y a su stdlib
//! (`<stdint.h>`, `<stddef.h>`, `<string.h>`, `<errno.h>`, `<time.h>`, etc).
//!
//! # Estructura
//!
//! ```text
//! bmo_abi/
//! +-- fundamentals/   -- Tipos que TODO codigo usa
//! |   +-- primitives/ -- int, bool, float (bx_u8..u64, bx_i*, bx_f*)
//! |   +-- status/      -- BmoStatus 16-byte, StatusFlags
//! |   +-- handle/      -- BmoHandle 64-bit + ops (dup, close, wait)
//! |   +-- capability/  -- BmoCap, BmoCapSet (bitset de permisos)
//! |   +-- option/      -- BmoOption<T> FFI-safe
//! |   +-- result/      -- BmoResult<T, E> FFI-safe
//! |   +-- error/       -- BmoError 16-byte unificado
//! |   +-- convert/     -- BmoStatus <-> BmoError <-> ErrorCode
//! |   +-- string/      -- BmoStr (borrowed), BmoString (owned)
//! |   +-- memory/      -- BmoSlice, BmoRange, BmoAligned
//! |   +-- buffer/      -- BmoBuffer shared memory descriptor
//! |   +-- allocator/   -- BmoAllocator trait + Global wrapper
//! |   +-- fmt/         -- BmoFormatter stack-allocated
//! |   +-- sync/        -- BmoAtomicU32/U64/Bool, MemOrder, BmoSpinLock
//! |
//! +-- values/         -- Tipos valor con semantica propia
//! |   +-- time/        -- BmoInstant, BmoDuration
//! |   +-- clock/       -- BmoClockId, sleep, sleep_until
//! |   +-- uuid/        -- BmoUuid 128-bit (RFC 4122)
//! |   +-- version/     -- BmoVersion semver (major.minor.patch)
//! |   +-- math/        -- sqrt, sin, cos, pow
//! |   +-- hash/        -- FNV-1a, CRC32c, CRC32
//! |   +-- net/         -- BmoIpv4Addr, BmoIpv6Addr, BmoSocketAddr
//! |   +-- reflect/     -- ReflectQuery
//! |
//! +-- runtime/        -- TypeRegistry, VTableStore, LangBridge
//! +-- windowing/      -- Contrato de ventanas
//! +-- fs/             -- File/Dir handles, OpenFlags, Stat
//! +-- surface/        -- Formatos de pixel, surfaces CPU/GPU
//! +-- error_code/     -- BmoErrorCode enum, BmoErrorSeverity, constants
//! +-- bef/            -- Formato BEF (header, secciones, relocs)
//! +-- syscalls/       -- Syscall numbers 0x100..0x1FF
//! +-- profile/        -- BmoLanguageProfile + ALL_PROFILES
//! ```
//!
//! Ver `SPEC.md` para la especificacion completa.
#![no_std]
#![allow(dead_code)]
extern crate alloc;
pub mod fundamentals;
pub mod values;
pub mod runtime;
pub mod dynobj;
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

// --- Re-exports planos para uso ergonomico -------------------------

pub use fundamentals::primitives;
pub use fundamentals::status;
pub use fundamentals::handle;
pub use fundamentals::sync as sync_re;

pub use values::time as values_time;

// --- Version + magic ----------------------------------------------

/// Version del BMO ABI implementada por este kernel.
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
