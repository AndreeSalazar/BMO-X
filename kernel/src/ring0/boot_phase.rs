//! Ring 0 Main Coordinator — pure Ring 0 boot phases.
//!
//! Orchestrates hardware setup: arch, memory, devices, display, scheduler.
//! No Ring 3 services (cabina, AHCI, XHCI, input, audio, visual).

use crate::info;
use crate::context::BootContext;

const CRASH_MARKER_ADDR: u64 = 0x9_0000;
const RAM_STAGE_ADDR: u64 = 0x9_0010;
const CRASH_MAGIC: u32 = 0x464F_5343;

pub fn write_crash_marker(stage: u32) {
    unsafe {
        core::ptr::write_volatile(CRASH_MARKER_ADDR as *mut u32, CRASH_MAGIC);
        core::ptr::write_volatile((CRASH_MARKER_ADDR + 4) as *mut u32, stage);
        core::ptr::write_volatile(RAM_STAGE_ADDR as *mut u32, stage);
    }
}

pub fn clear_crash_marker() {
    unsafe {
        core::ptr::write_volatile(CRASH_MARKER_ADDR as *mut u32, 0);
        core::ptr::write_volatile((CRASH_MARKER_ADDR + 4) as *mut u32, 0);
    }
}

fn s_log(msg: &str) {
    crate::dev::console::serial_write(msg);
    crate::dev::console::serial_write("\n");
}

fn validate_boot_info(
    ptr: *const bmo_boot_protocol::BootInfo,
) -> Result<&'static bmo_boot_protocol::BootInfo, &'static str> {
    if ptr.is_null() { return Err("boot_info_ptr is NULL"); }
    let bi = unsafe { &*ptr };
    if bi.magic != bmo_boot_protocol::BOOT_MAGIC { return Err("BootInfo magic mismatch"); }
    Ok(bi)
}

unsafe fn store_boot_info(bi: &bmo_boot_protocol::BootInfo) {
    info::BOOT_INFO = bi as *const _;
    info::FB_ADDR = bi.fb_addr;
    info::FB_WIDTH = bi.fb_width;
    info::FB_HEIGHT = bi.fb_height;
    info::FB_STRIDE = bi.fb_stride;
    info::FB_PIXEL_FORMAT = bi.fb_pixel_format;
}

fn phase0_arch(ctx: &mut BootContext, boot_start: u64) -> u64 {
    s_log("[phase0] === CPU Init ===");
    write_crash_marker(200);
    crate::uefi_rt::write_boot_stage("p0_gdt");
    crate::arch::gdt::init_gdt();
    write_crash_marker(201);
    crate::uefi_rt::write_boot_stage("p0_idt");
    crate::arch::idt::init_idt();
    crate::dev::watchdog::pet_fch_watchdog();
    write_crash_marker(202);
    crate::uefi_rt::write_boot_stage("p0_syscall");
    crate::arch::syscall::init_syscall();
    write_crash_marker(203);
    crate::uefi_rt::write_boot_stage("p0_cpu_init");
    let cpu = crate::cpu::init();
    write_crash_marker(204);
    crate::uefi_rt::write_boot_stage("p0_cpu_done");
    s_log("[phase0] CPU modular init DONE");
    ctx.cpu.tsc_freq_hz = cpu.tsc_freq;
    ctx.cpu.vendor = *b"AuthenticAMD";
    ctx.cpu.features_sse = true;
    ctx.cpu.features_avx = true;
    ctx.cpu.features_avx2 = true;
    ctx.cpu.features_aes = true;
    write_crash_marker(207);
    crate::uefi_rt::write_boot_stage("p0_timer");
    crate::dev::timer::init();
    write_crash_marker(208);
    crate::uefi_rt::write_boot_stage("p0_timer_done");
    let phase0_end = crate::cpu::rdtsc();
    ctx.record_phase(0, boot_start, phase0_end);
    s_log("[phase0] done");
    phase0_end
}

