//! Framebuffer globals — populated from `BootContext` at kernel entry.
//!
//! These are the simplest possible "interface" that the splash, syscall
//! handlers, and any future pixel-drawing code consume. They are written
//! exactly once from `_start` and then treated as read-only.

use boot_context::BootContext;

/// Linear framebuffer base address (XRGB-8888, 4 bytes per pixel).
pub static mut FB_ADDR: u64 = 0;
/// Framebuffer width in pixels.
pub static mut FB_WIDTH: u32 = 0;
/// Framebuffer height in pixels.
pub static mut FB_HEIGHT: u32 = 0;
/// Framebuffer stride in pixels (NOT bytes).
pub static mut FB_STRIDE: u32 = 0;
/// Framebuffer pixel format code (0=Unknown/RGB, 1=BGR, 2=RGB).
pub static mut FB_PIXEL_FORMAT: u32 = 0;

/// Populate the globals from a `BootContext` populated by the UEFI
/// chain. Safe to call once at kernel entry.
pub fn init_from(ctx: &BootContext) {
    unsafe {
        FB_ADDR = ctx.fb_addr;
        FB_WIDTH = ctx.fb_width;
        FB_HEIGHT = ctx.fb_height;
        FB_STRIDE = ctx.fb_stride;
        FB_PIXEL_FORMAT = ctx.fb_pixel_format;
    }
}

/// Returns true if a framebuffer is available and the splash should
/// render. The chain may pass an empty FB if UEFI GOP was not present.
pub fn has_fb() -> bool {
    unsafe { FB_ADDR != 0 && FB_WIDTH != 0 && FB_HEIGHT != 0 }
}
