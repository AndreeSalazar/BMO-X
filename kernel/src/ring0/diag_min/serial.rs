//! `ring0::diag_min::serial` — Helpers de serial sin locks.

#![allow(dead_code)]

use crate::dev::console;

/// Inicializa el serial mínimo (no-op en v1.8.8).
pub fn init() {}

/// Escribe una string al COM1.
pub fn write(s: &str) {
    console::serial_write(s);
}

/// Escribe un u64 en hexadecimal al COM1.
pub fn write_hex(val: u64) {
    console::serial_write_u64(val, 16);
}
