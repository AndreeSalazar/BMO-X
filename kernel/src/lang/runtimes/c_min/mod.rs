//! `lang::runtimes::c_min` — Runtime mínimo para C y BMO.
//!
//! Es lo que se linkea con cada binario compilado por el AOT cuando
//! el lenguaje requiere runtime (C, C++ sin allocators, BMO con I/O).
//!
//! ## Componentes
//!
//! - `start`   — punto de entrada `_start`, llama `main(argc, argv)`
//! - `mem`     — `memcpy`, `memset`, `memmove`, `memcmp`
//! - `syscall` — wrappers de BMO ABI (thin wrappers sobre `syscall`)
//! - `string`  — `strlen`, `strcmp`, `strcpy`
//! - `exit`    — `_exit`, `atexit` (no-op)
//!
//! ## Tamaño
//!
//! El runtime completo es **~2 KB** de código x86-64. Se incluye
//! solo si el binario lo necesita (linker decide).

#![allow(dead_code)]

pub mod start;
pub mod mem;
pub mod syscall;
pub mod string;
pub mod exit;

/// Versión del runtime.
pub const C_MIN_VERSION: (u8, u8) = (1, 0);

/// Tamaño aproximado del runtime en bytes.
pub const C_MIN_SIZE_BYTES: u32 = 2048;
