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
    s_log("  tasks        show scheduler state");
    s_log("  mem          frame allocator stats + vmm selftest");
    s_log("  ktest        spawn F1 context-switch demo task");
    s_log("  fb           show framebuffer info");
    s_log("  splash       re-run boot splash animation");
    s_log("  bex          show native executable-loader status");
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

fn shell_tasks() {
    let (total, runnable) = crate::ring0::scheduler::counts();
    crate::ring0::dev::console::serial_write("[tasks] total=");
    crate::ring0::dev::console::serial_write_u64(total as u64, 10);
    crate::ring0::dev::console::serial_write(" runnable=");
    crate::ring0::dev::console::serial_write_u64(runnable as u64, 10);
    crate::ring0::dev::console::serial_write(" current_tid=");
    crate::ring0::dev::console::serial_write_u64(
        crate::ring0::scheduler::current_tid() as u64,
        10,
    );
    crate::ring0::dev::console::serial_write(" ticks=");
    crate::ring0::dev::console::serial_write_u64(crate::ring0::timer::ticks(), 10);
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

fn shell_bex() {
    s_log("[bex] BEX v1 / BEF1 x86-64 admission is enabled");
    s_log("[bex] next: storage input, Ring 3 pages, then iretq entry");
}

/// F1 demo task: runs preempted by the timer, parks on a WAIT deadline,
/// wakes, and exits through the reaper. Watch the interleaving with the
/// shell prompt on serial — that interleaving IS the context switch.
extern "C" fn ktest_main(arg: u64) -> ! {
    use crate::ring0::dev::console::{serial_write, serial_write_u64};
    serial_write("[ktest] start tid=");
    serial_write_u64(crate::ring0::scheduler::current_tid() as u64, 10);
    serial_write(" arg=");
    serial_write_u64(arg, 10);
    serial_write("\n");
    for i in 0..3u64 {
        serial_write("[ktest] window ");
        serial_write_u64(i, 10);
        serial_write("\n");
        // Busy window ~250 ms so the timer preempts us several times and
        // the shell task runs in between (look for the '>' echoes).
        let start = crate::ring0::scheduler::rdtsc();
        let span = crate::ring0::scheduler::tsc_freq() / 4;
        while crate::ring0::scheduler::rdtsc().wrapping_sub(start) < span {
            core::hint::spin_loop();
        }
    }
    serial_write("[ktest] park 2000 ms (WAIT deadline)\n");
    let deadline = crate::ring0::scheduler::rdtsc()
        + crate::ring0::scheduler::ns_to_tsc(2_000_000_000);
    crate::ring0::scheduler::park_until(deadline);
    serial_write("[ktest] woke; exit via reaper\n");
    crate::ring0::scheduler::exit_and_park();
}

fn shell_ktest() {
    match crate::ring0::scheduler::spawn_kernel(ktest_main as usize as u64, 0xB0, 1) {
        Some(tid) => {
            crate::ring0::dev::console::serial_write("[ktest] spawned tid=");
            crate::ring0::dev::console::serial_write_u64(tid as u64, 10);
            crate::ring0::dev::console::serial_write("\n");
        }
        None => s_log("[ktest] spawn failed (no frames or task slots)"),
    }
}

fn shell_mem() {
    let (total, free) = crate::ring0::mm::phys::stats();
    crate::ring0::dev::console::serial_write("[mem] frames free=");
    crate::ring0::dev::console::serial_write_u64(free, 10);
    crate::ring0::dev::console::serial_write(" total=");
    crate::ring0::dev::console::serial_write_u64(total, 10);
    crate::ring0::dev::console::serial_write(" (");
    crate::ring0::dev::console::serial_write_u64(free * 4096 / (1024 * 1024), 10);
    crate::ring0::dev::console::serial_write(" MiB free)\n");
    dash_log("[mem] stats printed on serial");
    if crate::ring0::mm::vmm::self_test() {
        s_log("[mem] vmm selftest OK (alloc/map/translate/unmap/destroy)");
    } else {
        s_log("[mem] vmm selftest FAILED");
    }
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
        } else if cmd == b"tasks" {
            shell_tasks();
        } else if cmd == b"mem" {
            shell_mem();
        } else if cmd == b"ktest" {
            shell_ktest();
        } else if cmd == b"fb" {
            shell_fb();
        } else if cmd == b"splash" {
            shell_splash();
        } else if cmd == b"bex" {
            shell_bex();
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
pub fn main(ctx: &mut BootContext) {
    s_log("[ring0] validating BootContext");
    if !ctx.is_valid() {
        s_log("[ring0] FATAL: BootContext magic mismatch");
        loop { unsafe { core::arch::asm!("hlt"); } }
    }

    crate::ring0::dev::console::serial_write("[ring0] BootContext OK, version=");
    crate::ring0::dev::console::serial_write_u64(ctx.version as u64, 10);
    crate::ring0::dev::console::serial_write("\n");

    crate::ring0::percpu::init_bsp();
    crate::ring0::scheduler::init(ctx.tsc_freq);
    crate::ring0::mm::phys::init(ctx);
    crate::ring0::mm::vmm::init();
    let (frames_total, frames_free) = crate::ring0::mm::phys::stats();
    crate::ring0::dev::console::serial_write("[ring0] mm ready: frames free=");
    crate::ring0::dev::console::serial_write_u64(frames_free, 10);
    crate::ring0::dev::console::serial_write("/");
    crate::ring0::dev::console::serial_write_u64(frames_total, 10);
    crate::ring0::dev::console::serial_write("\n");
    crate::ring0::channel::init(ctx);
    crate::ring0::svc::register_all();
    crate::ring0::syscall::init();
    let timer_ready = crate::ring0::timer::init(ctx);
    if timer_ready {
        s_log("[ring0] scheduler + BMO Channel + SYSCALL + LAPIC tick ready (BSP)");
    } else {
        s_log("[ring0] WARNING: LAPIC tick unavailable; scheduler remains cooperative");
    }

    // Populate the active BMO CPU profile (today: Ryzen 5 5600X).
    // Identity, SMT/CCX topology, cache hierarchy, TSC calibration and
    // errata/speculation mitigations all live behind the profile —
    // changing CPU or vendor is a profile swap, never a kernel edit.
    let cpu_profile = crate::ring0::cpu_vendor::active();
    (cpu_profile.init)();
    crate::ring0::dev::console::serial_write("[cpu] profile: ");
    crate::ring0::dev::console::serial_write(cpu_profile.vendor);
    crate::ring0::dev::console::serial_write(" ");
    crate::ring0::dev::console::serial_write(cpu_profile.name);
    crate::ring0::dev::console::serial_write("\n");

    // BEX is the only native executable contract admitted by this kernel.
    // The parser is allocation-free so it is safe before the process allocator.
    crate::ring0::bex::announce();

    // F2: if the boot chain reserved a Ring 3 payload, admit it as the
    // init process. With no payload this is a no-op and the boot flow is
    // exactly the Ring 0 shell as before.
    crate::ring0::proc::init(ctx);
    if let Some(tid) = crate::ring0::proc::spawn_init(ctx) {
        crate::ring0::dev::console::serial_write("[ring0] Ring 3 init task ready, tid=");
        crate::ring0::dev::console::serial_write_u64(tid as u64, 10);
        crate::ring0::dev::console::serial_write("\n");
    }

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

    if timer_ready {
        crate::ring0::timer::enable();
    }

    run_shell(ctx);
}
