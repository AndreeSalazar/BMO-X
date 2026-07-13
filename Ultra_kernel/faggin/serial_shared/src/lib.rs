//! COM1 serial helpers shared by every fagging stage.
//!
//! Each stage statically links this `rlib` and uses the same
//! `serial_write` / `serial_hex` / etc. The base address of COM1
//! is `0x3F8` and the line discipline is 115200 8N1.

#![allow(dead_code)]

use core::arch::asm;

pub const COM1: u16 = 0x3F8;

#[inline]
pub unsafe fn outb(port: u16, val: u8) {
    asm!("out dx, al", in("dx") port, in("al") val);
}

#[inline]
pub unsafe fn inb(port: u16) -> u8 {
    let v: u8;
    asm!("in al, dx", in("dx") port, out("al") v);
    v
}

pub fn init() {
    unsafe {
        outb(COM1 + 1, 0x00);
        outb(COM1 + 3, 0x80);
        outb(COM1 + 0, 0x01);
        outb(COM1 + 1, 0x00);
        outb(COM1 + 3, 0x03);
        outb(COM1 + 2, 0xC7);
        outb(COM1 + 4, 0x0B);
    }
}

pub unsafe fn put_byte(b: u8) {
    let mut t = 100_000u32;
    while inb(COM1 + 5) & 0x20 == 0 {
        t = t.saturating_sub(1);
        if t == 0 { return; }
    }
    outb(COM1, b);
}

pub fn puts(s: &str) {
    for &b in s.as_bytes() {
        if b == b'\n' { unsafe { put_byte(b'\r'); } }
        unsafe { put_byte(b); }
    }
}

pub fn hex(mut v: u64) {
    if v == 0 { unsafe { put_byte(b'0'); } return; }
    let mut buf = [0u8; 16];
    let mut i = 0;
    while v > 0 {
        buf[i] = b"0123456789abcdef"[(v & 0xF) as usize];
        v >>= 4; i += 1;
    }
    for j in (0..i).rev() { unsafe { put_byte(buf[j]); } }
}

pub fn dec(mut v: usize) {
    if v == 0 { unsafe { put_byte(b'0'); } return; }
    let mut buf = [0u8; 20];
    let mut i = 0;
    while v > 0 {
        buf[i] = b'0' + (v % 10) as u8;
        v /= 10; i += 1;
    }
    for j in (0..i).rev() { unsafe { put_byte(buf[j]); } }
}
