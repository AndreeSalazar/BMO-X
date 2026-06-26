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
//! │   ├── buffer     ← blackbox 256 eventos              │
//! │   ├── telemetry  ← contadores atómicos               │
//! │   ├── snapshot   ← API limpia para leer ring0        │
//! │   ├── filter     ← filtros simples                   │
//! │   ├── query      ← DSL de filtros inteligentes       │
//! │   ├── serial     ← sink COM1                         │
//! │   ├── persistent ← spool RAM→SSD (futuro)            │
//! │   ├── overlay    ← HUD GOP con tabs                  │
//! │   ├── paint      ← primitivas FB                     │
//! │   └── panels/    ← cada tab es un panel              │
//! └──────────────────────────────────────────────────────┘
//! ```
//!
//! ## Activación
//!
//! - **F9**: mostrar/ocultar HUD
//! - **F10**: siguiente pestaña
//! - **Ctrl+Alt**: mostrar/ocultar HUD (emergencia)
//! - **Ctrl+Alt+F10**: siguiente pestaña (emergencia)

#![allow(dead_code)]

pub mod event;
pub mod buffer;
pub mod telemetry;
pub mod snapshot;
pub mod filter;
pub mod query;
pub mod serial;
pub mod persistent;
pub mod paint;
pub mod overlay;
pub mod panels;
pub mod tests;

pub use event::{Severity, Layer, Entity, Event};

use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static BOOT_READY: AtomicBool = AtomicBool::new(false);
static OVERLAY_ENABLED: AtomicBool = AtomicBool::new(false);
static CURRENT_TAB: AtomicU8 = AtomicU8::new(0);
static ACTIVE_QUERY: AtomicU8 = AtomicU8::new(0); // índice del query preset activo

/// Versión de la cabina.
pub const CABINA_VERSION: (u8, u8) = (1, 0);

/// Número máximo de eventos en la caja negra circular.
pub const BLACKBOX_CAP: usize = 256;

/// IDs de los queries pre-construidos.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueryId {
    /// Solo Fault + Panic.
    OnlyErrors    = 0,
    /// Solo Warning + Fault + Panic.
    OnlyCritical  = 1,
    /// Ring 0 + BMO Core.
    Kernel        = 2,
    /// Solo Ring 3.
    Ring3         = 3,
    /// Solo BMO GPU.
    Gpu           = 4,
    /// Sin filtro (todos).
    All           = 5,
}

/// Construye un query según el ID. v1.8.8: cada llamada aloca un
/// nuevo Vec (no hay const Vec con elementos no-Copy).
pub fn build_query(qid: QueryId) -> query::Query {
    use query::Query;
    use event::{Layer, Severity};
    match qid {
        QueryId::OnlyErrors => Query::new()
            .with_severities(alloc::vec![Severity::Fault, Severity::Panic]),
        QueryId::OnlyCritical => Query::new()
            .with_severities(alloc::vec![Severity::Warning, Severity::Fault, Severity::Panic]),
        QueryId::Kernel => Query::new()
            .with_layers(alloc::vec![Layer::Ring0, Layer::BmoCore]),
        QueryId::Ring3 => Query::new()
            .with_layers(alloc::vec![Layer::Ring3]),
        QueryId::Gpu => Query::new()
            .with_layers(alloc::vec![Layer::BmoGpu]),
        QueryId::All => Query::new(),
    }
}

/// Nombre del query según el ID.
pub fn query_id_name(qid: QueryId) -> &'static str {
    match qid {
        QueryId::OnlyErrors => "errors only",
        QueryId::OnlyCritical => "critical only",
        QueryId::Kernel => "kernel",
        QueryId::Ring3 => "ring3",
        QueryId::Gpu => "gpu",
        QueryId::All => "all",
    }
}

// ── Inicialización ────────────────────────────────────────────────

/// Inicializa la cabina.
pub fn init() {
    buffer::init();
    telemetry::init();
    serial::init();
    paint::init();
    persistent::init();
    BOOT_READY.store(true, Ordering::SeqCst);
}

/// Marca la cabina como "ready" (boot completado, framebuffer OK).
pub fn boot_ready() {
    BOOT_READY.store(true, Ordering::SeqCst);
    overlay::enable();
}

