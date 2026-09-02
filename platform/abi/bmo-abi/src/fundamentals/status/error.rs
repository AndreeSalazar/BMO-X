//! ErrorCode plain constants for C/FFI compatibility.
//!
//! The canonical definitions now live in `crate::bmo_abi::error_code`.
//! This file re-exports them so existing code using
//! `crate::bmo_abi::fundamentals::status::error_code::*` continues to compile.
//!
//! -- EL SEMAFORO (L6g) y las dos preguntas de antes (L6e, L6f) --------
//!
//! Que cuesta que falle, por que falla ESTA pieza, y que arrastro si la
//! toco. La ley esta en `META-KERNEL_HARD.md`.
//!
//! [carril]  AMARILLO     los constructores de error de `BmoStatus`
//! [cuesta]  NADA         se equivoca y un fallo sale con el codigo de otro
//! [riesgo]  ESPEJO       comparte verdad con `error_code/` y con
//!                        `error/mod.rs`

/// Devuelve un texto humano para el codigo de error. Cero asignacion.
pub const fn message(code: u32) -> &'static str {
    match code {
        0 => "ok",
        1 => "out of memory",
        2 => "invalid handle",
        3 => "permission denied",
        4 => "not found",
        5 => "busy",
        6 => "timeout",
        7 => "invalid argument",
        8 => "i/o error",
        9 => "internal error",
        10 => "unsupported",
        11 => "cancelled",
        12 => "deadlock",
        13 => "try again",
        14 => "buffer too small",
        15 => "invalid state",
        16 => "checksum mismatch",
        17 => "version mismatch",
        18 => "path not found",
        19 => "already exists",
        20 => "end of stream",
        _ => "unknown error",
    }
}
