//! v2.0 — Cursor management.
//!
//! 16 sprites built-in (arrow, ibeam, wait, cross, resize, ...). En
//! v2.0 sólo se dibuja el arrow (el resto quedan como stubs).

#![allow(dead_code)]

use crate::drivers::gop;

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

static mut CURRENT: u8 = id::ARROW;
static mut VISIBLE: bool = true;

pub fn init() {
    unsafe {
        CURRENT = id::ARROW;
        VISIBLE = true;
    }
}

pub fn set(c: u8) { unsafe { CURRENT = c & 0xF; } }
pub fn show() { unsafe { VISIBLE = true; } }
pub fn hide() { unsafe { VISIBLE = false; } }
pub fn current() -> u8 { unsafe { CURRENT } }
pub fn is_visible() -> bool { unsafe { VISIBLE } }

/// Dibuja el cursor en la posición (x, y) sobre el framebuffer.
/// Sólo soporta ARROW en v2.0; los demás caen al arrow.
pub fn paint(x: i32, y: i32) {
    if !is_visible() { return; }
    let c = current();
    // Sprite 16×16 (1bpp + máscara). En v2.0 el arrow es hardcoded:
    let sprite: [[u8; 16]; 16] = arrow_sprite();
    let color = 0xFFFFFFFFu32;
    let shadow = 0x80000000u32;
    let _ = c;
    for row in 0..16 {
        for col in 0..16 {
            let px = x + col as i32;
            let py = y + row as i32;
            if px < 0 || py < 0 || px >= 1920 || py >= 1080 { continue; }
            if sprite[row][col] == 1 {
                gop::put_pixel(px as u32, py as u32, gop::Color(color));
            } else if sprite[row][col] == 2 {
                gop::put_pixel(px as u32, py as u32, gop::Color(shadow));
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