/// Marca la cabina como "ready" (alias para compatibilidad).
pub fn mark_boot_ready() {
    boot_ready();
}

// ── Emisión de eventos ───────────────────────────────────────────

/// Emite un evento a la cabina.
/// La capa se infiere automáticamente del nombre del módulo.
pub fn emit(severity: Severity, module: &str, msg: &str) {
    if !BOOT_READY.load(Ordering::Relaxed) {
        serial::write_raw(severity, module, msg);
        return;
    }
    let ev = Event::new(severity, module, msg);
    buffer::push(&ev);
    serial::write_event(&ev);
    overlay::mark_dirty();
}

/// Emite con layer explícito.
pub fn emit_layer(severity: Severity, layer: event::Layer, module: &str, msg: &str) {
    if !BOOT_READY.load(Ordering::Relaxed) {
        serial::write_raw(severity, module, msg);
        return;
    }
    let mut ev = Event::new(severity, module, msg);
    ev.layer = layer;
    buffer::push(&ev);
    serial::write_event(&ev);
    overlay::mark_dirty();
}

/// Emite con layer + entity + entity_id + value.
pub fn emit_full(
    severity: Severity,
    layer: event::Layer,
    entity: event::Entity,
    entity_id: u32,
    module: &str,
    msg: &str,
    value: u64,
) {
    if !BOOT_READY.load(Ordering::Relaxed) {
        serial::write_raw(severity, module, msg);
        return;
    }
    let mut ev = Event::new(severity, module, msg);
    ev.layer = layer;
    ev.entity = entity;
    ev.entity_id = entity_id;
    ev.value = value;
    buffer::push(&ev);
    serial::write_event(&ev);
    overlay::mark_dirty();
}

/// Helpers de conveniencia (severidad simple).
pub fn info(module: &str, msg: &str) { emit(Severity::Info, module, msg); }
pub fn warn(module: &str, msg: &str) { emit(Severity::Warning, module, msg); }
pub fn fault(module: &str, msg: &str) { emit(Severity::Fault, module, msg); }
pub fn panic_msg(module: &str, msg: &str) { emit(Severity::Panic, module, msg); }
pub fn trace(module: &str, msg: &str) { emit(Severity::Trace, module, msg); }
pub fn assert(cond: bool, module: &str, msg: &str) {
    if !cond { fault(module, msg); }
}

/// Helpers con `u64` (compatibilidad con bmo_core::diag legacy).
pub fn info_u64(module: &str, msg: &str, value: u64) {
    let mut ev = Event::new(Severity::Info, module, msg);
    ev.value = value;
    emit_event_obj(ev);
}
pub fn warn_u64(module: &str, msg: &str, value: u64) {
    let mut ev = Event::new(Severity::Warning, module, msg);
    ev.value = value;
    emit_event_obj(ev);
}
pub fn fault_u64(module: &str, msg: &str, value: u64) {
    let mut ev = Event::new(Severity::Fault, module, msg);
    ev.value = value;
    emit_event_obj(ev);
}
pub fn trace_u64(module: &str, msg: &str, value: u64) {
    let mut ev = Event::new(Severity::Trace, module, msg);
    ev.value = value;
    emit_event_obj(ev);
}

fn emit_event_obj(ev: Event) {
    if !BOOT_READY.load(Ordering::Relaxed) {
        serial::write_event(&ev);
        return;
    }
    buffer::push(&ev);
    serial::write_event(&ev);
    overlay::mark_dirty();
}

/// Emit un evento arbitrario (compatibilidad).
pub fn event(severity: Severity, module: &str, msg: &str) {
    emit(severity, module, msg);
}

pub fn event_u64(severity: Severity, module: &str, msg: &str, value: u64) {
    let mut ev = Event::new(severity, module, msg);
    ev.value = value;
    emit_event_obj(ev);
}

// ── Overlay / Tabs ──────────────────────────────────────────────

/// Toggle del overlay (F9 o Ctrl+Alt).
pub fn toggle_overlay() {
    OVERLAY_ENABLED.store(!OVERLAY_ENABLED.load(Ordering::SeqCst), Ordering::SeqCst);
    if OVERLAY_ENABLED.load(Ordering::SeqCst) { overlay::enable(); } else { overlay::disable(); }
}

