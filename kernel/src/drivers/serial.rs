//! Serial port (COM1: 0x3F8) for debug output.

const COM1: u16 = 0x3F8;

#[inline]
fn outb(port: u16, val: u8) {
    unsafe { core::arch::asm!("out dx, al", in("dx") port, in("al") val); }
}

#[inline]
fn inb(port: u16) -> u8 {
    let v: u8;
    unsafe { core::arch::asm!("in al, dx", in("dx") port, out("al") v); }
    v
}

pub fn init_serial() {
    outb(COM1 + 1, 0x00);  // Disable IRQs
    outb(COM1 + 3, 0x80);  // DLAB
    outb(COM1 + 0, 0x01);  // 115200 baud
    outb(COM1 + 1, 0x00);
    outb(COM1 + 3, 0x03);  // 8N1
    outb(COM1 + 2, 0xC7);  // FIFO
    outb(COM1 + 4, 0x0B);
}

pub fn serial_write_byte(b: u8) {
    while inb(COM1 + 5) & 0x20 == 0 {}
    outb(COM1, b);
}

pub fn serial_write(s: &str) {
    for b in s.bytes() {
        if b == b'\n' { serial_write_byte(b'\r'); }
        serial_write_byte(b);
    }
}

pub fn serial_read_byte() -> Option<u8> {
    if inb(COM1 + 5) & 1 != 0 {
        Some(inb(COM1))
    } else {
        None
    }
}

/// Write a u64 to serial as hex (without 0x prefix).
pub fn serial_write_u64(value: u64, min_width: usize) {
    if value == 0 {
        for _ in 0..min_width.min(1) {
            serial_write_byte(b'0');
        }
        if min_width == 0 {
            serial_write_byte(b'0');
        }
        return;
    }
    let mut buf = [0u8; 16];
    let mut i = 0;
    let mut v = value;
    while v > 0 {
        let digit = (v & 0xF) as u8;
        buf[i] = if digit < 10 { b'0' + digit } else { b'a' + digit - 10 };
        v >>= 4;
        i += 1;
    }
    while i < min_width && i < buf.len() {
        buf[i] = b'0';
        i += 1;
    }
    while i > 0 {
        i -= 1;
        serial_write_byte(buf[i]);
    }
}
