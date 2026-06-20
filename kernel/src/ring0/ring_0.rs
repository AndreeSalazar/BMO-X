//! v1.7.4 — Ring 0 coordinator.
//!
//! Coordina la inicialización de todos los subsistemas de Ring 0 en el
//! orden correcto. NO contiene lógica de aplicación — sólo inicializa
//! y entrega el control a BMO Core vía `bmo_core::coord::enter`.
//!
//! Orden de inicialización (no modificar sin revisar dependencias):
//!   1. arch      — CPU primitives (GDT, IDT, APIC, SMP, paging, syscall)
//!   2. drivers   — Hardware drivers (GOP, serial, PCI, NVMe, AHCI, USB, net)
//!   3. memory    — Page allocator + VMM (depende de arch.paging)
//!   4. sched     — Scheduler, process, thread (depende de arch APIC)
//!   5. security  — ByteDefender, Restaurer (depende de memory + sched)
//!   6. syscall   — Syscall dispatcher 0x00..0xFF (depende de arch + sched)
//!
//! Después de init_ring0(), llama a `bmo_core::coord::enter()` que es
//! la fase 5 (welcome + desktop). Esa función no retorna.

use super::arch;
use super::boot;
use super::boot_info;
use super::drivers;
use super::memory;
use super::sched;
use super::security;
use super::syscall;

// `bmo_core` está declarado en `main.rs` con `#[path]`. Para usarlo
// desde ring_0.rs (que es sub-módulo de la crate root), hay que
// subir un nivel: `crate::bmo_core`.
use crate::bmo_core;

/// Inicializa todos los subsistemas de Ring 0.
///
/// Devuelve un `BootContext` con los recursos inicializados para que
/// `bmo_core::coord::enter()` los use.
pub fn init() -> boot::BootContext {
    // 0) serial: lo primero para tener logs.
    drivers::serial::init_serial();

    // 1) arch: CPU primitives — GDT, IDT, APIC, FPU, MTRR/PAT, perf, paging.
    //    `arch::cpu::init()` corre CPUID + CR/XCR + FPU + MTRR + PAT + perf.
    //    v1.6.x: GDT/IDT/APIC se inicializan dentro de cpu::init o en lazy_init.
    boot::log::info("ring0", "init: arch (cpu + gdt + idt + apic)");
    let _cpu_info = arch::cpu::init();
    // Forzamos init explícito de los críticos por si cpu::init los skipea.
    arch::gdt::init_gdt();
    arch::idt::init_idt();
    arch::fpu::init_fpu();
    arch::apic::init_apic(100);
    arch::syscall_entry::init_syscall();

    // 2) drivers: hardware. GOP + serial (ya) + PCI + storage + USB + net.
    //    Los drivers individuales exponen sus `init_X()`; aquí los
    //    llamamos en orden.
    boot::log::info("ring0", "init: drivers (gop + pci + storage + usb + net)");
    // PCI ECAM se inicializa con un rango fijo (ver phase2_devices).
    // El framebuffer GOP se inicializa con los datos del BootInfo.
    let bi = unsafe { &*boot_info::BOOT_INFO };
    if bi.fb_addr != 0 {
        drivers::gop::init_gop(bi.fb_addr, bi.fb_width, bi.fb_height, bi.fb_stride);
    }
    drivers::pci::init_ecam(0xE000_0000, 255);
    drivers::net::init();

    // 3) memory: page allocator + VMM. `page_alloc::init()` se llama
    //    desde boot/visual o desde acá. La init de VMM se hace en
    //    phase1_memory.
    boot::log::info("ring0", "init: memory");
    memory::init();

    // 4) sched: EDF + RR scheduler, process, thread. Inicializa
    //    las tablas vacías — las fases 4+ las pueblan.
    boot::log::info("ring0", "init: sched");
    sched::init();

    // 5) security: ByteDefender + Restaurer. No-op hasta phase 4.5.
    boot::log::info("ring0", "init: security");
    security::init();

    // 6) syscall: dispatcher 0x00..0xFF (legacy FastOS syscalls).
    //    El dispatcher de 0x100..0x1FF (BMO API) se inicializa en
    //    `bmo_core::bmo_api::init()`.
    boot::log::info("ring0", "init: syscall");
    syscall::init();

    // 7) BootContext con los recursos inicializados.
    let bi_ptr = unsafe { boot_info::BOOT_INFO };
    boot::BootContext::new(bi_ptr)
}

/// Despacha la fase 5 (welcome + desktop). Esta función NO retorna.
///
/// Equivalente a `boot::phases::phase5_desktop::run()` en la estructura
/// anterior — pero el orquestador de BMO Core decide qué hacer
/// después de que las fases 0-4 inicialicen.
pub fn dispatch_phase5(ctx: &boot::BootContext, t0: u64, phase4_end: u64) -> ! {
    boot::log::info("ring0", "dispatch phase5 -> bmo_core::coord::enter");
    crate::bmo_core::coord::enter(ctx, t0, phase4_end)
}

/// Punto de entrada desde el bootloader (entry point real en main.rs).
///
/// Esta función es el "hilo principal" de Ring 0. No retorna nunca.
/// Llama en orden:
///   1. init_ring0() — inicializa arch, drivers, memory, sched, security, syscall
///   2. boot::phases::run_all() — corre las fases 0-4 con splash visual
///   3. dispatch_phase5() — entrega el control a BMO Core (no retorna)
pub fn main(boot_info_ptr: *const fastos_boot_protocol::BootInfo) -> ! {
    // Validar el BootInfo del bootloader. Si falla, halt inmediato.
    let bi = match validate_boot_info(boot_info_ptr) {
        Ok(bi) => bi,
        Err(msg) => boot::log::fault("ring0", msg),
    };
    unsafe { store_boot_info(bi); }

    // Limpia el splash y arranca el subsistema.
    boot::visual::clear();
    boot::visual::log("ring0", "init start", boot::visual::color::OK);

    let mut ctx = init();

    // v1.6.18: fase 0-4 con splash visual progresivo.
    boot::phases::run_all(&mut ctx, arch::cpu::rdtsc());

    // v1.6.18: hold splash 1500ms para que el usuario lo vea.
    boot::visual::log("ring0", "hold splash 1500ms", boot::visual::color::OK);
    arch::cpu::busy_wait_ms(1500);

    // Full diag sinks (la fase 4 terminó).
    bmo_core::diag::mark_boot_ready();

    // v1.7.2: inicializa BMO API v2.0 (tablas de ventanas/clases/handles).
    boot::visual::log("ring0", "init bmo_api v2.0", boot::visual::color::OK);
    bmo_core::bmo_api::init();

    // Despacha la fase 5 — no retorna.
    let phase4_end = arch::cpu::rdtsc();
    let t0 = arch::cpu::rdtsc();
    dispatch_phase5(&ctx, t0, phase4_end);
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
    boot_info::BOOT_INFO          = bi as *const _;
    boot_info::RESERVED_PAYLOAD_ADDR = bi.gsp_addr;
    boot_info::RESERVED_PAYLOAD_SIZE = bi.gsp_size;
    boot_info::FB_ADDR  = bi.fb_addr;
    boot_info::FB_WIDTH = bi.fb_width;
    boot_info::FB_HEIGHT = bi.fb_height;
    boot_info::FB_STRIDE = bi.fb_stride;
}