/// Siguiente pestaña (F10).
pub fn cycle_tab() {
    let cur = CURRENT_TAB.load(Ordering::Relaxed);
    let next = if cur >= (panels::PANEL_COUNT as u8).saturating_sub(1) { 0 } else { cur + 1 };
    CURRENT_TAB.store(next, Ordering::SeqCst);
}

/// Siguiente query preset (F8).
pub fn cycle_query() {
    let cur = ACTIVE_QUERY.load(Ordering::SeqCst);
    let next = if cur >= 5 { 0 } else { cur + 1 };
    ACTIVE_QUERY.store(next, Ordering::SeqCst);
}

/// Repinta el overlay si está habilitado y dirty.
pub fn tick() {
    if !OVERLAY_ENABLED.load(Ordering::SeqCst) { return; }
    if !overlay::is_dirty() { return; }
    let tab = CURRENT_TAB.load(Ordering::SeqCst);
    overlay::paint(tab);
    overlay::clear_dirty();
}

/// Estado del overlay.
pub fn overlay_enabled() -> bool { OVERLAY_ENABLED.load(Ordering::SeqCst) }
pub fn current_tab() -> u8 { CURRENT_TAB.load(Ordering::SeqCst) }
pub fn is_ready() -> bool { BOOT_READY.load(Ordering::Relaxed) }
pub fn current_query() -> u8 { ACTIVE_QUERY.load(Ordering::SeqCst) }

/// Alias de `overlay_enabled` (compatibilidad diag).
pub fn is_overlay_enabled() -> bool { overlay_enabled() }

/// Habilita o deshabilita el overlay (compatibilidad diag).
pub fn set_overlay_enabled(enabled: bool) {
    OVERLAY_ENABLED.store(enabled, Ordering::SeqCst);
    if enabled { overlay::enable(); } else { overlay::disable(); }
}

/// Repinta el overlay (compatibilidad diag).
pub fn paint_overlay() {
    let tab = CURRENT_TAB.load(Ordering::SeqCst);
    overlay::paint(tab);
    overlay::clear_dirty();
}

// ── Snapshot del sistema (telemetría) ────────────────────────────

/// Refresca la telemetría. Llamado desde el timer tick de Ring 0.
pub fn tick_refresh() {
    telemetry::memory::set_free_pages(
        unsafe { crate::mem::phys::free_count() } as u64
    );
    telemetry::memory::add_heap_used(crate::mem::heap::heap_used() as u64);
}

/// Dump CR3 a serial (Ring 3 transition tracing).
pub fn read_cr3_into_serial() {
    let cr3: u64;
    unsafe { core::arch::asm!("mov {}, cr3", out(reg) cr3); }
    crate::dev::console::serial_write("0x");
    crate::dev::console::serial_write_u64(cr3, 16);
}

// ── Persistencia ────────────────────────────────────────────────

/// Ruta objetivo futura para el log persistente.
pub fn persistent_target_path() -> &'static str {
    persistent::TARGET_PATH
}

pub fn persistent_pending_bytes() -> usize {
    persistent::pending_bytes()
}

pub fn persistent_dropped_bytes() -> u64 {
    persistent::dropped_bytes()
}

pub fn copy_persistent_pending(out: &mut [u8]) -> usize {
    persistent::copy_pending(out)
}

pub fn ack_persistent_bytes(bytes: usize) {
    persistent::ack(bytes);
}

// ── Query activo ────────────────────────────────────────────────

/// Devuelve el query activo.
pub fn active_query() -> query::Query {
    let qid = match ACTIVE_QUERY.load(Ordering::SeqCst) {
        0 => QueryId::OnlyErrors,
        1 => QueryId::OnlyCritical,
        2 => QueryId::Kernel,
        3 => QueryId::Ring3,
        4 => QueryId::Gpu,
        _ => QueryId::All,
    };
    build_query(qid)
}

/// Devuelve el nombre del query activo.
pub fn active_query_name() -> &'static str {
    let qid = match ACTIVE_QUERY.load(Ordering::SeqCst) {
        0 => QueryId::OnlyErrors,
        1 => QueryId::OnlyCritical,
        2 => QueryId::Kernel,
        3 => QueryId::Ring3,
        4 => QueryId::Gpu,
        _ => QueryId::All,
    };
    query_id_name(qid)
}
