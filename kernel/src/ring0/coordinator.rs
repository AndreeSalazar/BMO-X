//! v1.7.11 — Ring 0 coordinator.
//!
//! El coordinator es deliberadamente pequeño: valida el `BootInfo`,
//! prepara el log más temprano posible y entrega el control al boot
//! por fases. Cada subsistema se inicializa una sola vez dentro de su
//! fase dueña; el coordinator no repite GDT/IDT/APIC/mem/dev/proc.
//!
//! Orden real de fases:
//!   0. arch/cpu  — CPUID, GDT, IDT, syscall, FPU
//!   1. mem       — frame allocator, heap, VMM base
//!   2. dev       — ACPI/PCI discovery seguro; storage/net/watchdog deferidos
//!   3. display   — GOP framebuffer heredado de UEFI
//!   4. proc      — scheduler/APIC timer/STI
//!   5. bmo_core  — desktop/API CPU-side; no retorna
//!
//! Política FastOS: Ring 0 es hardware puro, optimizado para el CPU
//! objetivo del build (hoy Ryzen 5 5600X). Otros CPUs deben entrar como
//! perfiles explícitos, no como una ruta genérica lenta.

use super::boot;
use super::boot::info;

use crate::bmo_core;

/// Prepara el contexto mínimo antes del boot por fases.
///
/// No inicializa subsistemas de hardware salvo COM1 para logs tempranos.
/// Las fases son la única fuente de verdad para arch/mem/dev/proc.
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
pub fn main(boot_info_ptr: *const fastos_boot_protocol::BootInfo) -> ! {
    let bi = match validate_boot_info(boot_info_ptr) {
        Ok(bi) => bi,
        Err(msg) => boot::log::fault("ring0", msg),
    };
    unsafe { store_boot_info(bi); }

    boot::visual::clear();
    boot::visual::log("ring0", "init start", boot::visual::color::OK);

    let mut ctx = init();
    let boot_start = crate::cpu::rdtsc();

    let phase4_end = boot::phases::run_all(&mut ctx, boot_start);

    boot::visual::log("ring0", "hold splash 1500ms", boot::visual::color::OK);
    crate::cpu::busy_wait_ms(1500);

    bmo_core::diag::mark_boot_ready();

    boot::visual::log("ring0", "init bmo_api v2.0", boot::visual::color::OK);
    bmo_core::bmo_api::init();

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
    info::RESERVED_PAYLOAD_ADDR = bi.gsp_addr;
    info::RESERVED_PAYLOAD_SIZE = bi.gsp_size;
    info::FB_ADDR  = bi.fb_addr;
    info::FB_WIDTH = bi.fb_width;
    info::FB_HEIGHT = bi.fb_height;
    info::FB_STRIDE = bi.fb_stride;
}
