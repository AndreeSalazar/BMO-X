//! `ring0::diag_min` — Diagnóstico mínimo para emergencias.
//!
//! v1.8.8: este módulo se usa cuando el kernel está en pánico o en
//! estado crítico. **No aloca, no usa el framebuffer, solo serial**.
//!
//! ## Componentes
//!
//! - `blackbox`: copia mínima de los últimos N eventos en un buffer fijo.
//! - `panic_view`: imprime el panic con formato legible al serial.
//! - `serial`: helpers para escribir al COM1 sin locks.

#![allow(dead_code)]

pub mod blackbox;
pub mod panic_view;
pub mod serial;

const BLACKBOX_MIN: usize = 32;

/// Inicializa el subsistema mínimo de diagnóstico.
pub fn init() {
    blackbox::init();
    serial::init();
}

/// Reporta un evento crítico (panic, double fault, etc.) al serial.
pub fn report_critical(module: &str, msg: &str) {
    serial::write("[CRIT] ");
    serial::write(module);
    serial::write(": ");
    serial::write(msg);
    serial::write("\n");
}
