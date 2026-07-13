//! Ring 0 boot phases — orchestrator for the kernel entry path.
//!
//! The legacy kernel had 4-5 phases plus SMP/ACPI orchestration via
//! `cpu_vendor_profile`. In Ultra_kernel's Ring 0 base we keep the
//! same Faggin-style phase structure but call only the local modules
//! (no external vendor crate).
//!
//! Phases:
//!   0. arch  — GDT, IDT, syscall MSRs, CPU init
//!   1. mem   — phys frame allocator
//!   2. dev   — framebuffer init, HPET, ACPI stub
//!   3. sched — single-CPU task table init
//!
//! After phases: splash completes, `clear`, and the kernel either
//! idles (single core) or jumps to a user shell.

use boot_context::BootContext;
use super::splash;

fn s_log(msg: &str) {
    crate::ring0::dev::console::serial_write(msg);
    crate::ring0::dev::console::serial_write("\n");
}

fn phase0_arch(_ctx: &BootContext) {
    s_log("[phase0] === CPU Init ===");
    crate::ring0::arch::gdt::init_gdt();
    crate::ring0::arch::idt::init_idt();
    crate::ring0::arch::syscall::init_syscall();
    let _cpu = crate::ring0::cpu::init();
    s_log("[phase0] done");
}

fn phase1_mem(ctx: &BootContext) {
    s_log("[phase1] === Memory Init ===");
    let entries = super::mm::types::from_ctx(&ctx.memory_map[..ctx.memory_map_count as usize]);
    let bi_phys = ctx as *const BootContext as u64;
    crate::ring0::mm::phys::init(&entries, bi_phys);
    crate::ring0::mm::vmm_stub::map_high_mem(&entries, ctx.memory_map_count as usize);
    crate::ring0::mm::heap_stub::init_heap();
    s_log("[phase1] done");
}

fn phase2_dev(ctx: &BootContext) {
    s_log("[phase2] === Device Init ===");
    let fmt = ctx.fb_pixel_format;
    crate::ring0::dev::framebuffer::init_gop(
        ctx.fb_addr,
        ctx.fb_width,
        ctx.fb_height,
        ctx.fb_stride,
        crate::ring0::dev::framebuffer::PixelFormat::Unknown, // legacy fmt enum re-mapped below
    );
    // The local `PixelFormat::Unknown` is just a placeholder; the real
    // boot path passes the value from `ctx.fb_pixel_format` (0=BGR, 1=RGB, 2=Unknown).
    crate::ring0::dev::framebuffer::init_gop(
        ctx.fb_addr,
        ctx.fb_width,
        ctx.fb_height,
        ctx.fb_stride,
        match fmt { 0 => crate::ring0::dev::framebuffer::PixelFormat::Bgr, 1 => crate::ring0::dev::framebuffer::PixelFormat::Rgb, _ => crate::ring0::dev::framebuffer::PixelFormat::Unknown },
    );
    crate::ring0::dev::timer::init();
    crate::ring0::dev::watchdog::arm(crate::ring0::cpu::rdtsc());
    s_log("[phase2] done");
}

fn phase3_sched(_ctx: &BootContext) {
    s_log("[phase3] === Scheduler Init ===");
    crate::ring0::proc::init();
    crate::ring0::irq::init();
    s_log("[phase3] done");
}

/// Public entry: called from `entry::kernel_main_real` after the
/// naked `_start` BSS zero.
pub fn main(ctx: &BootContext) {
    s_log("[ring0] validating BootContext");
    if !ctx.is_valid() {
        s_log("[ring0] FATAL: BootContext magic mismatch");
        loop { unsafe { core::arch::asm!("hlt"); } }
    }

    crate::ring0::dev::console::serial_write("[ring0] BootContext OK, version=");
    crate::ring0::dev::console::serial_write_u64(ctx.version as u64, 10);
    crate::ring0::dev::console::serial_write("\n");

    // Populate FB globals from the context.
    crate::info::init_from(ctx);

    // Show boot splash (if framebuffer available).
    if crate::info::has_fb() {
        splash::splash_init();
        splash::splash_progress(5, "Starting kernel...");
    } else {
        s_log("[splash] no framebuffer, skipping splash");
    }

    splash::splash_progress(15, "CPU, GDT, IDT...");
    phase0_arch(ctx);
    splash::splash_progress(35, "Memory allocators...");
    phase1_mem(ctx);
    splash::splash_progress(55, "Devices...");
    phase2_dev(ctx);
    splash::splash_progress(80, "Scheduler...");
    phase3_sched(ctx);
    splash::splash_progress(100, "BMO Ready.");
    splash::splash_clear();
    s_log("[ring0] boot complete");
    s_log("[ring0] BMO: Ok Ready");
}
