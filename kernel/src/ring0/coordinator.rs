//! v1.8.15 — Ring 0 coordinator.
//!
//! El coordinator es deliberadamente pequeño: valida el `BootInfo`,
//! prepara el log más temprano posible, y orquesta el boot en orden:
//!
//! Orden real:
//!   0. arch/cpu      — GDT, IDT, syscall, FPU, MSRs (EFER/STAR/LSTAR/FMASK)
//!   1. fastos_cpu    — CPUID, family/model, brand, cache, TSC, erratas, ACPI
//!   2. mem           — frame allocator, heap, VMM base
//!   3. dev           — PCIe scan (con ACPI MCFG real), ACPI tables
//!   4. display       — GOP framebuffer (con MTRR/PAT reales)
//!   5. proc          — scheduler/APIC timer/STI
//!   6. bmo_core      — desktop/API CPU-side; no retorna
//!
//! v1.8.15: init_fastos_cpu/init_msrs/init_acpi corren ANTES de las
//! fases 1-4 para garantizar que MTRR/PAT estén configurados antes
//! de Phase 3 (display) que toca el framebuffer.
//!
//! Política FastOS v1.8.15: el kernel es ESPECÍFICO del Ryzen 5 5600X.
//! Todos los datos del CPU se obtienen de `AMD/zen3/` (no de genéricos
//! o stubs). La detección se hace con `init_fastos_cpu()` en
//! `coordinator::main`, después de Phase 0 (arch).

use super::boot;
use super::boot::info;

use crate::bmo_core;

// ── Crash marker: physical address 0x90000 ─────────────────────────────
// The bootloader reads this on next boot and writes to crash.log on SSD.
// Format: [magic: u32 LE] [stage: u32 LE]
const CRASH_MARKER_ADDR: u64 = 0x9_0000;
const CRASH_MAGIC: u32 = 0x464F_5343; // "FOSC"

/// Write a boot stage marker to physical address 0x90000.
/// Called at each phase so the bootloader knows where we died if we crash.
pub fn write_crash_marker(stage: u32) {
    unsafe {
        core::ptr::write_volatile(CRASH_MARKER_ADDR as *mut u32, CRASH_MAGIC);
        core::ptr::write_volatile((CRASH_MARKER_ADDR + 4) as *mut u32, stage);
    }
}

/// Clear the crash marker (called after successful boot).
pub fn clear_crash_marker() {
    unsafe {
        core::ptr::write_volatile(CRASH_MARKER_ADDR as *mut u32, 0);
        core::ptr::write_volatile((CRASH_MARKER_ADDR + 4) as *mut u32, 0);
    }
}

/// Prepara el contexto mínimo antes del boot por fases.
pub fn init() -> boot::BootContext {
    crate::dev::console::init();
    let bi_ptr = unsafe { info::BOOT_INFO };
    boot::BootContext::new(bi_ptr)
}

/// Despacha la fase 5 (welcome + desktop). Esta función NO retorna.
pub fn dispatch_phase5(ctx: &boot::BootContext, t0: u64, phase4_end: u64) -> ! {
    boot::log::info("ring0", "dispatch phase5 -> bmo_core::coord::enter");
    crate::bmo_core::coord::enter(ctx, t0, phase4_end)
}

