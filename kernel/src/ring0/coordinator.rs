//! v1.7.4 — Ring 0 coordinator.
//!
//! Coordina la inicialización de todos los subsistemas de Ring 0 en el
//! orden correcto. NO contiene lógica de aplicación — sólo inicializa
//! y entrega el control a BMO Core vía `bmo_core::coord::enter`.
//!
//! Orden de inicialización (no modificar sin revisar dependencias):
//!   1. interrupt — GDT, IDT, APIC, SMP, syscall dispatcher
//!   2. device    — GOP, serial, PCI, watchdog, audio math
//!   3. memory    — Page allocator + VMM (depende de cpu paging)
//!   4. sched     — Scheduler, process, thread (depende de interrupt APIC)
//!   5. syscall   — Driver API legacy 0x00..0xFF (depende de interrupt + sched)
//!
//! Después de init(), llama a `bmo_core::coord::enter()` que es la
//! fase 5 (welcome + desktop). Esa función no retorna.

use super::boot;
use super::boot_info;
use super::sched;
use super::syscall;
#[allow(unused_imports)]
use super::{cpu, device, interrupt, memory};

// `bmo_core` está declarado en `entry.rs` con `#[path]`. Para usarlo
// desde coordinator.rs (que es sub-módulo de la crate root), hay que
// subir un nivel: `crate::bmo_core`.
use crate::bmo_core;

/// Inicializa todos los subsistemas de Ring 0.
///
/// Devuelve un `BootContext` con los recursos inicializados para que
/// `bmo_core::coord::enter()` los use.
pub fn init() -> boot::BootContext {
    // 0) serial: lo primero para tener logs.
    crate::device::serial::init_serial();

    // 1) interrupt: GDT, IDT, APIC, FPU, MTRR/PAT, perf, paging.
    //    `crate::cpu::init()` corre CPUID + CR/XCR + FPU + MTRR + PAT + perf.
    //    v1.6.x: GDT/IDT/APIC se inicializan dentro de crate::cpu::init o en lazy_init.
    boot::log::info("ring0", "init: interrupt (cpu + gdt + idt + apic)");
    let _cpu_info = crate::cpu::init();
    // Forzamos init explícito de los críticos por si crate::cpu::init los skipea.
    crate::interrupt::gdt::init_gdt();
    crate::interrupt::idt::init_idt();
    crate::cpu::fpu::init_fpu();
    crate::interrupt::apic::init_apic(100);
    crate::interrupt::syscall::init_syscall();

    // 2) device: hardware. GOP + serial (ya) + PCI + watchdog.
    //    Los drivers individuales exponen sus `init_X()`; aquí los
    //    llamamos en orden.
    boot::log::info("ring0", "init: device (gop + pci + watchdog)");
    // PCI ECAM se inicializa con un rango fijo (ver phase2_devices).
    // El framebuffer GOP se inicializa con los datos del BootInfo.
    let bi = unsafe { &*boot_info::BOOT_INFO };
    if bi.fb_addr != 0 {
        crate::device::gop::init_gop(bi.fb_addr, bi.fb_width, bi.fb_height, bi.fb_stride);
    }
    crate::device::pci::init_ecam(0xE000_0000, 255);
    crate::device::watchdog::init();

    // 3) memory: page allocator + VMM. `page_alloc::init()` se llama
    //    desde boot/visual o desde acá. La init de VMM se hace en
    //    phase1_memory.
    boot::log::info("ring0", "init: memory");
    crate::memory::init();

    // 4) sched: EDF + RR scheduler, process, thread. Inicializa
    //    las tablas vacías — las fases 4+ las pueblan.
    boot::log::info("ring0", "init: sched");
    sched::init();

    // 5) syscall: dispatcher 0x00..0xFF (legacy FastOS syscalls).
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

/// Punto de entrada desde el bootloader (entry point real en entry.rs).
///
/// Esta función es el "hilo principal" de Ring 0. No retorna nunca.
/// Llama en orden:
///   1. init() — inicializa interrupt, device, memory, sched, syscall
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
    boot::phases::run_all(&mut ctx, crate::cpu::rdtsc());

    // v1.6.18: hold splash 1500ms para que el usuario lo vea.
    boot::visual::log("ring0", "hold splash 1500ms", boot::visual::color::OK);
    crate::cpu::busy_wait_ms(1500);

    // Full diag sinks (la fase 4 terminó).
    bmo_core::diag::mark_boot_ready();

    // v1.7.2: inicializa BMO API v2.0 (tablas de ventanas/clases/handles).
    boot::visual::log("ring0", "init bmo_api v2.0", boot::visual::color::OK);
    bmo_core::bmo_api::init();

    // Despacha la fase 5 — no retorna.
    let phase4_end = crate::cpu::rdtsc();
    let t0 = phase4_end;
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
