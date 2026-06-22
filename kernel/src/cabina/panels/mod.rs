//! `cabina::panels` — Cada panel del HUD en su propio archivo.
//!
//! 11 paneles disponibles, cada uno es una función pura `(Snapshot) -> ()`:
//!
//! | #  | Panel  | Contenido                                                 |
//! |----|--------|-----------------------------------------------------------|
//! | 0  | OVER   | Resumen: uptime, ints, faults, heap, sched, syscalls      |
//! | 1  | CPU    | Interrupts, faults (PF/GP/NM/DF/UD/MC), TSC, vendor/family |
//! | 2  | MEM    | Heap (used/peak/allocs/frees/fragment), pages, layout      |
//! | 3  | I/O    | PCI devices, Serial, PS/2, block I/O, network              |
//! | 4  | SCHED  | Processes, threads, ctx switches, run queues              |
//! | 5  | SYSC   | Todos los syscalls con count + nombre + categoría         |
//! | 6  | EVENT  | Log con filtros (severity, modulo, texto)                 |
//! | 7  | GPU    | RDNA4 placeholder (device, VRAM, rings)                  |
//! | 8  | BOOT   | Fases del boot, drivers, errores                           |
//! | 9  | LANG   | AOT, linker, tests, benchmarks                             |
//! | 10 | QUERY  | Smart filter: ciclo de queries, colores por capa/severity |

#![allow(dead_code)]

use crate::cabina::snapshot::Snapshot;

pub mod overview;
pub mod cpu;
pub mod mem;
pub mod io;
pub mod sched;
pub mod syscalls;
pub mod events;
pub mod gpu;
pub mod boot;
pub mod lang;
pub mod query;

/// Número total de paneles.
pub const PANEL_COUNT: usize = 11;

/// Nombres cortos (para el header del HUD).
pub const PANEL_NAMES: [&str; PANEL_COUNT] = [
    "OVER", "CPU", "MEM", "I/O", "SCHED", "SYSC", "EVENT", "GPU", "BOOT", "LANG", "QUERY",
];

/// Colores por panel (para el header).
pub const PANEL_COLORS: [u32; PANEL_COUNT] = [
    0xFFCCCCCC, // OVER: gris
    0xFF00FFAA, // CPU: verde-azul
    0xFFAAFF00, // MEM: verde-amarillo
    0xFFFFAA00, // I/O: naranja
    0xFFFFFF00, // SCHED: amarillo
    0xFF00FFFF, // SYSC: cyan
    0xFFFF00FF, // EVENT: magenta
    0xFF00FFFF, // GPU: cyan
    0xFFFF8800, // BOOT: naranja oscuro
    0xFF00FFAA, // LANG: verde-azul
    0xFF00FFAA, // QUERY: verde-azul
];

/// Renderiza el panel `tab`.
pub fn render(tab: u8, s: &Snapshot) {
    match tab {
        0 => overview::render(s),
        1 => cpu::render(s),
        2 => mem::render(s),
        3 => io::render(s),
        4 => sched::render(s),
        5 => syscalls::render(s),
        6 => events::render(s),
        7 => gpu::render(s),
        8 => boot::render(s),
        9 => lang::render(s),
        10 => query::render(s),
        _ => overview::render(s),
    }
}

/// Nombre del panel actual.
pub fn name(tab: u8) -> &'static str {
    PANEL_NAMES.get(tab as usize).copied().unwrap_or("?")
}
