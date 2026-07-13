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
    // Mirror to the on-screen log panel (if framebuffer present).
    if crate::info::has_fb() {
        let row = unsafe { DASH_LOG_ROW };
        unsafe { DASH_LOG_ROW = (row + 1) % 14; }
        splash::splash_dashboard_log(row, msg);
    }
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
        // Switch from the boot splash to the persistent dashboard
        // (so something stays on screen for the user to read).
        splash::splash_dashboard_init();
    } else {
        s_log("[splash] no framebuffer, skipping");
    }
    s_log("[phase1] done");
}

// ---------------------------------------------------------------------------
// Serial shell (with optional framebuffer echo)
// ---------------------------------------------------------------------------

// Rolling index into the dashboard log. Each `dash_log` call
// advances this and wraps at DASH_LOG_LINES.
static mut DASH_LOG_ROW: usize = 0;

// Mirror the serial output to a line in the dashboard's log
// area, so the user can see what the kernel is doing without a
// serial terminal attached.
fn dash_log(msg: &str) {
    if !crate::info::has_fb() { return; }
    let row = unsafe { DASH_LOG_ROW };
    unsafe { DASH_LOG_ROW = (row + 1) % 14; }
    splash::splash_dashboard_log(row, msg);
}

// Mirror the current in-progress shell line to the framebuffer's
// prompt area. Called every time the user presses a key.
fn dash_prompt(line: &str) {
    if !crate::info::has_fb() { return; }
    splash::splash_dashboard_prompt(line);
}

fn shell_prompt() {
    crate::ring0::dev::console::serial_write("> ");
    dash_prompt("");
}

fn shell_read_line(buf: &mut [u8]) -> usize {
    let mut n = 0;
    loop {
        // Update the framebuffer's prompt with the current line
        // (so the screen shows what the user is typing).
        dash_prompt(core::str::from_utf8(&buf[..n]).unwrap_or(""));
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
                if n < buf.len() {
                    buf[n] = c;
                    n += 1;
                    crate::ring0::dev::console::serial_write_byte(c);
                }
            }
            _ => {}
        }
    }
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
    // Return to the persistent dashboard instead of clearing to black.
    splash::splash_dashboard_init();
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

    // Populate the BMO CPU profile (Ryzen 5 5600X topology + errata).
    // This detects CPUID, SMT/CCX layout, cache hierarchy, TSC freq,
    // and applies Zen 3 Spectre/MDS mitigations.
    crate::ring0::cpu_vendor::ryzen_5_5600x::init_bmo_cpu();

    // CPU identity detection (CPUID leaf 0, 1, 0x80000002-04)
    let cpu = crate::ring0::cpu::detect_cpu();
    let cpu_line = match cpu.vendor {
        crate::ring0::cpu::CpuVendor::Amd => "AMD",
        crate::ring0::cpu::CpuVendor::Intel => "Intel",
        crate::ring0::cpu::CpuVendor::Unknown => "Unknown",
    };
    let brand = cpu.brand.as_str();
    // Use a stack buffer to build the log line, then emit to both
    // serial and the framebuffer dashboard.
    let mut line = [0u8; 96];
    let prefix = b"[cpu] ";
    let mid1   = b" | ";
    let mid2   = b" | cores=";
    let mut off = 0;
    for &b in prefix { line[off] = b; off += 1; }
    for &b in brand.as_bytes() { if off < line.len() { line[off] = b; off += 1; } }
    for &b in mid1 { if off < line.len() { line[off] = b; off += 1; } }
    for &b in cpu_line.as_bytes() { if off < line.len() { line[off] = b; off += 1; } }
    for &b in mid2 { if off < line.len() { line[off] = b; off += 1; } }
    if off < line.len() { line[off] = b'0' + (cpu.logical_cores as u8 / 10); off += 1; }
    if off < line.len() { line[off] = b'0' + (cpu.logical_cores as u8 % 10); off += 1; }
    if let Ok(s) = core::str::from_utf8(&line[..off]) {
        s_log(s);
    }

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
