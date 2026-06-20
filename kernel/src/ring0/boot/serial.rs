//! Serial helpers used during early boot and tests.
//!
//! All functions are `no_std` and write to the COM1 port through
//! `crate::drivers::serial`. They never allocate and never touch the
//! framebuffer; visual logging is in `crate::boot::visual`.

use crate::drivers::serial;

/// Write a u64 as a 16-digit uppercase hexadecimal with `0x` prefix.
pub fn hex(val: u64) {
    const TABLE: &[u8; 16] = b"0123456789ABCDEF";
    serial::serial_write("0x");
    for i in (0..16).rev() {
        serial::serial_write_byte(TABLE[((val >> (i * 4)) & 0xF) as usize]);
    }
}

/// Write a u32 in decimal (no leading zeros, no sign).
pub fn u32_dec(mut val: u32) {
    let mut buf = [0u8; 10];
    let mut i = buf.len();
    if val == 0 {
        i -= 1;
        buf[i] = b'0';
    } else {
        while val > 0 {
            i -= 1;
            buf[i] = b'0' + (val % 10) as u8;
            val /= 10;
        }
    }
    serial::serial_write(core::str::from_utf8(&buf[i..]).unwrap_or("0"));
}
