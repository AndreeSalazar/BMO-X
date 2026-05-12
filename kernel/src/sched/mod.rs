//! Scheduler de FastOS.
//!
//! Spec: `FastOS_Scheduler_Spec.md`. Diseñado para juegos: prioridad
//! realtime para threads de audio/input/render, scheduling consciente de
//! núcleos y CCX del Ryzen 5 5600X (1 CCD × 6 cores × 2 threads).

#![allow(dead_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priority {
    /// Audio/input: garantía de latencia sub-ms.
    Realtime,
    /// Render thread del juego.
    HighGame,
    /// Threads de juego normales.
    Game,
    /// Apps interactivas (UI).
    Interactive,
    /// Background.
    Idle,
}

#[derive(Debug, Clone, Copy)]
pub struct ThreadId(pub u64);

#[derive(Debug, Clone, Copy)]
pub struct CoreAffinity {
    /// Bitmask de los 12 hilos del 5600X (0..=11).
    pub mask: u16,
}

impl CoreAffinity {
    pub const ANY: Self = Self { mask: 0x0FFF };
    /// Cores físicos solamente (sin SMT) — mejor para threads sensibles a latencia.
    pub const PHYSICAL_ONLY: Self = Self { mask: 0b0000_0101_0101_0101 };
}

pub fn yield_now() {
    // TODO: integrar con APIC timer + saved register state.
    core::hint::spin_loop();
}
