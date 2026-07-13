//! Ring 0 boot phases - orchestrator for the kernel entry path.
//!
//! In Ultra_kernel_x86-64's minimal Ring 0 base we keep only what's necessary:
//! the splash animation, the framebuffer init, and a serial shell.
//! All GDT/IDT/CPU/MM/IRQ/SMP subsystems live in the faggin stages
//! (s2_gdt, s3_idt, s4_cpuid, s5_control, s9_paging) and are already
//! configured by the time the kernel runs.
//!
//! Phases:
//!   0. fb    - framebuffer init from BootContext
//!   1. ui    - splash animation (if FB available)
//!
//! After phases: serial shell takes over so the user has a way to
//! interact even without a display.

use boot_context::BootContext;
use super::splash;

fn s_log(msg: &str) {
    crate::ring0::dev::console::serial_write(msg);
    crate::ring0::dev::console::serial_write("\n");
}

fn phase0_fb(ctx: &BootContext) {
    s_log("[phase0] === Framebuffer Init ===");
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
    s_log("[phase0] done");
}

fn phase1_ui(_ctx: &BootContext) {
    s_log("[phase1] === UI (splash) ===");
    if crate::info::has_fb() {
        splash::splash_progress(100, "BMO Ready.");
        splash::splash_clear();
    } else {
        s_log("[splash] no framebuffer, skipping");
    }
    s_log("[phase1] done");
}

// ---------------------------------------------------------------------------
// Serial shell
// ---------------------------------------------------------------------------

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
            Some(0x7f) | Some(0x08) => {
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

fn shell_fb() {
    if !crate::info::has_fb() {
        s_log("[fb] no framebuffer (headless boot)");
        return;
    }
    crate::ring0::dev::console::serial_write("[fb] base=0x");
    crate::ring0::dev::console::serial_write_u64(unsafe { crate::info::FB_ADDR }, 16);
    crate::ring0::dev::console::serial_write(" ");
    crate::ring0::dev::console::serial_write_u64(unsafe { crate::info::FB_WIDTH } as u64, 10);
    crate::ring0::dev::console::serial_write("x");
    crate::ring0::dev::console::serial_write_u64(unsafe { crate::info::FB_HEIGHT } as u64, 10);
    crate::ring0::dev::console::serial_write("x32 stride=");
    crate::ring0::dev::console::serial_write_u64(unsafe { crate::info::FB_STRIDE } as u64, 10);
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

fn shell_panic() -> ! {
    s_log("[shell] triggering test panic...");
    panic!("intentional panic from serial shell");
}

fn shell_reboot() -> ! {
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

    // CPU identity detection (CPUID leaf 0, 1, 0x80000002-04)
    let cpu = crate::ring0::cpu::detect_cpu();
    crate::ring0::dev::console::serial_write("[cpu] ");
    crate::ring0::dev::console::serial_write(cpu.brand.as_str());
    crate::ring0::dev::console::serial_write(" | ");
    crate::ring0::dev::console::serial_write(match cpu.vendor {
        crate::ring0::cpu::CpuVendor::Amd => "AMD",
        crate::ring0::cpu::CpuVendor::Intel => "Intel",
        crate::ring0::cpu::CpuVendor::Unknown => "Unknown",
    });
    crate::ring0::dev::console::serial_write(" | cores=");
    crate::ring0::dev::console::serial_write_u64_dec(cpu.logical_cores as u64);
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

    splash::splash_progress(15, "Framebuffer init...");
    phase0_fb(ctx);
    phase1_ui(ctx);
    s_log("[ring0] boot complete");
    s_log("[ring0] BMO: Ok Ready");

    run_shell(ctx);
}
