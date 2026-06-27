//! Phase 1 — Ring 0 Main Coordinator
//!
//! This is the main entry point for Ring 0 initialization.
//! It orchestrates ALL hardware setup before handing off to the next phase.
//!
//! Boot order:
//!   1. Validate BootInfo
//!   2. Init UEFI Runtime Services (NVRAM)
//!   3. Phase 0: GDT + IDT + SYSCALL + CPU init
//!   4. Phase 1: Frame allocator + heap + high-mem
//!   5. Phase 2: ACPI + PCI discovery
//!   6. Phase 3: GOP framebuffer
//!   7. Phase 4: Scheduler init
//!   8. init_fastos_cpu (Ryzen 5 5600X detection)
//!   9. init_acpi (ACPI tables)
//!  10. SMP init (AP cores)
//!  11. Return to caller (next phase)

use crate::info;
use crate::context::BootContext;

// ── Crash marker ────────────────────────────────────────────────────────────

const CRASH_MARKER_ADDR: u64 = 0x9_0000;
const CRASH_MAGIC: u32 = 0x464F_5343; // "FOSC"

pub fn write_crash_marker(stage: u32) {
    unsafe {
        core::ptr::write_volatile(CRASH_MARKER_ADDR as *mut u32, CRASH_MAGIC);
        core::ptr::write_volatile((CRASH_MARKER_ADDR + 4) as *mut u32, stage);
    }
}

pub fn clear_crash_marker() {
    unsafe {
        core::ptr::write_volatile(CRASH_MARKER_ADDR as *mut u32, 0);
        core::ptr::write_volatile((CRASH_MARKER_ADDR + 4) as *mut u32, 0);
    }
}

// ── Boot validation ─────────────────────────────────────────────────────────

fn validate_boot_info(
    ptr: *const fastos_boot_protocol::BootInfo,
) -> Result<&'static fastos_boot_protocol::BootInfo, &'static str> {
    if ptr.is_null() {
        return Err("boot_info_ptr is NULL");
    }
    let bi = unsafe { &*ptr };
    if bi.magic != fastos_boot_protocol::BOOT_MAGIC {
        return Err("BootInfo magic mismatch");
    }
    Ok(bi)
}

unsafe fn store_boot_info(bi: &fastos_boot_protocol::BootInfo) {
    info::BOOT_INFO = bi as *const _;
    info::FB_ADDR = bi.fb_addr;
    info::FB_WIDTH = bi.fb_width;
    info::FB_HEIGHT = bi.fb_height;
    info::FB_STRIDE = bi.fb_stride;
    info::FB_PIXEL_FORMAT = bi.fb_pixel_format;
}

// ── Phase 0: CPU Init (GDT + IDT + SYSCALL + CPU features) ─────────────────

fn phase0_arch(ctx: &mut BootContext, boot_start: u64) -> u64 {
    crate::log::info("phase0", "=== Phase 0: CPU Init ===");

    // GDT
    write_crash_marker(200);
    crate::uefi_rt::write_boot_stage("p0_gdt");
    crate::arch::gdt::init_gdt();

    // IDT
    write_crash_marker(201);
    crate::uefi_rt::write_boot_stage("p0_idt");
    crate::arch::idt::init_idt();

    // FIRST safe watchdog pet — IDT loaded, MMIO faults can be caught
    crate::dev::watchdog::pet_fch_watchdog();

    // SYSCALL MSRs
    write_crash_marker(202);
    crate::uefi_rt::write_boot_stage("p0_syscall");
    crate::arch::syscall::init_syscall();

    // CPU features, CR0/CR4, XCR0, FPU, MTRR/PAT, TSC
    crate::dev::watchdog::pet_fch_watchdog();
    write_crash_marker(203);
    crate::uefi_rt::write_boot_stage("p0_cpu_init");

    let cpu = crate::cpu::init();
    crate::dev::watchdog::pet_fch_watchdog();
    write_crash_marker(204);
    crate::uefi_rt::write_boot_stage("p0_cpu_done");
    crate::log::info("phase0", "CPU modular init DONE");

    // Init BMO ABI clock
    crate::bmo_abi::values::time::init_clock(crate::cpu::rdtsc(), cpu.tsc_freq);

    // Persist state
    ctx.cpu.tsc_freq_hz = cpu.tsc_freq;
    ctx.cpu.vendor = *b"AuthenticAMD";
    ctx.cpu.features_sse = true;
    ctx.cpu.features_avx = true;
    ctx.cpu.features_avx2 = true;
    ctx.cpu.features_aes = true;
    ctx.bmo_abi_initialized = true;

    // Timer subsystem init (HPET detection + timer wheel + timestamps)
    crate::dev::timer::init();

    let phase0_end = crate::cpu::rdtsc();
    ctx.record_phase(0, boot_start, phase0_end);

    crate::log::info_u64("phase0", "TSC frequency (Hz)", cpu.tsc_freq);
    crate::log::info_u64("phase0", "Phase 0 time (TSC ticks)", phase0_end - boot_start);

    phase0_end
}

