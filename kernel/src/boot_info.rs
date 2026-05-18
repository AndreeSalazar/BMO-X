//! Boot info globals — stores the BootInfo pointer from the bootloader,
//! plus simple framebuffer + input globals consumed by syscalls (Ring 3 desktop).

/// Global pointer to the BootInfo structure passed by the bootloader.
pub static mut BOOT_INFO: *const fastos_boot_protocol::BootInfo = core::ptr::null();

/// Global GSP firmware info (populated from BootInfo for backward compat).
pub static mut GSP_FW_ADDR: u64 = 0;
pub static mut GSP_FW_SIZE: u64 = 0;

// ─── Framebuffer globals (used by syscalls 0x60-0x63) ───────────────
/// Linear framebuffer base address (XRGB-8888, 4 bytes per pixel).
pub static mut FB_ADDR: u64 = 0;
/// Framebuffer width in pixels.
pub static mut FB_WIDTH: u32 = 0;
/// Framebuffer height in pixels.
pub static mut FB_HEIGHT: u32 = 0;
/// Framebuffer stride in pixels (NOT bytes).
pub static mut FB_STRIDE: u32 = 0;
