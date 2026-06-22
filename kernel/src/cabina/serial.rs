//! `cabina::serial` — Sink serial COM1 para eventos.
//!
//! Formato: `[SEVERITY] module: msg (0xVALUE)\n`
//!
//! Si el serial no está inicializado (early boot), los eventos se
//! descartan silenciosamente.

#![allow(dead_code)]

use crate::cabina::event::{Event, Severity};

static SERIAL_READY: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);

/// Inicializa el sink serial.
pub fn init() {
    SERIAL_READY.store(true, core::sync::atomic::Ordering::SeqCst);
}

/// Escribe un evento al serial.
pub fn write_event(ev: &Event) {
    if !SERIAL_READY.load(core::sync::atomic::Ordering::Relaxed) { return; }
    write_raw(ev.severity, &ev.module, &ev.msg);
}

/// Escribe formato crudo.
pub fn write_raw(severity: Severity, module: &str, msg: &str) {
    if !SERIAL_READY.load(core::sync::atomic::Ordering::Relaxed) { return; }
    let line = format_line(severity, module, msg);
    crate::dev::console::serial_write(&line);
}

/// Formatea una línea.
fn format_line(severity: Severity, module: &str, msg: &str) -> alloc::string::String {
    alloc::format!("[{}] {}: {}\n", severity.name(), module, msg)
}
