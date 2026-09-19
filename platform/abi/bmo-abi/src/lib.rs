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
//! +-- syscalls/       -- Las DOS puertas (INVOKE 0x00, WAIT 0x02) y su superficie
//! +-- profile/        -- BmoLanguageProfile + ALL_PROFILES
//! ```
//!
//! Ver `SPEC.md` para la especificacion completa.
//!
//! # [isa] x86-64 -- entero, y a proposito (2026-09-18)
//!
//! Este es **el ABI de BMO-X para x86-64**, no un ABI "portable" con un x86
//! dentro. La puerta es la instruccion `syscall`, sus argumentos van en
//! `rdi, rsi, rdx, r10, r8, r9` (una llamada normal usa siete, con `rcx`: ver
//! `types::convention`), el resultado vuelve en `rax:rdx`, el puntero
//! del hilo es `FS_BASE`, y el formato BEF solo admite imagenes de arquitectura
//! `0x01`. Nada de eso se abstrae: una capa que "podria ser otra CPU" es un
//! camino que ninguna maquina de este repositorio ejecuta.
//!
//! En BMO-X todo es x86-64 MENOS los frontends de los compiladores
//! (`toolchain/tools/isa`). Un BMO-X de otra CPU es otro repositorio con su
//! propio ABI; lo unico que comparten es el byte de arquitectura del BEF.
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