/// Punto de entrada desde el bootloader.
///
/// v1.8.8: ahora invoca `init_fastos_cpu()` que detecta TODO sobre el
/// 5600X (vendor, family, model, brand, features, cache, TSC, TLB,
/// topology) y aplica los workarounds de erratas.
pub fn main(boot_info_ptr: *const fastos_boot_protocol::BootInfo) -> ! {
    let bi = match validate_boot_info(boot_info_ptr) {
        Ok(bi) => bi,
        Err(msg) => boot::log::fault("ring0", msg),
    };
    crate::dev::console::serial_write("[coord] main: boot_info validated\n");
    unsafe { store_boot_info(bi); }

    // Initialize UEFI Runtime Services for NVRAM access
    boot::uefi_rt::init(bi.uefi_system_table);
    boot::uefi_rt::write_boot_stage("kernel_start");

    // Stage 1: boot_info validated
    write_crash_marker(1);

    boot::visual::clear();
    boot::visual::log("ring0", "init start", boot::visual::color::OK);
    crate::dev::console::serial_write("[coord] main: visual init done\n");

    let mut ctx = init();
    crate::dev::console::serial_write("[coord] main: init() done\n");
    let boot_start = crate::cpu::rdtsc();

    // Stage 2: phases 0-4
    write_crash_marker(2);
    boot::uefi_rt::write_boot_stage("phase_0_to_4");
    boot::visual::log("ring0", "[0/5] boot phases", boot::visual::color::OK);
    crate::dev::console::serial_write("[coord] main: starting phases 0-4\n");
    let phase4_end = boot::phases::run_phases_0_to_4(&mut ctx, boot_start);
    boot::visual::log("ring0", "[0/5] phases done", boot::visual::color::OK);
    crate::dev::console::serial_write("[coord] main: phases 0-4 returned\n");

    // Stage 3: init_fastos_cpu
    write_crash_marker(3);
    boot::uefi_rt::write_boot_stage("init_fastos_cpu");
    boot::visual::log("ring0", "[1/5] detect 5600X", boot::visual::color::OK);
    crate::dev::console::serial_write("[coord] main: calling init_fastos_cpu\n");
    crate::vendor::amd::cpu::zen3::init_fastos_cpu();
    boot::visual::log("ring0", "[1/5] 5600X detected", boot::visual::color::OK);
    crate::dev::console::serial_write("[coord] main: init_fastos_cpu returned\n");

    // Stage 4: init_acpi
    write_crash_marker(4);
    boot::uefi_rt::write_boot_stage("init_acpi");
    let rsdp_hint = if bi.rsdp_addr != 0 { Some(bi.rsdp_addr) } else { None };
    boot::visual::log("ring0", "[2/5] init ACPI", boot::visual::color::OK);
    crate::dev::console::serial_write("[coord] main: calling init_acpi\n");
    crate::vendor::amd::cpu::zen3::init_acpi(rsdp_hint);
    crate::dev::console::serial_write("[coord] main: init_acpi returned\n");

    boot::log::info("ring0", "fastos_cpu init complete");
    boot::visual::log("ring0", "[3/5] CPU+ACPI ready", boot::visual::color::OK);

    // Stage 4.5: SMP — start AP cores
    write_crash_marker(45);
    boot::uefi_rt::write_boot_stage("smp_init");
    boot::visual::log("ring0", "[3.5/5] SMP init", boot::visual::color::OK);
    crate::dev::console::serial_write("[coord] main: calling smp::init\n");
    unsafe { crate::arch::smp::init(); }
    let smp_state = crate::arch::smp::state();
    let smp_cores = crate::arch::smp::core_count();
    crate::dev::console::serial_write("[coord] main: SMP state=");
    crate::dev::console::serial_write_u64(smp_state as u64, 10);
    crate::dev::console::serial_write(" cores=");
    crate::dev::console::serial_write_u64(smp_cores as u64, 10);
    crate::dev::console::serial_write("\n");
    if smp_cores > 1 {
        boot::visual::log("ring0", "[3.5/5] SMP online", boot::visual::color::OK);
    } else {
        boot::visual::log("ring0", "[3.5/5] SMP single-core", boot::visual::color::WARN);
    }

    boot::visual::log("ring0", "hold splash 1500ms", boot::visual::color::OK);
    // Pet the AMD FCH hardware watchdog during the long wait.
    // The watchdog fires after ~10-15 seconds if not petted via FCH MMIO.
    let wait_start = crate::cpu::rdtsc();
    let tsc_per_ms = crate::cpu::tsc_per_sec() / 1000;
    while crate::cpu::rdtsc().wrapping_sub(wait_start) < 1500 * tsc_per_ms.max(1) {
        let elapsed_ms = crate::cpu::rdtsc().wrapping_sub(wait_start) / tsc_per_ms.max(1);
        if elapsed_ms % 200 == 0 {
            // Pet FCH watchdog: write 0x01 to WD_GCR at FED80B00
            unsafe {
                core::ptr::write_volatile(0xFED8_0B00 as *mut u8, 0x01);
            }
        }
        core::hint::spin_loop();
    }

    // Stage 5: bmo_core::coord::init
    write_crash_marker(5);
    boot::uefi_rt::write_boot_stage("bmo_core_init");
    boot::visual::log("ring0", "init bmo_core", boot::visual::color::OK);
    crate::dev::console::serial_write("[coord] main: calling bmo_core::coord::init\n");
    bmo_core::coord::init();
    crate::dev::console::serial_write("[coord] main: bmo_core::coord::init returned\n");

    // Stage 6: entering welcome (no return expected)
    write_crash_marker(6);
    boot::uefi_rt::write_boot_stage("welcome");

    dispatch_phase5(&ctx, boot_start, phase4_end);
}

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
    info::RESERVED_PAYLOAD_ADDR = bi.reserved_addr;
    info::RESERVED_PAYLOAD_SIZE = bi.reserved_size;
    info::FB_ADDR  = bi.fb_addr;
    info::FB_WIDTH = bi.fb_width;
    info::FB_HEIGHT = bi.fb_height;
    info::FB_STRIDE = bi.fb_stride;
    info::FB_PIXEL_FORMAT = bi.fb_pixel_format;
}