// ── Phase 1: Memory Init (frame allocator + heap + high-mem) ────────────────

fn phase1_mem(ctx: &mut BootContext, prev_end: u64) -> u64 {
    crate::log::info("phase1", "=== Phase 1: Memory Init ===");

    // Validate UEFI memory map
    let bi = match ctx.boot_info() {
        Some(bi) => bi,
        None => crate::log::fault("phase1", "BootInfo is null"),
    };
    if bi.memory_map_count == 0 {
        crate::log::fault("phase1", "UEFI memory map is empty");
    }

    // Init frame allocator from UEFI memory map
    unsafe {
        crate::mm::phys::init(
            &bi.memory_map,
            bi.memory_map_count as usize,
            bi.reserved_addr,
            bi.reserved_size,
            0, // kernel_base (not needed for basic boot)
            0, // kernel_size
        );
    }
    let free_pages = crate::mm::phys::free_count();
    let free_mb = (free_pages * 4096) / (1024 * 1024);
    crate::log::info_u64("phase1", "free pages", free_pages as u64);
    crate::log::info_u64("phase1", "free MB", free_mb as u64);

    // Map all physical RAM into high-mem region
    unsafe {
        crate::mm::virt::map_high_mem(&bi.memory_map, bi.memory_map_count as usize);
    }
    crate::log::info("phase1", "high-mem mapped");

    // Init kernel heap
    crate::mm::heap::init_heap();
    crate::log::info("phase1", "heap initialized");

    // Smoke test
    unsafe {
        let test = alloc::alloc::alloc(core::alloc::Layout::from_size_align(64, 8).unwrap());
        if !test.is_null() {
            core::ptr::write_bytes(test, 0xAA, 64);
            alloc::alloc::dealloc(test, core::alloc::Layout::from_size_align(64, 8).unwrap());
            crate::log::info("phase1", "heap smoke test PASSED");
        } else {
            crate::log::fault("phase1", "heap smoke test FAILED");
        }
    }

    // Persist state
    ctx.memory.free_pages = free_pages as u64;
    ctx.memory.free_mb = free_mb as u64;
    ctx.memory.heap_total_bytes = crate::mm::heap::heap_total() as u64;
    ctx.memory.heap_used_bytes = crate::mm::heap::heap_used() as u64;

    let phase1_end = crate::cpu::rdtsc();
    ctx.record_phase(1, prev_end, phase1_end);

    crate::log::info_u64("phase1", "Phase 1 time (TSC ticks)", phase1_end - prev_end);

    phase1_end
}

// ── Phase 2: Device Discovery (ACPI + PCI) ──────────────────────────────────

