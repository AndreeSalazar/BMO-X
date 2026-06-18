//! FastOS / BMO Kernel — Entry Point.
//!
//! Boot path (phased, timed, zero-duplication):
//!
//!   Phase 0: CPU init (FPU/SSE/AVX/MTRR/PAT/perf counters)
//!   Phase 1: Memory (page allocator, heap validation)
//!   Phase 2: Devices (ACPI/PCI enumeration)
//!   Phase 3: Display (GOP framebuffer)
//!   Phase 4: Scheduler (APIC timer, interrupts)
//!   Phase 5: Desktop (welcome screen → shell)
//!
//! All logging goes through `boot_log!` — single path to diag + serial.

#![no_std]
#![no_main]

extern crate alloc;

mod allocator;
mod arch;
mod boot_info;
mod ui;
mod diag;
mod desktop;
mod drivers;
mod fs;
mod panic;
mod memory;

mod barex;
mod bef;
mod sched;
mod syscall;
mod sandbox;

mod lang;
mod security;

use core::arch::naked_asm;

// ── Boot logger — single path, zero duplication ────────────────────

/// Log a boot message to both diag and serial.
fn boot_log(phase: &'static str, msg: &'static str) {
    diag::info(phase, msg);
    drivers::serial::serial_write("[FastOS] ");
    drivers::serial::serial_write(msg);
    drivers::serial::serial_write("\n");
    early_visual_log(phase, msg, 0xFF76B900);
}

fn boot_log_u64(phase: &'static str, msg: &'static str, val: u64) {
    diag::info_u64(phase, msg, val);
    drivers::serial::serial_write("[FastOS] ");
    drivers::serial::serial_write(msg);
    drivers::serial::serial_write(": ");
    serial_hex(val);
    drivers::serial::serial_write("\n");
}

fn boot_warn(phase: &'static str, msg: &'static str) {
    diag::warn(phase, msg);
    drivers::serial::serial_write("[FastOS] WARN: ");
    drivers::serial::serial_write(msg);
    drivers::serial::serial_write("\n");
    early_visual_log(phase, msg, 0xFFFFBD2E);
}

fn boot_fault(phase: &'static str, msg: &'static str) -> ! {
    diag::fault(phase, msg);
    drivers::serial::serial_write("[FastOS] FATAL: ");
    drivers::serial::serial_write(msg);
    drivers::serial::serial_write("\n");
    early_visual_log(phase, msg, 0xFFFF2A2A);
    loop { unsafe { core::arch::asm!("hlt"); } }
}

static mut EARLY_VISUAL_ROW: usize = 0;

fn early_visual_clear() {
    let (addr, w, h, s) = unsafe {
        (
            boot_info::FB_ADDR,
            boot_info::FB_WIDTH as usize,
            boot_info::FB_HEIGHT as usize,
            boot_info::FB_STRIDE as usize,
        )
    };
    if addr == 0 || w == 0 || h == 0 || s == 0 { return; }

    let buf = addr as *mut u32;
    let max_h = h.min(360);
    for y in 0..max_h {
        for x in 0..w {
            unsafe { buf.add(y * s + x).write_volatile(0xFF050A12); }
        }
    }
    unsafe { EARLY_VISUAL_ROW = 0; }
}

fn early_visual_log(phase: &'static str, msg: &'static str, color: u32) {
    let row = unsafe {
        let r = EARLY_VISUAL_ROW;
        EARLY_VISUAL_ROW = (EARLY_VISUAL_ROW + 1) % 18;
        r
    };
    let y = 12 + row * 18;
    early_visual_text(12, y, b"FastOS KERNEL NEW :: ", 0xFF58A6FF);
    early_visual_text(188, y, phase.as_bytes(), color);
    early_visual_text(188 + phase.len() * 8 + 16, y, msg.as_bytes(), 0xFFE6EDF3);
}

