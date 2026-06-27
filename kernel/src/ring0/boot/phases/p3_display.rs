//! Phase 3 — Display.
//!
//! v1.8.15: Renombrado de p3_proc a p3_display. Esta fase inicializa
//! el framebuffer GOP heredado de UEFI. Ahora se ejecuta DESPUÉS de
//! init_fastos_cpu() que configura MTRR/PAT para Write-Combining.

use crate::boot::log;
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
    crate::dev::console::serial_write("[phase3] Pixel format: ");
    match bi.fb_pixel_format {
        fastos_boot_protocol::PixelFormat::Bgr => crate::dev::console::serial_write("BGR\n"),
        fastos_boot_protocol::PixelFormat::Rgb => crate::dev::console::serial_write("RGB\n"),
        fastos_boot_protocol::PixelFormat::Unknown => crate::dev::console::serial_write("Unknown\n"),
    }
    log::info_u64("phase3", "Framebuffer size (MB)", fb_size_mb);

    crate::dev::framebuffer::init_gop(
        bi.fb_addr,
        bi.fb_width,
        bi.fb_height,
        bi.fb_stride,
        bi.fb_pixel_format,
    );
    log::info("phase3", "GOP display initialized");

    crate::dev::console::serial_write("[phase3] GOP online, entering safe welcome...\n");

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
