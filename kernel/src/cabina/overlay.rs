//! `cabina::overlay` — HUD visual GOP con pestañas.
//!
//! v1.8.8: el overlay es un **cliente** de `cabina::snapshot`. Solo
//! lee snapshots (nunca escribe). El repintado está limitado por
//! `dirty flag` para evitar repintar en cada evento.

#![allow(dead_code)]

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use crate::cabina::snapshot::take;

static ENABLED: AtomicBool = AtomicBool::new(false);
static DIRTY: AtomicBool = AtomicBool::new(false);
static LAST_PAINT_MS: AtomicU64 = AtomicU64::new(0);

/// Habilita el overlay.
pub fn enable() {
    ENABLED.store(true, Ordering::SeqCst);
    DIRTY.store(true, Ordering::SeqCst);
}

/// Deshabilita el overlay.
pub fn disable() {
    ENABLED.store(false, Ordering::SeqCst);
}

/// Marca el overlay como "necesita repintar".
/// Llamar desde `cabina::emit`.
pub fn mark_dirty() {
    DIRTY.store(true, Ordering::SeqCst);
}

/// Limpia el flag dirty.
pub fn clear_dirty() {
    DIRTY.store(false, Ordering::SeqCst);
}

/// `true` si está habilitado.
pub fn is_enabled() -> bool { ENABLED.load(Ordering::Relaxed) }

/// `true` si necesita repintar.
pub fn is_dirty() -> bool { DIRTY.load(Ordering::Relaxed) }

/// Repinta el overlay con la pestaña `tab`.
/// Esta función es **costosa** (~ms). Solo llamar desde el timer.
pub fn paint(tab: u8) {
    if !is_enabled() { return; }
    let s = take();
    crate::cabina::panels::render(tab, &s);
    LAST_PAINT_MS.store({
        let tsc = crate::cpu::rdtsc();
        let freq = crate::cpu::tsc_per_sec();
        if freq == 0 { 0 } else { (tsc.wrapping_mul(1_000_000_000) / freq) / 1_000_000 }
    }, Ordering::SeqCst);
}

