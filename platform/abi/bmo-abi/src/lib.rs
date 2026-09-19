//! `bmo_abi` -- el CONTRATO de BMO-X en x86-64: las dos puertas, la
//! convencion de llamada, el formato BEF y los tipos que cruzan la frontera.
//!
//! No es una libc: la libc de BMO-X es `toolchain/lang/base` (C) y `bmo-rt`
//! (Rust). Aqui solo vive lo que dos partes tienen que acordar.
//!
//! # Estructura (2026-09-19: lo que QUEDA, y todo tiene usuario)
//!
//! ```text
//! bmo_abi/
//! +-- fundamentals/   -- lo que cruza la frontera en cada puerta
//! |   +-- primitives/ -- bx_u8..u64, bx_i*, bx_f*, bx_bool
//! |   +-- status/     -- BmoStatus 16 B: codigo | banderas << 32 en rax, valor en rdx
//! |   +-- handle/     -- BmoHandle 64 bits (tag, kind, generacion, indice), HandleKind
//! |   +-- sync/       -- BmoSpinLock y atomicos (los usa el monton de bmo-rt)
//! +-- types/          -- la CONVENCION de llamada (la importan C e INTI) y la
//! |                      regla de disposicion de agregados (C, C++, COBOL, INTI)
//! +-- syscalls/       -- las DOS puertas (INVOKE 0x00, WAIT 0x02) y su superficie
//! +-- bef/            -- el formato BEF: cabecera, secciones, relocs, firma,
//! |                      requisitos, recursos, objetos (.bo), writer y validator
//! +-- bex.rs          -- el .bex es un BEF ejecutable
//! +-- dynobj/         -- texto, lista, tabla: los objetos del runtime de INTI
//! +-- profile/        -- el perfil de cada lenguaje
//! +-- cpu_profiles/   -- (sale en el corte 5: LEY 24, el perfil se MIDE)
//! ```
//!
//! ** Se fueron el 19-09, ~6.700 lineas sin un solo usuario vivo y tapadas por
//! veinticinco `#![allow(dead_code)]`: `values/`, `runtime/`, `ir/`,
//! `standards/`, `windowing/`, `fs/`, `surface/`, `error_code/`, diez de los
//! catorce `fundamentals/`, `types::{signature, field}` y
//! `bef::{loader, tls, manifest}`. Los `allow` se fueron con ellos: lo que se
//! muera a partir de ahora, lo dice el compilador.
//!
//! Ver `SPEC.md` para la especificacion completa.
//!
//! # [isa] x86-64 -- entero, y a proposito (2026-09-18)
//!
//! Este es **el ABI de BMO-X para x86-64**, no un ABI "portable" con un x86
//! dentro. La puerta es la instruccion `syscall`, sus argumentos van en
//! `rdi, rsi, rdx, r10, r8, r9` y vuelve codigo en `rax` y valor en `rdx`; una
//! llamada normal usa SEIS registros, `rdi, rsi, rdx, rcx, r8, r9`, y devuelve
//! en `rax` (ver `types::convention`, que es lo que leen los emisores); NO hay
//! puntero de hilo -- ningun emisor usa `fs:` y el kernel no programa
//! `FS_BASE` para Ring 3 --, y el formato BEF solo admite imagenes de
//! arquitectura `0x01`. Nada de eso se abstrae: una capa que "podria ser otra CPU" es un
//! camino que ninguna maquina de este repositorio ejecuta.
//!
//! En BMO-X todo es x86-64 MENOS los frontends de los compiladores
//! (`toolchain/tools/isa`). Un BMO-X de otra CPU es otro repositorio con su
//! propio ABI; lo unico que comparten es el byte de arquitectura del BEF.
#![no_std]
extern crate alloc;
pub mod fundamentals;
pub mod dynobj;
pub mod types;
pub mod bef;
pub mod bex;
pub mod syscalls;
pub mod profile;
pub mod cpu_profiles;

// --- Re-exports planos para uso ergonomico -------------------------

pub use fundamentals::primitives;
pub use fundamentals::status;
pub use fundamentals::handle;
pub use fundamentals::sync as sync_re;


// --- Version + magic ----------------------------------------------

/// Version del BMO ABI implementada por este kernel.
pub const BMO_ABI_VERSION: (u8, u8) = (2, 0);

/// Returns whether an artifact using `required` can run on this ABI.
/// Major versions are incompatible; minor versions are additive.
///
/// ** Sin mayor heredado desde el 2026-09-19: un `.bex` 1.0 solo podia llamar
/// a la tabla v1, que el kernel no despacha. Ver `bmo_bex_gate::abi_admisible`.
pub const fn supports_abi(required: (u8, u8)) -> bool {
    required.0 == BMO_ABI_VERSION.0 && required.1 <= BMO_ABI_VERSION.1
}

/// Magic constant en headers BEF para identificar BMO ABI.
pub const BMO_ABI_MAGIC: u32 = u32::from_le_bytes(*b"BMO1");

/// The CPU contract selected when this ABI crate was compiled.
///
/// A BEF producer can record this contract in its manifest; the BMO loader
/// must reject a binary whose required profile is not available at boot.
pub const BMO_CPU_PROFILE: cpu_profiles::CpuProfile = cpu_profiles::ACTIVE;

pub use crate as bmo_abi;
