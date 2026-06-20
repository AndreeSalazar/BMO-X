//! ÑEXO std::io — E/S serial y framebuffer.

#![allow(dead_code)]

use crate::bmo_core::lang::bmo::runtime::io as rt;

/// Imprimir string a serial.
pub fn print(s: &str) { rt::serial_write(s); }

/// Imprimir string con newline.
pub fn println(s: &str) { rt::serial_write(s); rt::serial_write_byte(b'\n'); }

/// Imprimir byte.
pub fn put(b: u8) { rt::serial_write_byte(b); }

/// Leer byte (non-blocking).
pub fn get() -> Option<u8> { rt::serial_read_byte() }

/// Imprimir número como decimal.
pub fn print_num(n: u64) {
    let mut buf = [0u8; 20]; let mut i = buf.len(); let mut val = n;
    if val == 0 { i -= 1; buf[i] = b'0'; } else { while val > 0 { i -= 1; buf[i] = b'0' + (val % 10) as u8; val /= 10; } }
    rt::serial_write(core::str::from_utf8(&buf[i..]).unwrap_or("0"));
}

/// Imprimir número como hex.
pub fn print_hex(n: u64) {
    rt::serial_write("0x");
    let mut buf = [0u8; 16];
    for i in 0..16 { let nibble = (n >> (60 - i * 4)) & 0xF; buf[i] = if nibble < 10 { b'0' + nibble as u8 } else { b'a' + (nibble - 10) as u8 }; }
    rt::serial_write(core::str::from_utf8(&buf).unwrap_or("0"));
}
