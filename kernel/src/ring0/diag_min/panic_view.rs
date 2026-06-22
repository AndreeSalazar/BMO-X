//! `ring0::diag_min::panic_view` — Imprime un panic con formato legible.

#![allow(dead_code)]

use super::serial;

/// Imprime un mensaje de panic al serial.
pub fn panic_view(module: &str, msg: &str) {
    serial::write("\n!!! PANIC !!!\n");
    serial::write("module: ");
    serial::write(module);
    serial::write("\nmsg:    ");
    serial::write(msg);
    serial::write("\n");
}

/// Imprime información de un CPU exception.
pub fn exception(vector: u8, error: u64) {
    serial::write("\n!!! CPU EXCEPTION !!!\n");
    serial::write("vector: 0x");
    serial::write_hex(vector as u64);
    serial::write("\nerror:  0x");
    serial::write_hex(error);
    serial::write("\n");
}
