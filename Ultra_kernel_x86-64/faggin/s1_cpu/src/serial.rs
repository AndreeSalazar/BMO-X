//! **COM1** -- the only output that exists before there is a screen.
//!
//! Fifteen lines, and they earn a file: everything else in this stage can be
//! debugged by printing, and this is what printing IS. If it is wrong, every
//! other diagnostic in the boot lies quietly.
//!
//! [!] The write spins on the line-status register with a bounded counter. An
//! unbounded wait here hangs the machine before any of the tools that could say
//! why exist.

#[allow(unused_imports)]
use crate::*;

// ===================================================================
//  COM1 SERIAL
// ===================================================================

#[inline] pub unsafe fn outb(port: u16, val: u8) { asm!("out dx, al", in("dx") port, in("al") val); }
#[inline] pub unsafe fn inb(port: u16) -> u8 { let v: u8; asm!("in al, dx", in("dx") port, out("al") v); v }
pub unsafe fn put_byte(b: u8) { let mut t = 100_000u32; while inb(COM1 + 5) & 0x20 == 0 { t = t.saturating_sub(1); if t == 0 { return; } } outb(COM1, b); }
pub fn serial_init() { unsafe { outb(COM1 + 1, 0); outb(COM1 + 3, 0x80); outb(COM1 + 0, 1); outb(COM1 + 1, 0); outb(COM1 + 3, 3); outb(COM1 + 2, 0xC7); outb(COM1 + 4, 0xB); } }
pub unsafe fn put_str(s: &str) { for &b in s.as_bytes() { if b == b'\n' { put_byte(b'\r'); } put_byte(b); } }
pub unsafe fn put_hex(mut v: u64) { if v == 0 { put_byte(b'0'); return; } let mut b = [0u8; 16]; let mut i = 0; while v > 0 { b[i] = b"0123456789abcdef"[(v & 0xF) as usize]; v >>= 4; i += 1; } for j in (0..i).rev() { put_byte(b[j]); } }
pub unsafe fn put_dec(mut v: usize) { if v == 0 { put_byte(b'0'); return; } let mut b = [0u8; 20]; let mut i = 0; while v > 0 { b[i] = b'0' + (v % 10) as u8; v /= 10; i += 1; } for j in (0..i).rev() { put_byte(b[j]); } }
#[macro_export]
macro_rules! ser_print { ($s:expr) => { unsafe { put_str($s); } }; }
#[macro_export]
macro_rules! ser_hex { ($v:expr) => { unsafe { put_hex($v); } }; }
#[macro_export]
macro_rules! ser_dec { ($v:expr) => { unsafe { put_dec($v); } }; }
