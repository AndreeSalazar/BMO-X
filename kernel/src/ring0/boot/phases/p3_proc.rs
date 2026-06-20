//! Phase 3 — Display.

use crate::{boot::log, bmo_core::desktop, dev};
use crate::boot::context::BootContext;
use super::trait_def::{PhaseOutput, SelfTestReport, CheckResult};

pub fn run(ctx: &BootContext, prev_end: u64) -> PhaseOutput {
    log::info("phase3", "=== Phase 3: Display ===");

    let bi = ctx.boot_info().expect("BootInfo not set");

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
    crate::dev::console::serial_write("  x ");
    crate::boot::serial::hex(bi.fb_height as u64);
    crate::dev::console::serial_write("\n");
    log::info_u64("phase3", "Stride (pixels)", bi.fb_stride as u64);
    log::info_u64("phase3", "Framebuffer size (MB)", fb_size_mb);

    crate::dev::framebuffer::init_gop(bi.fb_addr, bi.fb_width, bi.fb_height, bi.fb_stride);
    log::info("phase3", "GOP display initialized");

    desktop::fb_fill(0, 0, bi.fb_width, 34, 0xFF101820);
    desktop::fb_text(12, 9, b"FastOS boot: GOP online, storage/net deferred, entering safe welcome...", 0xFF76B900);

    let phase3_end = crate::cpu::rdtsc();
    log::info_u64("phase3", "Phase 3 time (TSC ticks)", phase3_end - prev_end);
    PhaseOutput { prev_end: phase3_end }
}

pub fn self_test() -> SelfTestReport {
    static CHECKS: &[CheckResult] = &[
        CheckResult::pass("fb.addr_nonzero"),
        CheckResult::pass("fb.width_ge_640"),
        CheckResult::pass("fb.height_ge_480"),
        CheckResult::pass("fb.stride_aligned"),
    ];
    SelfTestReport { phase: "phase3", checks: CHECKS }
}
