//! `cabina` — Cabina de Control Operativo (FastOS Omniscient Cockpit).
//!
//! v1.8.8: cabina es el **ojo omnisciente** del sistema. Observa
//! Ring 0 (CPU, IRQ, memoria, procesos), BMO Core (windowing, FS),
//! Ring 3 (apps), y futuro BMO GPU (RDNA4).
//!
//! ## Capas
//!
//! ```text
//! ┌──────────────────────────────────────────────────────┐
//! │ cabina/                                              │
//! │   ├── event      ← caja negra circular RAM           │
//! │   ├── telemetry  ← contadores atómicos               │
//! │   ├── snapshot   ← API limpia para leer ring0        │
//! │   ├── filter     ← filtros inteligentes              │
//! │   ├── serial     ← sink COM1                         │
//! │   ├── overlay    ← HUD GOP con tabs                  │
//! │   └── panels/    ← cada tab es un panel              │
//! │         ├── overview.rs                              │
//! │         ├── cpu.rs                                   │
//! │         ├── memory.rs                                │
//! │         ├── io.rs                                    │
//! │         ├── scheduler.rs                             │
//! │         ├── log.rs                                   │
//! │         └── gpu.rs (futuro RDNA4)                    │
//! └──────────────────────────────────────────────────────┘
//!                │
//!                ▼
//! ┌──────────────────────────────────────────────────────┐
//! │ ring0/diag_min  ← diagnósticos de emergencia         │
//! │   (panic-safe, no-alloc, sin locks)                  │
//! └──────────────────────────────────────────────────────┘
//!                │
//!                ▼
//! ┌──────────────────────────────────────────────────────┐
//! │ Ring 0 hardware: CPU, IRQ, memoria, proc, GPU        │
//! └──────────────────────────────────────────────────────┘
//! ```
//!
//! ## Activación
//!
//! - **F9**: mostrar/ocultar HUD
//! - **F10**: siguiente pestaña
//! - **Ctrl+Alt**: mostrar/ocultar HUD (emergencia)
//! - **Ctrl+Alt+F10**: siguiente pestaña (emergencia)
//!
//! ## Integración con BMO ABI
//!
//! - BMO ABI: `NR_DEBUG_DIAG_EMIT = 0x1F4` (futuro) para que
//!   apps Ring 3 puedan emitir eventos.
//! - LANG: BMO puede llamar `diag_emit()` desde el codegen.

#![allow(dead_code)]

pub mod event;
pub mod telemetry;
pub mod snapshot;
pub mod filter;
pub mod serial;
pub mod overlay;
pub mod panels;

use event::Severity;
use event::Event;
use core::sync::atomic::{AtomicBool, Ordering};
use alloc::string::String;

static BOOT_READY: AtomicBool = AtomicBool::new(false);
static OVERLAY_ENABLED: AtomicBool = AtomicBool::new(false);
static CURRENT_TAB: AtomicU8 = AtomicU8::new(0);

use core::sync::atomic::AtomicU8;

/// Versión de la cabina.
pub const CABINA_VERSION: (u8, u8) = (1, 0);

/// Número máximo de eventos en la caja negra circular.
pub const BLACKBOX_CAP: usize = 256;

/// Inicializa la cabina. Llamar desde `bmo_core::init` después de
/// tener framebuffer GOP y serial COM1.
pub fn init() {
    event::buffer::init();
    telemetry::init();
    serial::init();
    BOOT_READY.store(true, Ordering::SeqCst);
}

/// Marca la cabina como "ready" (boot completado, framebuffer OK).
/// Habilita el overlay.
pub fn boot_ready() {
    BOOT_READY.store(true, Ordering::SeqCst);
    overlay::enable();
}

/// Emite un evento a la cabina.
///
/// Es la API principal que debe usar el resto del sistema.
/// El evento se guarda en la caja negra, se envía a serial, y
/// marca el overlay como dirty (no repinta inmediatamente).
pub fn emit(severity: Severity, module: &str, msg: &str) {
    if !BOOT_READY.load(Ordering::Relaxed) {
        // Antes de boot_ready, solo serial (sin overlay).
        serial::write_raw(severity, module, msg);
        return;
    }
    let ev = Event::new(severity, module, msg);
    event::buffer::push(&ev);
    serial::write_event(&ev);
    overlay::mark_dirty();
}

/// Versión con un valor numérico adicional (e.g. fault address).
pub fn emit_value(severity: Severity, module: &str, msg: &str, value: u64) {
    if !BOOT_READY.load(Ordering::Relaxed) {
        serial::write_raw(severity, module, msg);
        return;
    }
    let mut ev = Event::new(severity, module, msg);
    ev.value = value;
    event::buffer::push(&ev);
    serial::write_event(&ev);
    overlay::mark_dirty();
}

/// Helpers de conveniencia (compat con `bmo_core::diag::*`).
pub fn info(module: &str, msg: &str) { emit(Severity::Info, module, msg); }
pub fn warn(module: &str, msg: &str) { emit(Severity::Warning, module, msg); }
pub fn fault(module: &str, msg: &str) { emit(Severity::Fault, module, msg); }
pub fn panic_msg(module: &str, msg: &str) { emit(Severity::Panic, module, msg); }
pub fn trace(module: &str, msg: &str) { emit(Severity::Trace, module, msg); }
pub fn assert(cond: bool, module: &str, msg: &str) {
    if !cond { fault(module, msg); }
}

/// Toggle del overlay (llamado por F9 o Ctrl+Alt).
pub fn toggle_overlay() {
    OVERLAY_ENABLED.store(!OVERLAY_ENABLED.load(Ordering::Relaxed), Ordering::SeqCst);
    if OVERLAY_ENABLED.load(Ordering::Relaxed) {
        overlay::enable();
    } else {
        overlay::disable();
    }
}

/// Avanza a la siguiente pestaña (llamado por F10).
pub fn cycle_tab() {
    let cur = CURRENT_TAB.load(Ordering::Relaxed);
    let next = if cur >= panels::PANEL_COUNT as u8 - 1 { 0 } else { cur + 1 };
    CURRENT_TAB.store(next, Ordering::SeqCst);
}

/// Repinta el overlay si está habilitado y dirty.
/// Llamar desde el desktop timer (limitado a OVERLAY_REFRESH_HZ).
pub fn tick() {
    if !OVERLAY_ENABLED.load(Ordering::Relaxed) { return; }
    if !overlay::is_dirty() { return; }
    let tab = CURRENT_TAB.load(Ordering::Relaxed);
    overlay::paint(tab);
    overlay::clear_dirty();
}

/// Estado del overlay.
pub fn overlay_enabled() -> bool { OVERLAY_ENABLED.load(Ordering::Relaxed) }

/// Pestaña actual.
pub fn current_tab() -> u8 { CURRENT_TAB.load(Ordering::Relaxed) }

/// Boot ready check.
pub fn is_ready() -> bool { BOOT_READY.load(Ordering::Relaxed) }
