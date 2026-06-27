//! v2.0 — Cursor management.
//!
//! 16 sprites built-in (arrow, ibeam, wait, cross, resize, ...). En
//! v2.0 sólo se dibuja el arrow (el resto quedan como stubs).
//! Estado protegido con AtomicU8 spinlock.

#![allow(dead_code)]

use core::sync::atomic::{AtomicU8, AtomicBool, Ordering};

pub mod id {
    pub const ARROW: u8 = 0;
    pub const IBEAM: u8 = 1;
    pub const WAIT:  u8 = 2;
    pub const CROSS: u8 = 3;
    pub const SIZENWSE: u8 = 4;
    pub const SIZENESW: u8 = 5;
    pub const SIZEWE: u8 = 6;
    pub const SIZENS: u8 = 7;
    pub const SIZEALL: u8 = 8;
    pub const NO: u8 = 9;
    pub const HAND: u8 = 10;
    pub const APPSTARTING: u8 = 11;
    pub const HELP: u8 = 12;
    pub const PEN: u8 = 13;
    pub const CDLINK: u8 = 14;
    pub const CDNONE: u8 = 15;
}

static CURRENT: AtomicU8 = AtomicU8::new(id::ARROW);
static VISIBLE: AtomicBool = AtomicBool::new(true);

pub fn init() {
    CURRENT.store(id::ARROW, Ordering::Relaxed);
    VISIBLE.store(true, Ordering::Relaxed);
}

pub fn set(c: u8) { CURRENT.store(c & 0xF, Ordering::Relaxed); }
pub fn show() { VISIBLE.store(true, Ordering::Relaxed); }
pub fn hide() { VISIBLE.store(false, Ordering::Relaxed); }
pub fn current() -> u8 { CURRENT.load(Ordering::Relaxed) }
pub fn is_visible() -> bool { VISIBLE.load(Ordering::Relaxed) }

pub fn paint(x: i32, y: i32) {
    if !is_visible() { return; }
    let sprite: [[u8; 16]; 16] = arrow_sprite();
    let color = 0xFFFFFFFFu32;
    let shadow = 0x80000000u32;
    let (fbw, fbh) = unsafe { (crate::boot::info::FB_WIDTH, crate::boot::info::FB_HEIGHT) };
    for row in 0..16 {
        for col in 0..16 {
            let px = x + col as i32;
            let py = y + row as i32;
            if px < 0 || py < 0 || px >= fbw as i32 || py >= fbh as i32 { continue; }
            if sprite[row][col] == 1 {
                crate::dev::framebuffer::put_pixel(px as u32, py as u32, crate::dev::framebuffer::Color(color));
            } else if sprite[row][col] == 2 {
                crate::dev::framebuffer::put_pixel(px as u32, py as u32, crate::dev::framebuffer::Color(shadow));
            }
        }
    }
}

fn arrow_sprite() -> [[u8; 16]; 16] {
    [
        [1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],
        [1,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0],
        [1,2,1,0,0,0,0,0,0,0,0,0,0,0,0,0],
        [1,2,2,1,0,0,0,0,0,0,0,0,0,0,0,0],
        [1,2,2,2,1,0,0,0,0,0,0,0,0,0,0,0],
        [1,2,2,2,2,1,0,0,0,0,0,0,0,0,0,0],
        [1,2,2,2,2,2,1,0,0,0,0,0,0,0,0,0],
        [1,2,2,2,2,2,2,1,0,0,0,0,0,0,0,0],
        [1,2,2,2,2,2,2,2,1,0,0,0,0,0,0,0],
        [1,2,2,2,2,1,1,1,1,1,0,0,0,0,0,0],
        [1,2,2,1,2,1,0,0,0,0,0,0,0,0,0,0],
        [1,2,1,0,1,2,1,0,0,0,0,0,0,0,0,0],
        [1,1,0,0,0,1,2,1,0,0,0,0,0,0,0,0],
        [1,0,0,0,0,0,1,2,1,0,0,0,0,0,0,0],
        [0,0,0,0,0,0,0,1,2,1,0,0,0,0,0,0],
        [0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0],
    ]
}
