//! v1.8.8 — Ring 0 coordinator.
//!
//! El coordinator es deliberadamente pequeño: valida el `BootInfo`,
//! prepara el log más temprano posible, invoca `vendor::amd::cpu::zen3::fastos_cpu`
//! para detectar todos los datos del 5600X, y entrega el control al
//! boot por fases.
//!
//! Orden real:
//!   0. fastos_cpu    — CPUID, family/model, brand, cache, TSC, errata, ACPI
//!   0. arch/cpu      — GDT, IDT, syscall, FPU, MSRs (EFER/STAR/LSTAR/FMASK)
//!   1. mem           — frame allocator, heap, VMM base
//!   2. dev           — PCIe scan (con ACPI MCFG real), ACPI tables
//!   3. display       — GOP framebuffer (con MTRR/PAT reales)
//!   4. proc          — scheduler/APIC timer/STI
//!   5. bmo_core      — desktop/API CPU-side; no retorna
//!
//! Política FastOS v1.8.8: el kernel es ESPECÍFICO del Ryzen 5 5600X.
//! Todos los datos del CPU se obtienen de `AMD/zen3/` (no de genéricos
//! o stubs). La detección se hace con `init_fastos_cpu()` en
//! `coordinator::main`, antes de cualquier otra fase.

use super::boot;
use super::boot::info;

use crate::bmo_core;

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

    boot::visual::clear();
    boot::visual::log("ring0", "init start", boot::visual::color::OK);
    crate::dev::console::serial_write("[coord] main: visual init done\n");

    let mut ctx = init();
    crate::dev::console::serial_write("[coord] main: init() done\n");
    let boot_start = crate::cpu::rdtsc();

    // ── Phase 0 + 1 + 2 + 3 + 4 ──────────────────────────────────
    // p0_arch inicializa GDT/IDT/SYSCALL antes de cualquier CPUID
    // complejo. p1_mem inicializa el heap. Sin esto, init_fastos_cpu
    // colgaba porque el código asumía un entorno más inicializado.
    boot::visual::log("ring0", "[0/5] arch init", boot::visual::color::OK);
    crate::dev::console::serial_write("[coord] main: starting phases::run_all\n");
    let phase4_end = boot::phases::run_all(&mut ctx, boot_start);
    crate::dev::console::serial_write("[coord] main: phases::run_all returned\n");

    // ── Initialize ALL data of the 5600X (one-shot) ───────────────
    // AHORA es seguro: GDT/IDT/SYSCALL están listos (p0), heap está
    // listo (p1), display está listo (p3). CPUID complejo, TSC
    // calibration con PM timer, MSR writes y ACPI parsing funcionan
    // en este orden. Antes se llamaba aquí sin init, lo que colgaba.
    boot::visual::log("ring0", "[1/5] detect 5600X", boot::visual::color::OK);
    crate::dev::console::serial_write("[coord] main: calling init_fastos_cpu\n");
    crate::vendor::amd::cpu::zen3::init_fastos_cpu();
    boot::visual::log("ring0", "[1/5] 5600X detected", boot::visual::color::OK);
    crate::dev::console::serial_write("[coord] main: init_fastos_cpu returned\n");

    // ── Init MSRs (EFER, STAR, LSTAR, FMASK, PAT, TSC_AUX) ───────
    let syscall_entry = bi.kernel_base;
    boot::visual::log("ring0", "[2/5] init MSRs", boot::visual::color::OK);
    crate::dev::console::serial_write("[coord] main: calling init_msrs\n");
    crate::vendor::amd::cpu::zen3::init_msrs(syscall_entry);
    crate::dev::console::serial_write("[coord] main: init_msrs returned\n");

    // ── Init ACPI (uses RSDP address from BootInfo) ──────────────
    let rsdp_hint = if bi.rsdp_addr != 0 { Some(bi.rsdp_addr) } else { None };
    boot::visual::log("ring0", "[3/5] init ACPI", boot::visual::color::OK);
    crate::dev::console::serial_write("[coord] main: calling init_acpi\n");
    crate::vendor::amd::cpu::zen3::init_acpi(rsdp_hint);
    crate::dev::console::serial_write("[coord] main: init_acpi returned\n");

    boot::log::info("ring0", "fastos_cpu init complete");
    boot::visual::log("ring0", "[4/5] CPU ready", boot::visual::color::OK);

    boot::visual::log("ring0", "hold splash 1500ms", boot::visual::color::OK);
    crate::cpu::busy_wait_ms(1500);

    // ── BMO Core init (cabina + defense + timeback + bmo_api + desktop) ──
    boot::visual::log("ring0", "init bmo_core", boot::visual::color::OK);
    crate::dev::console::serial_write("[coord] main: calling bmo_core::coord::init\n");
    bmo_core::coord::init();
    crate::dev::console::serial_write("[coord] main: bmo_core::coord::init returned\n");

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
}