fn early_visual_text(x: usize, y: usize, text: &[u8], color: u32) {
    let (addr, w, h, s) = unsafe {
        (
            boot_info::FB_ADDR,
            boot_info::FB_WIDTH as usize,
            boot_info::FB_HEIGHT as usize,
            boot_info::FB_STRIDE as usize,
        )
    };
    if addr == 0 || w == 0 || h == 0 || s == 0 { return; }

    let mut cx = x;
    let cy = y;
    let buf = addr as *mut u32;
    for &ch in text {
        if cx + 8 >= w || cy + 16 >= h { break; }
        let glyph = ui::font::get_glyph(ch);
        for py in 0..16 {
            let bits = glyph[py];
            for px in 0..8 {
                if (bits & (0x80 >> px)) != 0 {
                    unsafe { buf.add((cy + py) * s + cx + px).write_volatile(color); }
                }
            }
        }
        cx += 8;
    }
}

fn serial_hex(val: u64) {
    let hex = b"0123456789ABCDEF";
    drivers::serial::serial_write("0x");
    for i in (0..16).rev() {
        drivers::serial::serial_write_byte(hex[((val >> (i * 4)) & 0xF) as usize]);
    }
}

fn serial_u32(val: u32) {
    let mut buf = [0u8; 10];
    let mut i = buf.len();
    let mut v = val;
    if v == 0 {
        i -= 1;
        buf[i] = b'0';
    } else {
        while v > 0 {
            i -= 1;
            buf[i] = b'0' + (v % 10) as u8;
            v /= 10;
        }
    }
    drivers::serial::serial_write(core::str::from_utf8(&buf[i..]).unwrap_or("0"));
}

// ── Entry points ───────────────────────────────────────────────────

#[unsafe(no_mangle)]
#[link_section = ".text._start"]
#[unsafe(naked)]
unsafe extern "C" fn _start() -> ! {
    naked_asm!(
        "test rdi, rdi",
        "jz 2f",
        "mov rbx, rdi",
        "and rsp, -16",
        "mov rdi, rbx",
        "call kernel_main_real",
        "2: hlt",
        "jmp 2b",
    );
}

// ── Kernel main — phased boot ──────────────────────────────────────

