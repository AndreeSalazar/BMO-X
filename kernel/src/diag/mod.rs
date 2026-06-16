//! FastOS diag/ — diagnóstico modular integrado desde Ring 0.
//!
//! Capas:
//! - `event`: tipos y severidad,
//! - `buffer`: caja negra circular en RAM (256 eventos),
//! - `serial_sink`: salida COM1,
//! - `overlay`: render visual GOP con pestañas omniscientes,
//! - `telemetry`: contadores atómicos en tiempo real.

#![allow(dead_code)]

mod buffer;
mod event;
mod overlay;
mod serial_sink;
pub mod telemetry;

pub use event::Severity;

use event::Event;

// ── Tab system for overlay ─────────────────────────────────────────

/// Which panel the overlay is showing.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum OverlayTab {
    /// Main overview (CPU, Memory, Tasks, GOP).
    Overview = 0,
    /// CPU telemetry (interrupts, faults, frequency).
    Cpu = 1,
    /// Memory telemetry (allocs, frees, heap, fragmentation).
    Memory = 2,
    /// I/O telemetry (PCI, serial, PS/2).
    Io = 3,
    /// Scheduler telemetry (context switches, queues).
    Scheduler = 4,
    /// Event log (last 256 events).
    Log = 5,
}

impl OverlayTab {
    pub const ALL: [OverlayTab; 6] = [
        OverlayTab::Overview,
        OverlayTab::Cpu,
        OverlayTab::Memory,
        OverlayTab::Io,
        OverlayTab::Scheduler,
        OverlayTab::Log,
    ];

    pub fn next(self) -> Self {
        match self {
            OverlayTab::Overview => OverlayTab::Cpu,
            OverlayTab::Cpu => OverlayTab::Memory,
            OverlayTab::Memory => OverlayTab::Io,
            OverlayTab::Io => OverlayTab::Scheduler,
            OverlayTab::Scheduler => OverlayTab::Log,
            OverlayTab::Log => OverlayTab::Overview,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            OverlayTab::Overview => "Overview",
            OverlayTab::Cpu => "CPU",
            OverlayTab::Memory => "Memory",
            OverlayTab::Io => "I/O",
            OverlayTab::Scheduler => "Scheduler",
            OverlayTab::Log => "Log",
        }
    }
}

static mut CURRENT_TAB: OverlayTab = OverlayTab::Overview;

/// Cycle to the next overlay tab.
pub fn cycle_tab() {
    unsafe {
        CURRENT_TAB = CURRENT_TAB.next();
    }
}

/// Get the current overlay tab.
pub fn current_tab() -> OverlayTab {
    unsafe { CURRENT_TAB }
}

// ── Init ───────────────────────────────────────────────────────────

pub fn init() {
    info("diag", "diag online: serial + GOP overlay + RAM blackbox");
}

// ── Event emission ─────────────────────────────────────────────────

pub fn info(module: &'static str, message: &'static str) {
    emit(Event::new(Severity::Info, module, message));
}

pub fn warn(module: &'static str, message: &'static str) {
    emit(Event::new(Severity::Warn, module, message));
}

pub fn fault(module: &'static str, message: &'static str) {
    emit(Event::new(Severity::Fault, module, message));
}

pub fn trace(module: &'static str, message: &'static str) {
    emit(Event::new(Severity::Trace, module, message));
}

pub fn trace_u64(module: &'static str, message: &'static str, value: u64) {
    emit(Event::new_u64(Severity::Trace, module, message, value));
}

pub fn panic_event(module: &'static str, message: &'static str) {
    emit(Event::new(Severity::Panic, module, message));
}

pub fn info_u64(module: &'static str, message: &'static str, value: u64) {
    emit(Event::new_u64(Severity::Info, module, message, value));
}

pub fn warn_u64(module: &'static str, message: &'static str, value: u64) {
    emit(Event::new_u64(Severity::Warn, module, message, value));
}

pub fn fault_u64(module: &'static str, message: &'static str, value: u64) {
    emit(Event::new_u64(Severity::Fault, module, message, value));
}

pub fn event(severity: Severity, module: &'static str, message: &'static str) {
    emit(Event::new(severity, module, message));
}

pub fn event_u64(severity: Severity, module: &'static str, message: &'static str, value: u64) {
    emit(Event::new_u64(severity, module, message, value));
}

// ── Overlay control ────────────────────────────────────────────────

pub fn set_overlay_enabled(enabled: bool) {
    overlay::set_enabled(enabled);
}

pub fn is_overlay_enabled() -> bool {
    overlay::is_enabled()
}

pub fn paint_overlay() {
    overlay::paint();
}

// ── Periodic refresh (called from APIC timer tick) ─────────────────
//
// Called every timer tick.  The overlay only repaints at REFRESH_HZ
// to avoid burning CPU on every 10ms tick.

/// Refresh rate for the overlay (Hz).
pub const OVERLAY_REFRESH_HZ: u64 = 4; // 4 Hz = 250ms between repaints

/// Called every timer tick. Only updates telemetry counters.
/// The overlay is repainted on diag events and explicit paint_overlay() calls.
pub fn tick_refresh() {
    // Update telemetry snapshots (lightweight — no framebuffer access from IRQ)
    telemetry::t().mem.update_free_pages(
        unsafe { crate::arch::page_alloc::free_count() } as u64
    );
    telemetry::t().mem.update_heap(crate::allocator::heap_used() as u64);
}

// ── Private ────────────────────────────────────────────────────────

fn emit(event: Event) {
    let event = buffer::push(event);
    serial_sink::write_event(event);
    overlay::paint();
}

// ── Macros ─────────────────────────────────────────────────────────

#[macro_export]
macro_rules! diag_info {
    ($module:expr, $message:expr) => {
        $crate::diag::info($module, $message)
    };
}

#[macro_export]
macro_rules! diag_warn {
    ($module:expr, $message:expr) => {
        $crate::diag::warn($module, $message)
    };
}

#[macro_export]
macro_rules! diag_fault {
    ($module:expr, $message:expr) => {
        $crate::diag::fault($module, $message)
    };
}
