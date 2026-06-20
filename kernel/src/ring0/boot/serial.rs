//! Serial helpers consolidados (v1.7.4).
//!
//! Antes existían 6+ copias de `serial_write_u32`/`serial_hex` repartidas
//! por `vmm.rs`, `rt.rs`, `paging.rs`, `idt.rs`, `allocator.rs`, etc.
//! Aquí vive una sola implementación. Todas las funciones son `no_std`,
//! no alocan, y escriben por COM1 vía `crate::device::serial`.

use crate::device::serial;

const HEX_TABLE: &[u8; 16] = b"0123456789ABCDEF";

/// Escribe un u64 como hex 16 dígitos upper-case con prefijo "0x".
/// Si `digits` < 16, se trunca el prefijo (sin padding).
pub fn hex(val: u64) {
    serial::serial_write("0x");
    for i in (0..16).rev() {
        serial::serial_write_byte(HEX_TABLE[((val >> (i * 4)) & 0xF) as usize]);
    }
}

/// Escribe un u64 en decimal (sin padding, sin signo).
pub fn u64_dec(mut val: u64) {
    if val == 0 {
        serial::serial_write_byte(b'0');
        return;
    }
    let mut buf = [0u8; 20]; // u64::MAX = 18446744073709551615 = 20 dígitos
    let mut i = buf.len();
    while val > 0 {
        i -= 1;
        buf[i] = b'0' + (val % 10) as u8;
        val /= 10;
    }
    let s = core::str::from_utf8(&buf[i..]).unwrap_or("0");
    serial::serial_write(s);
}

/// Escribe un u32 en decimal (sin padding, sin signo).
pub fn u32_dec(val: u32) {
    u64_dec(val as u64);
}

/// Escribe un buffer como hex (sin prefijo, sin separador).
/// Usado para imprimir dumps de memoria y tablas IDT/GDT.
pub fn hex_bytes(data: &[u8]) {
    for &b in data {
        serial::serial_write_byte(HEX_TABLE[(b >> 4) as usize]);
        serial::serial_write_byte(HEX_TABLE[(b & 0xF) as usize]);
    }
}