fn phase1_mem(ctx: &mut BootContext, prev_end: u64) -> u64 {
    s_log("[phase1] === Memory Init ===");
    write_crash_marker(2101);
    let bi = match ctx.boot_info() {
        Some(bi) => bi,
        None => { s_log("[phase1] FATAL: BootInfo null"); loop { unsafe { core::arch::asm!("hlt"); } } }
    };
    write_crash_marker(2102);
    crate::uefi_rt::write_boot_stage("p1_phys_init");
    unsafe {
        crate::mm::phys::init(&bi.memory_map, bi.memory_map_count as usize,
            bi.reserved_addr, bi.reserved_size, 0, 0);
    }
    write_crash_marker(2104);
    unsafe { crate::mm::vmm::map_high_mem(&bi.memory_map, bi.memory_map_count as usize); }
    s_log("[phase1] high-mem direct map enabled");
    write_crash_marker(2105);
    crate::uefi_rt::write_boot_stage("p1_heap_init");
    crate::mm::heap::init_heap();
    let free_pages = crate::mm::phys::free_count();
    ctx.memory.free_pages = free_pages as u64;
    ctx.memory.free_mb = ((free_pages * 4096) / (1024 * 1024)) as u64;
    ctx.memory.heap_total_bytes = crate::mm::heap::heap_total() as u64;
    ctx.memory.heap_used_bytes = crate::mm::heap::heap_used() as u64;
    write_crash_marker(2107);
    crate::uefi_rt::write_boot_stage("p1_done");

    // Init vDSO page
    crate::ring0::vdso::init();
    let phase1_end = crate::cpu::rdtsc();
    ctx.record_phase(1, prev_end, phase1_end);
    s_log("[phase1] done");
    phase1_end
}

fn phase2_dev(ctx: &mut BootContext, prev_end: u64) -> u64 {
    s_log("[phase2] === Device Discovery ===");
    write_crash_marker(2201);
    let bi = match ctx.boot_info() {
        Some(bi) => bi,
        None => { s_log("[phase2] FATAL: BootInfo null"); loop { unsafe { core::arch::asm!("hlt"); } } }
    };
    let rsdp_addr = bi.rsdp_addr;
    let mcfg = if rsdp_addr != 0 { crate::dev::acpi::parse_mcfg(rsdp_addr) } else { None };
    if let Some(ref m) = mcfg {
        s_log("[phase2] ECAM PCIe enumeration OK");
        crate::dev::pcie::init_ecam(m.base, m.end_bus);
    } else {
        s_log("[phase2] ACPI MCFG not found, using legacy IO");
        crate::dev::pcie::init_ecam(0, 0);
    }
    write_crash_marker(2202);
    let scan = crate::dev::pcie::scan_pci_bus();
    s_log("[phase2] PCI scan complete");

    // Store XHCI controller addresses for modules + map MMIO.
    // AMD platforms have TWO controllers: CPU SoC + chipset (A320/Promontory).
    s_log("[phase2] calling find_all_xhci_mmio...");
    let (xhci1, xhci2) = crate::dev::pcie::find_all_xhci_mmio();
    s_log("[phase2] find_all_xhci_mmio returned");

    // Map first XHCI MMIO region
    if let Some(mmio) = xhci1 {
        let mmio_base = mmio & !0x1F_FFFF;
        let virt = crate::mm::vmm::HIGH_MEM_BASE + mmio_base;
        let _ = unsafe { crate::mm::vmm::map_kernel_mmio_huge(mmio_base, virt, 2 * 1024 * 1024) };
        s_log("[phase2] XHCI1 MMIO mapped at high-mem");
        unsafe {
            let bi = crate::info::BOOT_INFO as *mut bmo_boot_protocol::BootInfo;
            (*bi).xhci_mmio = mmio;
        }
        crate::ring0::vdso::set_xhci_mmio(mmio);
    }

    // Map second XHCI MMIO region (chipset)
    if let Some(mmio) = xhci2 {
        let mmio_base = mmio & !0x1F_FFFF;
        let virt = crate::mm::vmm::HIGH_MEM_BASE + mmio_base;
        let _ = unsafe { crate::mm::vmm::map_kernel_mmio_huge(mmio_base, virt, 2 * 1024 * 1024) };
        s_log("[phase2] XHCI2 MMIO mapped at high-mem");
        unsafe {
            let bi = crate::info::BOOT_INFO as *mut bmo_boot_protocol::BootInfo;
            (*bi).xhci_mmio2 = mmio;
        }
        crate::ring0::vdso::set_xhci_mmio2(mmio);
    }

    if xhci1.is_some() || xhci2.is_some() {
        s_log("[phase2] XHCI found, stored in BootInfo");
    }
    ctx.devices.acpi_mcfg_base = mcfg.as_ref().map(|m| m.base).unwrap_or(0);
    ctx.devices.pci_devices_found = scan.count as u32;
    write_crash_marker(2205);
    crate::dev::power::init();
    let phase2_end = crate::cpu::rdtsc();
    ctx.record_phase(2, prev_end, phase2_end);
    s_log("[phase2] done");
    phase2_end
}

fn phase3_display(_ctx: &mut BootContext, prev_end: u64) -> u64 {
    s_log("[phase3] === Display Init ===");
    let (fb_addr, fb_w, fb_h, fb_s) = unsafe {
        (crate::info::FB_ADDR, crate::info::FB_WIDTH, crate::info::FB_HEIGHT, crate::info::FB_STRIDE)
    };
    let fmt = unsafe { crate::info::FB_PIXEL_FORMAT };
    crate::dev::framebuffer::init_gop(fb_addr, fb_w, fb_h, fb_s, fmt);
    s_log("[phase3] GOP framebuffer initialized");
    let phase3_end = crate::cpu::rdtsc();
    phase3_end
}

