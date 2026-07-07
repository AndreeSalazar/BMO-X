//! Tipos enteros canÃ³nicos del BMO ABI. Reemplaza `<stdint.h>` y `<stddef.h>`.
//!
//! Garantizado en todas las plataformas BMO (x86-64): tamaÃ±os fijos,
//! sin `int` ambiguo, sin `long` que cambie con la plataforma.

#![allow(non_camel_case_types)]

// â”€â”€â”€ Sin signo â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
pub type bx_u8  = u8;
pub type bx_u16 = u16;
pub type bx_u32 = u32;
pub type bx_u64 = u64;
pub type bx_u128 = u128;

// â”€â”€â”€ Con signo â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
pub type bx_i8  = i8;
pub type bx_i16 = i16;
pub type bx_i32 = i32;
pub type bx_i64 = i64;
pub type bx_i128 = i128;

// â”€â”€â”€ TamaÃ±os/punteros â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
//   En x86-64 ambos son 64-bit. Si algÃºn dÃ­a se porta a otra arch, este
//   alias se actualiza UN solo punto.
pub type bx_usize = u64;
pub type bx_isize = i64;
pub type bx_uptr  = u64;  // sustituye uintptr_t
pub type bx_iptr  = i64;  // sustituye intptr_t

// â”€â”€â”€ Tipos especializados (semÃ¡nticos) â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
/// Offset dentro de un buffer/archivo (ej. `seek`, `mmap`).
pub type bx_offset = i64;
/// Identificador de proceso BEF.
pub type bx_pid = u32;
/// Identificador de thread.
pub type bx_tid = u32;
/// Identificador de CPU lÃ³gica (0..=11 en el 5600X con SMT).
pub type bx_cpu_id = u8;

// â”€â”€â”€ Constantes de lÃ­mites (sustituye limits.h) â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
pub const BX_U8_MAX:  bx_u8  = u8::MAX;
pub const BX_U16_MAX: bx_u16 = u16::MAX;
pub const BX_U32_MAX: bx_u32 = u32::MAX;
pub const BX_U64_MAX: bx_u64 = u64::MAX;

pub const BX_I8_MIN:  bx_i8  = i8::MIN;
pub const BX_I8_MAX:  bx_i8  = i8::MAX;
pub const BX_I16_MIN: bx_i16 = i16::MIN;
pub const BX_I16_MAX: bx_i16 = i16::MAX;
pub const BX_I32_MIN: bx_i32 = i32::MIN;
pub const BX_I32_MAX: bx_i32 = i32::MAX;
pub const BX_I64_MIN: bx_i64 = i64::MIN;
pub const BX_I64_MAX: bx_i64 = i64::MAX;

/// Marcador de "no hay valor" para tipos `bx_u64` (sustituye al feo
/// `(uint64_t)-1` tÃ­pico de C).
pub const BX_U64_NONE: bx_u64 = u64::MAX;
pub const BX_U32_NONE: bx_u32 = u32::MAX;
