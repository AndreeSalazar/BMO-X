//! Tipos enteros canonicos del BMO ABI. Reemplaza `<stdint.h>` y `<stddef.h>`.
//!
//! Garantizado en todas las plataformas BMO (x86-64): tamanos fijos,
//! sin `int` ambiguo, sin `long` que cambie con la plataforma.

#![allow(non_camel_case_types)]

// --- Sin signo ---------------------------------------------------------
pub type bx_u8 = u8;
pub type bx_u16 = u16;
pub type bx_u32 = u32;
pub type bx_u64 = u64;
pub type bx_u128 = u128;

// --- Con signo --------------------------------------------------------
pub type bx_i8 = i8;
pub type bx_i16 = i16;
pub type bx_i32 = i32;
pub type bx_i64 = i64;
pub type bx_i128 = i128;

// --- Tamanos/punteros -------------------------------------------------
//   En x86-64 ambos son 64-bit. Si algun dia se porta a otra arch, este
//   alias se actualiza UN solo punto.
pub type bx_usize = u64;
pub type bx_isize = i64;
pub type bx_uptr = u64; // sustituye uintptr_t
pub type bx_iptr = i64; // sustituye intptr_t

// --- Tipos especializados (semanticos) --------------------------------
/// Offset dentro de un buffer/archivo (ej. `seek`, `mmap`).
pub type bx_offset = i64;
/// Identificador de proceso BEF.
pub type bx_pid = u32;
/// Identificador de thread.
pub type bx_tid = u32;
/// Identificador de CPU logica (0..=11 en el 5600X con SMT).
pub type bx_cpu_id = u8;

// --- Constantes de limites (sustituye limits.h) -----------------------
pub const BX_U8_MAX: bx_u8 = u8::MAX;
pub const BX_U16_MAX: bx_u16 = u16::MAX;
pub const BX_U32_MAX: bx_u32 = u32::MAX;
pub const BX_U64_MAX: bx_u64 = u64::MAX;

pub const BX_I8_MIN: bx_i8 = i8::MIN;
pub const BX_I8_MAX: bx_i8 = i8::MAX;
pub const BX_I16_MIN: bx_i16 = i16::MIN;
pub const BX_I16_MAX: bx_i16 = i16::MAX;
pub const BX_I32_MIN: bx_i32 = i32::MIN;
pub const BX_I32_MAX: bx_i32 = i32::MAX;
pub const BX_I64_MIN: bx_i64 = i64::MIN;
pub const BX_I64_MAX: bx_i64 = i64::MAX;

/// Marcador de "no hay valor" para tipos `bx_u64` (sustituye al feo
/// `(uint64_t)-1` tipico de C).
pub const BX_U64_NONE: bx_u64 = u64::MAX;
pub const BX_U32_NONE: bx_u32 = u32::MAX;
