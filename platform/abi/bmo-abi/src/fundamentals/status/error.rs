//! El texto humano de cada codigo de `BmoStatus`.
//!
//! ** Hasta el 2026-09-19 esto decia re-exportar `error_code/`, que tenia otra
//! copia de los numeros, y `fundamentals/error/` una tercera. Ninguna de las
//! dos tenia un usuario vivo y se fueron; los numeros de verdad son los que
//! contesta el kernel (`syscall/ops.rs`, `ERROR_*`).
//!
//! -- EL SEMAFORO (L6g) y las dos preguntas de antes (L6e, L6f) --------
//!
//! Que cuesta que falle, por que falla ESTA pieza, y que arrastro si la
//! toco. La ley esta en `META-KERNEL_HARD.md`.
//!
//! [carril]  AMARILLO     los constructores de error de `BmoStatus`
//! [cuesta]  NADA         se equivoca y un fallo sale con el codigo de otro
//! [riesgo]  ESPEJO       comparte verdad con los `ERROR_*` del kernel
//!                        (`Ultra_kernel_x86-64/.../syscall/ops.rs`)

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
