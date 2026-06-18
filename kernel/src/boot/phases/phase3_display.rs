//! Phase 3 — Display.
//!
//! Validates the GOP framebuffer, initialises the GOP driver, and paints the
//! boot banner. After this phase returns, `desktop::fb_fill`, `desktop::fb_text`,
//! and the welcome screen are usable.

use crate::{boot::log, desktop, drivers};
use fastos_boot_protocol;

pub fn run(bi: &fastos_boot_protocol::BootInfo, prev_end: u64) -> u64 {
    log::info("phase3", "=== Phase 3: Display ===");
    crate::boot::visual::log("phase3", "=== Phase 3: Display ===",
        crate::boot::visual::color::HEADER);

    if bi.fb_addr == 0 {
        log::fault("phase3", "No framebuffer; cannot start visual desktop");
    }
    if bi.fb_width == 0 || bi.fb_height == 0 || bi.fb_stride == 0 {
        log::fault("phase3", "Invalid framebuffer dimensions");
    }
    if bi.fb_addr < 0x100000 || bi.fb_addr > 0xFFFFFFFF {
        log::fault("phase3", "Framebuffer address out of usable range");
    }

    let fb_size_mb = (bi.fb_width as u64 * bi.fb_height as u64 * 4) / (1024 * 1024);
    log::info_u64("phase3", "Framebuffer base", bi.fb_addr);
    log::info_u64("phase3", "Resolution", bi.fb_width as u64);
    crate::drivers::serial::serial_write("  x ");
    crate::boot::serial::hex(bi.fb_height as u64);
    crate::drivers::serial::serial_write("\n");
    log::info_u64("phase3", "Stride (pixels)", bi.fb_stride as u64);
    log::info_u64("phase3", "Framebuffer size (MB)", fb_size_mb);

    drivers::gop::init_gop(bi.fb_addr, bi.fb_width, bi.fb_height, bi.fb_stride);
    log::info("phase3", "GOP display initialized");

    desktop::fb_fill(0, 0, bi.fb_width, 34, 0xFF101820);
    desktop::fb_text(12, 9, b"FastOS boot: GOP online, storage/net deferred, entering safe welcome...", 0xFF76B900);

    let phase3_end = crate::arch::cpu::rdtsc();
    log::info_u64("phase3", "Phase 3 time (TSC ticks)", phase3_end - prev_end);
    phase3_end
}