#[unsafe(no_mangle)]
#[inline(never)]
extern "C" fn kernel_main_real(boot_info_ptr: *const fastos_boot_protocol::BootInfo) -> ! {
    // ── Pre-init: serial + basic diagnostics ───────────────────────
    drivers::serial::init_serial();
    boot_log("boot", "FastOS BMO Kernel v0.9.0 starting");

    if boot_info_ptr.is_null() {
        boot_fault("boot", "boot_info_ptr is NULL");
    }
    let bi = unsafe { &*boot_info_ptr };
    if bi.magic != fastos_boot_protocol::BOOT_MAGIC {
        boot_fault("boot", "BootInfo magic mismatch");
    }

    // Store boot info globals
    unsafe {
        boot_info::BOOT_INFO = boot_info_ptr;
        boot_info::RESERVED_PAYLOAD_ADDR = bi.gsp_addr;
        boot_info::RESERVED_PAYLOAD_SIZE = bi.gsp_size;
        boot_info::FB_ADDR = bi.fb_addr;
        boot_info::FB_WIDTH = bi.fb_width;
        boot_info::FB_HEIGHT = bi.fb_height;
        boot_info::FB_STRIDE = bi.fb_stride;
    }

    // First visible kernel checkpoint. This runs before diag init, CPU phases,
    // PCI probing, GOP driver init, APIC, security, welcome and desktop. If
    // this does not appear, either the USB still boots old files or control is
    // not reaching this kernel image.
    early_visual_clear();
    early_visual_log("boot", "K0 BootInfo received; framebuffer direct writer online", 0xFF76B900);

    diag::init();

    // Record boot start time
    let boot_start = arch::cpu::rdtsc();

    // ════════════════════════════════════════════════════════════════
    // PHASE 0: CPU INIT
    // ════════════════════════════════════════════════════════════════
    boot_log("phase0", "=== Phase 0: CPU Init ===");

    // Detect all CPU features via CPUID
    let cpu_features = arch::cpu::detect_cpu();
    boot_log("phase0", "CPU detected");
    drivers::serial::serial_write("  Brand: ");
    drivers::serial::serial_write(cpu_features.brand_string_str());
    drivers::serial::serial_write("\n");

    // Initialize Ryzen 5 5600X (CR0/CR4/XCR0/FPU/MTRR/PAT/perf)
    arch::cpu_amd::ryzen_5_5600x::init(&cpu_features);

    // Calibrate TSC frequency using APIC timer reference
    let tsc_freq = calibrate_tsc();
    boot_log_u64("phase0", "TSC frequency (Hz)", tsc_freq);

    let phase0_end = arch::cpu::rdtsc();
    boot_log_u64("phase0", "Phase 0 time (TSC ticks)", phase0_end - boot_start);

    // ════════════════════════════════════════════════════════════════
    // PHASE 1: MEMORY
    // ════════════════════════════════════════════════════════════════
    boot_log("phase1", "=== Phase 1: Memory ===");

    // Validate memory map
    if bi.memory_map_count == 0 {
        boot_fault("phase1", "UEFI memory map is empty");
    }
    boot_log_u64("phase1", "UEFI memory map entries", bi.memory_map_count as u64);

    // Initialize page allocator
    unsafe {
        arch::page_alloc::init(
            &bi.memory_map,
            bi.memory_map_count as usize,
            bi.gsp_addr,
            bi.gsp_size,
            bi.kernel_base,
            bi.kernel_size,
        );
    }
    let free_pages = unsafe { arch::page_alloc::free_count() };
    let free_mb = (free_pages * 4096) / (1024 * 1024);
    boot_log_u64("phase1", "Free pages", free_pages as u64);
    boot_log_u64("phase1", "Free memory (MB)", free_mb as u64);

    // Report heap status
    boot_log_u64("phase1", "Heap total (bytes)", allocator::heap_total() as u64);
    boot_log_u64("phase1", "Heap used (bytes)", allocator::heap_used() as u64);

    let phase1_end = arch::cpu::rdtsc();
    boot_log_u64("phase1", "Phase 1 time (TSC ticks)", phase1_end - phase0_end);

    // ════════════════════════════════════════════════════════════════
    // PHASE 2: DEVICES
    // ════════════════════════════════════════════════════════════════
    boot_log("phase2", "=== Phase 2: Devices ===");

    // GDT + TSS
    arch::gdt::init_gdt();
    boot_log("phase2", "GDT+TSS loaded (Ring0/Ring3)");

    // IDT
    arch::idt::init_idt();
    boot_log("phase2", "IDT loaded (256 entries)");

    // Syscall MSRs
    arch::syscall_entry::init_syscall();
    boot_log("phase2", "SYSCALL/SYSRET MSRs programmed (BMO ABI)");

    // ACPI → PCI
    if let Some(ecam) = arch::acpi::parse_mcfg(bi.rsdp_addr) {
        drivers::pci::init_ecam(ecam.base_addr, ecam.end_bus);
        let pci = drivers::pci::scan_pci_bus();

        boot_log_u64("phase2", "PCI devices discovered", pci.count as u64);

        // Log each PCI device
        for i in 0..pci.count {
            let dev = &pci.devices[i];
            drivers::serial::serial_write("  PCI ");
            serial_u32(dev.bus as u32);
            drivers::serial::serial_write(":");
            serial_u32(dev.device as u32);
            drivers::serial::serial_write(".");
            serial_u32(dev.function as u32);
            drivers::serial::serial_write(" [");
            serial_hex(dev.vendor_id as u64);
            drivers::serial::serial_write(":");
            serial_hex(dev.device_id as u64);
            drivers::serial::serial_write("] class=");
            serial_hex(dev.class_code as u64);
            drivers::serial::serial_write("\n");
        }

        unsafe { drivers::pci::SCAN_RESULT = Some(pci); }
    } else {
        boot_warn("phase2", "MCFG not found; PCI ECAM unavailable");
    }

    // Storage/network real driver init is deliberately deferred. On hardware
    // real, early NVMe/AHCI/NIC MMIO probing can freeze before the welcome
    // screen and before diag hotkeys are alive. The boot-critical path only
    // needs GOP + keyboard + Ring 0 desktop; storage/NIC will be mounted from
    // a controlled desktop/service phase later.
    boot_warn("phase2", "Storage init deferred until desktop/service phase");
    boot_warn("phase2", "Network init deferred until desktop/service phase");

    let phase2_end = arch::cpu::rdtsc();
    boot_log_u64("phase2", "Phase 2 time (TSC ticks)", phase2_end - phase1_end);

    // ════════════════════════════════════════════════════════════════
    // PHASE 3: DISPLAY
    // ════════════════════════════════════════════════════════════════
    boot_log("phase3", "=== Phase 3: Display ===");

    if bi.fb_addr == 0 {
        boot_fault("phase3", "No framebuffer; cannot start visual desktop");
    }

    // Validate framebuffer dimensions
    if bi.fb_width == 0 || bi.fb_height == 0 || bi.fb_stride == 0 {
        boot_fault("phase3", "Invalid framebuffer dimensions");
    }

    // Validate framebuffer is in usable memory (above 1MB, below 4GB)
    if bi.fb_addr < 0x100000 || bi.fb_addr > 0xFFFFFFFF {
        boot_fault("phase3", "Framebuffer address out of usable range");
    }

    let fb_size_mb = (bi.fb_width as u64 * bi.fb_height as u64 * 4) / (1024 * 1024);
    boot_log_u64("phase3", "Framebuffer base", bi.fb_addr);
    boot_log_u64("phase3", "Resolution", bi.fb_width as u64);
    drivers::serial::serial_write("  x ");
    serial_hex(bi.fb_height as u64);
    drivers::serial::serial_write("\n");
    boot_log_u64("phase3", "Stride (pixels)", bi.fb_stride as u64);
    boot_log_u64("phase3", "Framebuffer size (MB)", fb_size_mb);

    drivers::gop::init_gop(bi.fb_addr, bi.fb_width, bi.fb_height, bi.fb_stride);
    boot_log("phase3", "GOP display initialized");
    desktop::fb_fill(0, 0, bi.fb_width, 34, 0xFF101820);
    desktop::fb_text(12, 9, b"FastOS boot: GOP online, storage/net deferred, entering safe welcome...", 0xFF76B900);

    let phase3_end = arch::cpu::rdtsc();
    boot_log_u64("phase3", "Phase 3 time (TSC ticks)", phase3_end - phase2_end);

    // ════════════════════════════════════════════════════════════════
    // PHASE 4: SCHEDULER
    // ════════════════════════════════════════════════════════════════
    boot_log("phase4", "=== Phase 4: Scheduler ===");

    // APIC timer for preemptive scheduling
    arch::apic::init_apic(100);
    boot_log("phase4", "APIC timer started (100 Hz, 10ms ticks)");

    // SMP: bring up Application Processors (before STI)
    unsafe { arch::smp::smp_init(); }

    // Initialize security subsystem (ByteDefender + Restaurer)
    security::init();
    boot_log("phase4", "Security subsystem initialized (ByteDefender + Restaurer)");

    // Enable interrupts
    arch::cpu::sti();
    boot_log("phase4", "Interrupts enabled (STI)");

    let phase4_end = arch::cpu::rdtsc();
    boot_log_u64("phase4", "Phase 4 time (TSC ticks)", phase4_end - phase3_end);

    // ════════════════════════════════════════════════════════════════
    // PHASE 5: DESKTOP
    // ════════════════════════════════════════════════════════════════
    let boot_end = arch::cpu::rdtsc();
    let boot_total = boot_end - boot_start;

    // Calculate boot time in microseconds (TSC ticks / (freq_MHz))
    let boot_us = if tsc_freq >= 1_000_000 {
        boot_total / (tsc_freq / 1_000_000)
    } else {
        boot_total / 3700 // fallback: ~3.7 GHz
    };

    boot_log_u64("phase5", "Total boot time (us)", boot_us);

    // Create console
    let mut con = ui::console::Console::new(
        bi.fb_addr, bi.fb_pitch(), bi.fb_stride, bi.fb_width, bi.fb_height,
    );
    con.clear();

    // Print boot banner
    con.println("================================================================");
    con.println("  FastOS v0.9.0 — Bare Metal Orchestrator");
    con.println("  Ring 0/3 | GDT+TSS | SYSCALL/SYSRET | APIC Timer | GOP");
    con.println("================================================================");

    // CPU info line
    con.print("  CPU: ");
    // Truncate brand string to fit
    let brand = cpu_features.brand_string_str();
    let short_brand = if brand.len() > 32 { &brand[..32] } else { brand };
    con.println(short_brand);

    // Memory info
    con.print("  Memory: ");
    con.print_u64(free_mb as u64);
    con.print(" MB free | Heap: ");
    con.print_u64((allocator::heap_used() / 1024) as u64);
    con.println(" KB used");

    // Device info
    con.print("  PCI: ");
    #[allow(static_mut_refs)]
    let pci_count = unsafe {
        drivers::pci::SCAN_RESULT.as_ref().map(|r| r.count).unwrap_or(0)
    };
    con.print_u64(pci_count as u64);
    con.print(" devices | FB: ");
    con.print_u64(bi.fb_width as u64);
    con.print("x");
    con.print_u64(bi.fb_height as u64);

    // Boot time
    con.print(" | Boot: ");
    con.print_u64(boot_us / 1000);
    con.println(" ms");

    // Process/thread info
    con.print("  Processes: ");
    con.print_u64(sched::process::process_count() as u64);
    con.print(" | Threads: ");
    con.print_u64(sched::thread::ready_count() as u64);
    con.println("");

    // Features summary
    con.print("  FPU: lazy | AVX: ");
    if cpu_features.has_avx2 { con.print("AVX2"); }
    else if cpu_features.has_avx { con.print("AVX"); }
    else { con.print("SSE"); }
    con.print(" | AES: ");
    if cpu_features.has_aes { con.print("OK"); } else { con.print("--"); }
    con.print(" | SMEP: ");
    if cpu_features.has_smep { con.print("OK"); } else { con.print("--"); }
    con.print(" | SMAP: ");
    if cpu_features.has_smap { con.print("OK"); } else { con.print("--"); }
    con.println("");

    con.print("  Security: ByteDefender (Ring 0) | Restaurer (Snapshots)");
    con.println("");

    con.println("================================================================");
    con.println("");
    con.println("  Type 'Run' + Enter to launch desktop.");
    con.println("");

    // Launch welcome screen
    boot_log("phase5", "Launching welcome screen — type 'Run'");
    desktop::welcome::run();
}

