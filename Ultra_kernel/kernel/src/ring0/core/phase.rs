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
//! After phases: splash completes, `clear`, and a serial shell takes
//! over so the user has a way to interact even without a display.

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
    let fmt = match ctx.fb_pixel_format {
        0 => crate::ring0::dev::framebuffer::PixelFormat::Bgr,
        1 => crate::ring0::dev::framebuffer::PixelFormat::Rgb,
        _ => crate::ring0::dev::framebuffer::PixelFormat::Unknown,
    };
    crate::ring0::dev::framebuffer::init_gop(
        ctx.fb_addr,
        ctx.fb_width,
        ctx.fb_height,
        ctx.fb_stride,
        fmt,
    );
    crate::ring0::dev::timer::init();
    crate::ring0::dev::watchdog::arm();
    s_log("[phase2] done");
}

fn phase3_sched(_ctx: &BootContext) {
    s_log("[phase3] === Scheduler Init ===");
    crate::ring0::proc::init();
    crate::ring0::irq::init();
    s_log("[phase3] done");
}

// ── Serial shell ──────────────────────────────────────────────────
//
// After all phases, the kernel runs an interactive shell over COM1.
// The user can inspect BootContext, re-run the splash, trigger a
// panic, reboot, or just halt. This is the base layer of "user
// interaction" before Ring 3 exists.

fn shell_prompt() {
    crate::ring0::dev::console::serial_write("> ");
}

fn shell_read_line(buf: &mut [u8]) -> usize {
    let mut n = 0;
    while n < buf.len() {
        match crate::ring0::dev::console::serial_read_byte() {
            Some(b'\r') | Some(b'\n') => {
                crate::ring0::dev::console::serial_write("\n");
                return n;
            }
            Some(0x7f) | Some(b'\b') => {
                if n > 0 {
                    n -= 1;
                    crate::ring0::dev::console::serial_write("\x08 \x08");
                }
            }
            Some(c) if c >= 0x20 && c < 0x7f => {
                buf[n] = c;
                n += 1;
                crate::ring0::dev::console::serial_write_byte(c);
            }
            _ => {}
        }
    }
    n
}

fn shell_help() {
    s_log("commands:");
    s_log("  help         show this help");
    s_log("  info         dump BootContext fields");
    s_log("  regs         show GDT/IDT/TSS/syscall pointers");
    s_log("  fb           show framebuffer info");
    s_log("  splash       re-run boot splash animation");
    s_log("  panic        trigger test panic (does not return)");
    s_log("  reboot       keyboard reset pulse");
    s_log("  halt         stop and hlt");
}

fn shell_info(ctx: &BootContext) {
    s_log("--- BootContext ---");
    s_log("magic          = FOSCBOOT");
    crate::ring0::dev::console::serial_write("version         = ");
    crate::ring0::dev::console::serial_write_u64(ctx.version as u64, 10);
    crate::ring0::dev::console::serial_write("\n");
    crate::ring0::dev::console::serial_write("fb_addr         = 0x");
    crate::ring0::dev::console::serial_write_u64(ctx.fb_addr, 16);
    crate::ring0::dev::console::serial_write("\n");
    crate::ring0::dev::console::serial_write("fb_w x fb_h     = ");
    crate::ring0::dev::console::serial_write_u64(ctx.fb_width as u64, 10);
    crate::ring0::dev::console::serial_write(" x ");
    crate::ring0::dev::console::serial_write_u64(ctx.fb_height as u64, 10);
    crate::ring0::dev::console::serial_write("\n");
    crate::ring0::dev::console::serial_write("mem map entries = ");
    crate::ring0::dev::console::serial_write_u64(ctx.memory_map_count as u64, 10);
    crate::ring0::dev::console::serial_write("\n");
    crate::ring0::dev::console::serial_write("pml4            = 0x");
    crate::ring0::dev::console::serial_write_u64(ctx.pml4, 16);
    crate::ring0::dev::console::serial_write("\n");
    crate::ring0::dev::console::serial_write("tsc_freq        = ");
    crate::ring0::dev::console::serial_write_u64_dec(ctx.tsc_freq);
    crate::ring0::dev::console::serial_write(" Hz\n");
    crate::ring0::dev::console::serial_write("rsdp            = 0x");
    crate::ring0::dev::console::serial_write_u64(ctx.rsdp, 16);
    crate::ring0::dev::console::serial_write("\n");
}