fn phase2_dev(ctx: &mut BootContext, prev_end: u64) -> u64 {
    crate::log::info("phase2", "=== Phase 2: Device Discovery ===");

    let bi = match ctx.boot_info() {
        Some(bi) => bi,
        None => crate::log::fault("phase2", "BootInfo is null"),
    };

    // Parse ACPI MCFG
    let rsdp_addr = bi.rsdp_addr;
    let mcfg = if rsdp_addr != 0 {
        crate::dev::acpi::parse_mcfg(rsdp_addr)
    } else {
        None
    };
    if let Some(ref m) = mcfg {
        crate::log::info_u64("phase2", "ECAM base", m.base);
        crate::dev::console::serial_write("[phase2] ECAM end_bus=");
        crate::dev::console::serial_write_u64(m.end_bus as u64, 10);
        crate::dev::console::serial_write("\n");
    } else {
        crate::log::warn("phase2", "ACPI MCFG not found; PCI scan skipped");
    }

    // PCI scan is intentionally skipped in Ring 0 stable mode. Raw legacy
    // 0xCF8/0xCFC probing and unverified ECAM MMIO can wedge/reboot real AMD
    // boards before diagnostics are visible. ACPI is parsed above; a later
    // vendor-specific driver can opt into mapped ECAM safely.
    crate::dev::pcie::init_ecam(0, 0);
    let scan = crate::dev::pcie::PciScanResult::empty();
    crate::log::info_u64("phase2", "PCI devices found", scan.count as u64);

    // AHCI detection
    if crate::dev::pcie::has_ahci() {
        crate::log::info("phase2", "AHCI controller detected");
        if let Some(mmio) = crate::dev::pcie::find_ahci_mmio() {
            crate::dev::console::serial_write("[phase2] AHCI MMIO=0x");
            crate::dev::console::serial_write(&alloc::format!("{:x}", mmio));
            crate::dev::console::serial_write("\n");
        }
    }

    // Persist state
    ctx.devices.acpi_mcfg_base = mcfg.as_ref().map(|m| m.base).unwrap_or(0);
    ctx.devices.acpi_mcfg_end_bus = mcfg.as_ref().map(|m| m.end_bus).unwrap_or(0);
    ctx.devices.ecam_mapped = false;
    ctx.devices.pci_devices_found = scan.count as u32;

    // PS/2 Keyboard (IRQ1)
    crate::dev::keyboard::init();

    // PS/2 Mouse (IRQ12)
    crate::dev::mouse::init();

    // USB HID (xHCI native keyboard/mouse)
    crate::dev::usb_hid::init();

    // Storage drivers (AHCI detection, NVMe detection)
    if crate::dev::pcie::has_ahci() {
        if let Some(mmio) = crate::dev::pcie::find_ahci_mmio() {
            unsafe { crate::dev::ahci::probe(mmio, 0); }
        }
    }

    // Power management (C-states, thermal monitoring)
    crate::dev::power::init();

    let phase2_end = crate::cpu::rdtsc();
    ctx.record_phase(2, prev_end, phase2_end);

    crate::log::info_u64("phase2", "Phase 2 time (TSC ticks)", phase2_end - prev_end);

    phase2_end
}

// ── Phase 3: Display Init (GOP framebuffer) ─────────────────────────────────

fn phase3_display(ctx: &mut BootContext, prev_end: u64) -> u64 {
    crate::log::info("phase3", "=== Phase 3: Display Init ===");

    let (fb_addr, fb_w, fb_h, fb_s) = unsafe {
        (crate::info::FB_ADDR, crate::info::FB_WIDTH, crate::info::FB_HEIGHT, crate::info::FB_STRIDE)
    };

    if fb_addr == 0 || fb_w == 0 || fb_h == 0 || fb_s == 0 {
        crate::log::fault("phase3", "Framebuffer parameters invalid");
    }

    crate::log::info_u64("phase3", "framebuffer addr", fb_addr);
    crate::log::info_u64("phase3", "resolution", (fb_w as u64) << 32 | fb_h as u64);

    // Init GOP display
    let fmt = unsafe { crate::info::FB_PIXEL_FORMAT };
    crate::dev::framebuffer::init_gop(fb_addr, fb_w, fb_h, fb_s, fmt);
    crate::log::info("phase3", "GOP display initialized");

    let phase3_end = crate::cpu::rdtsc();
    ctx.record_phase(3, prev_end, phase3_end);

    crate::log::info_u64("phase3", "Phase 3 time (TSC ticks)", phase3_end - prev_end);

    phase3_end
}

// ── Phase 4: Scheduler Init ─────────────────────────────────────────────────

fn phase4_sched(ctx: &mut BootContext, prev_end: u64) -> u64 {
    crate::log::info("phase4", "=== Phase 4: Scheduler Init ===");

    // Init process/task tables
    crate::proc::init();
    crate::log::info("phase4", "scheduler tables initialized");

    // NOTE: APIC timer, interrupts, watchdog DISABLED (cooperative mode)
    // Will be re-enabled when bmo_core is ready

    let phase4_end = crate::cpu::rdtsc();
    ctx.record_phase(4, prev_end, phase4_end);

    crate::log::info_u64("phase4", "Phase 4 time (TSC ticks)", phase4_end - prev_end);

    phase4_end
}

// ── Main Entry Point ────────────────────────────────────────────────────────