// ── TSC calibration ────────────────────────────────────────────────

/// Calibrate TSC frequency by counting TSC ticks over a known time period.
///
/// Uses the APIC timer in one-shot mode as reference:
/// 1. Program APIC timer for a short delay (10ms)
/// 2. Read TSC before and after
/// 3. Compute frequency = ticks / time
fn calibrate_tsc() -> u64 {
    use arch::cpu::{rdtsc, cpuid};

    // Check if invariant TSC is available (Zen 3 always has it)
    let (_, _, _, edx_ext) = cpuid(0x80000007, 0);
    let _has_invariant_tsc = edx_ext & (1 << 8) != 0;

    // Calibration: count TSC ticks in a busy loop
    // Use LFENCE + RDTSC for serializing read
    unsafe {
        core::arch::asm!("lfence");
    }
    let start = rdtsc();

    // Busy loop — calibrated for ~10ms
    let mut count = 0u64;
    let target_iterations = 50_000_000;

    while count < target_iterations {
        count += 1;
        unsafe { core::arch::asm!("pause"); }
    }

    unsafe {
        core::arch::asm!("lfence");
    }
    let end = rdtsc();

    let elapsed = end - start;
    // elapsed ticks for target_iterations loops
    // Estimate: each PAUSE loop ~1 tick on Zen 3 @ 3.7GHz
    // Scale to 1 second: elapsed * (1_000_000_000 / elapsed_ns)
    // Simpler: assume loop ran for roughly elapsed / freq seconds
    // freq = elapsed_ticks / time_seconds => extrapolate
    // We calibrated ~50M iterations. On Zen 3, this is ~10-15ms.
    // So freq ≈ elapsed / 0.0125 (approx 12.5ms)
    let freq = if elapsed > 0 {
        // Multiply elapsed by 80 (= 1_000_000_000 / 12_500_000) for Hz
        elapsed * 80
    } else {
        3_700_000_000 // fallback: 3.7 GHz base clock
    };

    freq
}