fn shell_regs() {
    s_log("--- Ring 0 globals ---");
    crate::ring0::dev::console::serial_write("FB_ADDR         = 0x");
    crate::ring0::dev::console::serial_write_u64(crate::info::FB_ADDR, 16);
    crate::ring0::dev::console::serial_write(" (framebuffer physical)\n");
    crate::ring0::dev::console::serial_write("FB_WIDTH        = ");
    crate::ring0::dev::console::serial_write_u64(crate::info::FB_WIDTH as u64, 10);
    crate::ring0::dev::console::serial_write("\n");
    crate::ring0::dev::console::serial_write("FB_HEIGHT       = ");
    crate::ring0::dev::console::serial_write_u64(crate::info::FB_HEIGHT as u64, 10);
    crate::ring0::dev::console::serial_write("\n");
    crate::ring0::dev::console::serial_write("FB_STRIDE       = ");
    crate::ring0::dev::console::serial_write_u64(crate::info::FB_STRIDE as u64, 10);
    crate::ring0::dev::console::serial_write("\n");
}

fn shell_fb() {
    if !crate::info::has_fb() {
        s_log("[fb] no framebuffer (headless boot)");
        return;
    }
    crate::ring0::dev::console::serial_write("[fb] base=0x");
    crate::ring0::dev::console::serial_write_u64(crate::info::FB_ADDR, 16);
    crate::ring0::dev::console::serial_write(" ");
    crate::ring0::dev::console::serial_write_u64(crate::info::FB_WIDTH as u64, 10);
    crate::ring0::dev::console::serial_write("x");
    crate::ring0::dev::console::serial_write_u64(crate::info::FB_HEIGHT as u64, 10);
    crate::ring0::dev::console::serial_write("x32 stride=");
    crate::ring0::dev::console::serial_write_u64(crate::info::FB_STRIDE as u64, 10);
    crate::ring0::dev::console::serial_write("\n");
}

fn shell_splash() {
    if !crate::info::has_fb() {
        s_log("[splash] no framebuffer");
        return;
    }
    splash::splash_init();
    splash::splash_progress(50, "Shell re-triggered splash");
    splash::splash_clear();
    s_log("[splash] done");
}

fn shell_panic() {
    s_log("[shell] triggering test panic...");
    panic!("intentional panic from serial shell");
}

fn shell_reboot() {
    s_log("[shell] reboot (keyboard reset pulse)");
    unsafe { core::arch::asm!("out 0x64, al", in("al") 0xFEu8); }
    loop { unsafe { core::arch::asm!("hlt"); } }
}

fn shell_halt() -> ! {
    s_log("[shell] halting");
    loop { unsafe { core::arch::asm!("sti; hlt"); } }
}

fn run_shell(ctx: &BootContext) -> ! {
    s_log("");
    s_log("=== BMO v2.0 Ring 0 shell (type 'help') ===");
    shell_prompt();

    let mut buf = [0u8; 64];
    loop {
        let n = shell_read_line(&mut buf);
        if n == 0 { shell_prompt(); continue; }

        let cmd = &buf[..n];

        if cmd == b"help" {
            shell_help();
        } else if cmd == b"info" {
            shell_info(ctx);
        } else if cmd == b"regs" {
            shell_regs();
        } else if cmd == b"fb" {
            shell_fb();
        } else if cmd == b"splash" {
            shell_splash();
        } else if cmd == b"panic" {
            shell_panic();
        } else if cmd == b"reboot" {
            shell_reboot();
        } else if cmd == b"halt" {
            shell_halt();
        } else {
            s_log("unknown command (try 'help')");
        }
        shell_prompt();
    }
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

    run_shell(ctx);
}
