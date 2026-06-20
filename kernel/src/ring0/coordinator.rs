//! v1.7.5 — Ring 0 coordinator.
//!
//! Coordina la inicialización de todos los subsistemas de Ring 0 en el
//! orden correcto. NO contiene lógica de aplicación — sólo inicializa
//! y entrega el control a BMO Core vía `bmo_core::coord::enter`.
//!
//! Orden de inicialización (no modificar sin revisar dependencias):
//!   1. platform — CPUID, ACPI tables, firmware
//!   2. arch     — GDT, IDT, APIC, SMP, syscall dispatcher
//!   3. dev      — GOP, serial, PCI, watchdog, audio math
//!   4. mem      — Page allocator + VMM (depende de cpu paging)
//!   5. proc     — Scheduler, process, task (depende de arch APIC)
//!   6. bmo_core — BMO API v2.0 init
//!
//! Después de init(), llama a `bmo_core::coord::enter()` que es la
//! fase 5 (welcome + desktop). Esa función no retorna.

use super::boot;
use super::boot::info;
use super::proc;
use super::arch::syscall;
#[allow(unused_imports)]
use super::{cpu, dev, mem, platform};

use crate::bmo_core;

/// Inicializa todos los subsistemas de Ring 0.
pub fn init() -> boot::BootContext {
    // 0) serial: lo primero para tener logs.
    crate::dev::console::init();

    // 1) arch: GDT, IDT, APIC, FPU, MTRR/PAT, perf, paging.
    boot::log::info("ring0", "init: arch (cpu + gdt + idt + apic)");
    let _cpu_info = crate::cpu::init();
    crate::arch::gdt::init_gdt();
    crate::arch::idt::init_idt();
    crate::cpu::fpu::init_fpu();
    crate::arch::apic::init_apic(100);
    crate::arch::syscall::init_syscall();

    // 2) dev: hardware. GOP + serial (ya) + PCI + watchdog.
    boot::log::info("ring0", "init: dev (gop + pci + watchdog)");
    let bi = unsafe { &*info::BOOT_INFO };
    if bi.fb_addr != 0 {
        crate::dev::framebuffer::init_gop(bi.fb_addr, bi.fb_width, bi.fb_height, bi.fb_stride);
    }
    crate::dev::pcie::init_ecam(0xE000_0000, 255);
    crate::dev::watchdog::init();

    // 3) mem: page allocator + VMM.
    boot::log::info("ring0", "init: mem");
    crate::mem::init();

    // 4) proc: scheduler, process, task.
    boot::log::info("ring0", "init: proc");
    proc::init();

    // 5) syscall: dispatcher 0x00..0xFF (legacy).
    boot::log::info("ring0", "init: syscall");
    syscall::init();

    // 6) BootContext con los recursos inicializados.
    let bi_ptr = unsafe { info::BOOT_INFO };
    boot::BootContext::new(bi_ptr)
}

/// Despacha la fase 5 (welcome + desktop). Esta función NO retorna.
pub fn dispatch_phase5(ctx: &boot::BootContext, t0: u64, phase4_end: u64) -> ! {
    boot::log::info("ring0", "dispatch phase5 -> bmo_core::coord::enter");
    crate::bmo_core::coord::enter(ctx, t0, phase4_end)
}

/// Punto de entrada desde el bootloader.
pub fn main(boot_info_ptr: *const fastos_boot_protocol::BootInfo) -> ! {
    let bi = match validate_boot_info(boot_info_ptr) {
        Ok(bi) => bi,
        Err(msg) => boot::log::fault("ring0", msg),
    };
    unsafe { store_boot_info(bi); }

    boot::visual::clear();
    boot::visual::log("ring0", "init start", boot::visual::color::OK);

    let mut ctx = init();

    boot::phases::run_all(&mut ctx, crate::cpu::rdtsc());

    boot::visual::log("ring0", "hold splash 1500ms", boot::visual::color::OK);
    crate::cpu::busy_wait_ms(1500);

    bmo_core::diag::mark_boot_ready();

    boot::visual::log("ring0", "init bmo_api v2.0", boot::visual::color::OK);
    bmo_core::bmo_api::init();

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
    info::BOOT_INFO = bi as *const _;
    info::RESERVED_PAYLOAD_ADDR = bi.gsp_addr;
    info::RESERVED_PAYLOAD_SIZE = bi.gsp_size;
    info::FB_ADDR  = bi.fb_addr;
    info::FB_WIDTH = bi.fb_width;
    info::FB_HEIGHT = bi.fb_height;
    info::FB_STRIDE = bi.fb_stride;
}
