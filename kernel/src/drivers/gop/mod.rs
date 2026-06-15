#![allow(dead_code)]

//! GOP Display and Backbuffer Driver — UEFI GOP and static backbuffer management.

use crate::ui::fb::Framebuffer;

/// ARGB color (32-bit: 0xAARRGGBB).
#[derive(Debug, Clone, Copy)]
pub struct Color(pub u32);

impl Color {
    pub const BLACK:   Color = Color(0xFF000000);
    pub const WHITE:   Color = Color(0xFFFFFFFF);
    pub const RED:     Color = Color(0xFFFF0000);
    pub const GREEN:   Color = Color(0xFF00FF00);
    pub const BLUE:    Color = Color(0xFF0000FF);
    pub const YELLOW:  Color = Color(0xFFFFFF00);
    pub const CYAN:    Color = Color(0xFF00FFFF);
    pub const MAGENTA: Color = Color(0xFFFF00FF);
    pub const GRAY:    Color = Color(0xFF808080);
    pub const DARK_BG: Color = Color(0xFF1A1A2E);
    pub const ACCENT:  Color = Color(0xFF16213E);
    pub const HIGHLIGHT: Color = Color(0xFF0F3460);
    pub const NEON:    Color = Color(0xFFE94560);

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color(0xFF000000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32))
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Color(((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32))
    }
}

/// GOP framebuffer display state.
#[derive(Debug, Clone, Copy)]
pub struct GopDisplay {
    pub base: *mut u32,
    pub width: u32,
    pub height: u32,
    pub stride: u32, // pixels per scanline (may be > width due to padding)
}

static mut GOP_DISPLAY: Option<GopDisplay> = None;

/// Initialize the global GOP display from boot info.
pub fn init_gop(fb_base: u64, width: u32, height: u32, stride: u32) {
    unsafe {
        GOP_DISPLAY = Some(GopDisplay {
            base: fb_base as *mut u32,
            width,
            height,
            stride,
        });
    }
}

/// Get a reference to the global GOP display.
#[allow(static_mut_refs)]
pub fn display() -> Option<&'static GopDisplay> {
    unsafe { GOP_DISPLAY.as_ref() }
}

// ── Static Backbuffer ────────────────────────────────────────────────
// We define a static 8.29 MB backbuffer (1920x1080 resolution) in BSS.
// This prevents dynamic allocation failures due to physical memory fragmentation
// and guarantees a contiguous memory block at boot time.
const BACKBUFFER_SIZE: usize = 1920 * 1080;
static mut BACKBUFFER_MEM: [u32; BACKBUFFER_SIZE] = [0; BACKBUFFER_SIZE];

/// Get a Framebuffer representing the static backbuffer.
#[allow(static_mut_refs)]
pub fn get_backbuffer_fb() -> Framebuffer {
    unsafe {
        let addr = BACKBUFFER_MEM.as_mut_ptr() as u64;
        // 1920 width * 4 bytes per pixel = 7680 stride bytes
        Framebuffer::new(addr, 1920 * 4, 1920, 1080)
    }
}