/// Main entry point for Ring 0. Called from kernel_main_real.
///
/// Initializes ALL hardware and returns a BootContext for the next phase.
/// This function DOES return — the caller (bmo_core) takes over after this.
pub fn main(boot_info_ptr: *const fastos_boot_protocol::BootInfo) -> BootContext {
    // 1. Validate BootInfo
    crate::cabina_0::info("ring0", "validating BootInfo");
    let bi = match validate_boot_info(boot_info_ptr) {
        Ok(bi) => bi,
        Err(msg) => crate::log::fault("ring0", msg),
    };
    crate::dev::console::serial_write("[ring0] boot_info validated\n");
    unsafe { store_boot_info(bi); }
    crate::cabina_0::info("ring0", "BootInfo stored");

    // 2. Init UEFI Runtime Services
    crate::uefi_rt::init(bi.uefi_system_table);
    crate::cabina_0::info("ring0", "UEFI RT initialized");

    // 3. Write crash marker + NVRAM
    write_crash_marker(0);
    let nvram_ok = crate::uefi_rt::write_boot_stage("kernel_start");
    crate::cabina_0::info("ring0", "NVRAM boot_stage=kernel_start");
    crate::dev::console::serial_write("[ring0] NVRAM kernel_start=");
    crate::dev::console::serial_write(if nvram_ok { "OK\n" } else { "FAIL\n" });

    // 4. Init visual (splash screen)
    crate::visual::clear();
    crate::visual::log("ring0", "Ring 0 init start", crate::visual::color::OK);

    // 5. Create BootContext
    let mut ctx = BootContext::new(boot_info_ptr);
    let boot_start = crate::cpu::rdtsc();

    // 6. Phase 0-4
    write_crash_marker(2);
    crate::uefi_rt::write_boot_stage("phases_0_to_4");
    crate::cabina_0::info("ring0", "starting phases 0-4");
    crate::visual::log("ring0", "[0/5] boot phases", crate::visual::color::OK);

    let mut prev_end = boot_start;
    crate::cabina_0::info("ring0", "entering phase0_arch");
    prev_end = phase0_arch(&mut ctx, prev_end);
    crate::cabina_0::info("ring0", "entering phase1_mem");
    prev_end = phase1_mem(&mut ctx, prev_end);
    crate::cabina_0::info("ring0", "entering phase2_dev");
    prev_end = phase2_dev(&mut ctx, prev_end);
    crate::cabina_0::info("ring0", "entering phase3_display");
    prev_end = phase3_display(&mut ctx, prev_end);
    crate::cabina_0::info("ring0", "entering phase4_sched");
    prev_end = phase4_sched(&mut ctx, prev_end);

    crate::visual::log("ring0", "[0/5] phases done", crate::visual::color::OK);
    crate::cabina_0::info("ring0", "all boot phases completed");

    // 7. CPU-specific init (Ryzen 5 5600X)
    write_crash_marker(3);
    crate::uefi_rt::write_boot_stage("init_fastos_cpu");
    crate::cabina_0::info("ring0", "detecting CPU");
    crate::visual::log("ring0", "[1/5] detect 5600X", crate::visual::color::OK);
    crate::vendor::amd::cpu::zen3::init_fastos_cpu();
    crate::cabina_0::info("ring0", "CPU detected");
    crate::visual::log("ring0", "[1/5] 5600X detected", crate::visual::color::OK);

    // 8. ACPI tables
    write_crash_marker(4);
    crate::uefi_rt::write_boot_stage("init_acpi");
    crate::cabina_0::info("ring0", "parsing ACPI tables");
    let rsdp_hint = if bi.rsdp_addr != 0 { Some(bi.rsdp_addr) } else { None };
    crate::visual::log("ring0", "[2/5] init ACPI", crate::visual::color::OK);
    crate::vendor::amd::cpu::zen3::init_acpi(rsdp_hint);
    crate::cabina_0::info("ring0", "ACPI initialized");

    // 9. SMP init
    write_crash_marker(45);
    crate::uefi_rt::write_boot_stage("smp_init");
    crate::cabina_0::info("ring0", "SMP init start");
    crate::visual::log("ring0", "[3.5/5] SMP init", crate::visual::color::OK);
    unsafe { crate::arch::smp::init(); }
    let smp_cores = crate::arch::smp::core_count();
    crate::cabina_0::info("ring0", "SMP init done");
    if smp_cores > 1 {
        crate::visual::log("ring0", "[3.5/5] SMP online", crate::visual::color::OK);
    } else {
        crate::visual::log("ring0", "[3.5/5] SMP single-core", crate::visual::color::WARN);
    }

    // 10. Mark boot complete
    write_crash_marker(5);
    crate::uefi_rt::write_boot_stage("ring0_complete");
    crate::cabina_0::info("ring0", "Ring 0 boot complete — returning to caller");
    crate::visual::log("ring0", "Ring 0 boot complete", crate::visual::color::OK);
    crate::dev::console::serial_write("[ring0] boot complete — returning to caller\n");

    // Dump all CABINA_0 events to serial for diagnostics
    crate::cabina_0::dump_serial();

    ctx
}