fn phase4_sched(_ctx: &mut BootContext, prev_end: u64) -> u64 {
    s_log("[phase4] === Scheduler Init ===");
    crate::proc::init();
    s_log("[phase4] scheduler tables initialized");
    let phase4_end = crate::cpu::rdtsc();
    phase4_end
}

fn wrap_boot_stage(s: &str) {
    let _ = crate::uefi_rt::write_boot_stage(s);
}

pub fn main(boot_info_ptr: *const bmo_boot_protocol::BootInfo) -> BootContext {
    s_log("[ring0] validating BootInfo");
    let bi = match validate_boot_info(boot_info_ptr) {
        Ok(bi) => bi,
        Err(msg) => { s_log(msg); loop { unsafe { core::arch::asm!("hlt"); } } }
    };
    unsafe { store_boot_info(bi); }
    s_log("[ring0] BootInfo stored");
    crate::uefi_rt::init(bi.uefi_system_table);
    s_log("[ring0] UEFI RT initialized");
    write_crash_marker(0);
    crate::uefi_rt::write_boot_stage("kernel_start");
    let mut ctx = BootContext::new(boot_info_ptr);
    let boot_start = crate::cpu::rdtsc();

    // Show boot splash
    crate::ring0::boot_splash::splash_init();
    crate::ring0::boot_splash::splash_progress(5, "Starting kernel...");

    write_crash_marker(2);
    crate::uefi_rt::write_boot_stage("phases_0_to_4");
    s_log("[ring0] starting boot phases");
    crate::ring0::boot_splash::splash_progress(10, "Phase 0: CPU, GDT, IDT...");
    let mut prev_end = boot_start;
    prev_end = phase0_arch(&mut ctx, prev_end);
    crate::ring0::boot_splash::splash_progress(30, "Phase 1: Memory allocators...");
    prev_end = phase1_mem(&mut ctx, prev_end);
    crate::ring0::boot_splash::splash_progress(50, "Phase 2: Device discovery...");
    prev_end = phase2_dev(&mut ctx, prev_end);
    crate::ring0::boot_splash::splash_progress(65, "Phase 3: Display init...");
    prev_end = phase3_display(&mut ctx, prev_end);
    crate::ring0::boot_splash::splash_progress(75, "Phase 4: Scheduler init...");
    prev_end = phase4_sched(&mut ctx, prev_end);
    s_log("[ring0] all boot phases completed");
    write_crash_marker(3);
    crate::ring0::boot_splash::splash_progress(82, "Detecting CPU...");
    unsafe {
        cpu_vendor_profile::LOG_WRITE_STR = Some(crate::dev::console::serial_write as fn(&str));
        cpu_vendor_profile::LOG_WRITE_U64 = Some(crate::dev::console::serial_write_u64 as fn(u64, usize));
        cpu_vendor_profile::LOG_BOOT_STAGE = Some(wrap_boot_stage as fn(&str));
    }
    crate::uefi_rt::write_boot_stage("init_bmo_cpu");
    s_log("[ring0] detecting CPU");
    cpu_vendor_profile::amd::cpu::zen3::init_bmo_cpu();
    s_log("[ring0] CPU detected");
    write_crash_marker(4);
    crate::ring0::boot_splash::splash_progress(90, "ACPI tables...");
    crate::uefi_rt::write_boot_stage("init_acpi");
    let rsdp_hint = if bi.rsdp_addr != 0 { Some(bi.rsdp_addr) } else { None };
    cpu_vendor_profile::amd::cpu::zen3::init_acpi(rsdp_hint);
    s_log("[ring0] ACPI initialized");
    write_crash_marker(45);
    crate::ring0::boot_splash::splash_progress(95, "SMP: starting cores...");
    crate::uefi_rt::write_boot_stage("smp_init");
    s_log("[ring0] SMP init start");
    unsafe { crate::arch::smp::init(); }
    s_log("[ring0] SMP init done");
    write_crash_marker(5);
    crate::uefi_rt::write_boot_stage("ring0_complete");
    crate::ring0::boot_splash::splash_progress(100, "Boot complete.");
    crate::ring0::boot_splash::splash_clear();
    s_log("[ring0] boot complete");
    let hal = alloc::boxed::Box::new(crate::ring0::hal_init::build(&ctx));
    unsafe { crate::ring0::hal_init::HAL_SERVICES = alloc::boxed::Box::into_raw(hal) as *const _; }
    s_log("[ring0] HalServices built");
    s_log("[ring0] BMO: Ok Ready");
    ctx
}
