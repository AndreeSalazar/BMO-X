//! FastOS diag/ — diagnóstico modular integrado desde Ring 0.
//!
//! Capas:
//! - `event`: tipos y severidad,
//! - `buffer`: caja negra circular en RAM,
//! - `serial_sink`: salida COM1,
//! - `overlay`: render visual GOP.

#![allow(dead_code)]

mod buffer;
mod event;
mod overlay;
mod serial_sink;

pub use event::Severity;

use event::Event;

pub fn init() {
    info("diag", "diag online: serial + GOP overlay + RAM blackbox");
}

pub fn info(module: &'static str, message: &'static str) {
    emit(Event::new(Severity::Info, module, message));
}

pub fn warn(module: &'static str, message: &'static str) {
    emit(Event::new(Severity::Warn, module, message));
}

pub fn fault(module: &'static str, message: &'static str) {
    emit(Event::new(Severity::Fault, module, message));
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

pub fn set_overlay_enabled(enabled: bool) {
    overlay::set_enabled(enabled);
}

pub fn paint_overlay() {
    overlay::paint();
}

fn emit(event: Event) {
    let event = buffer::push(event);
    serial_sink::write_event(event);
    overlay::paint();
}

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
