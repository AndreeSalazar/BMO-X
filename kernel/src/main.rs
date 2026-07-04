//! FastOS kernel — entry point único.
//!
//! Boot order:
//!   1. `_start` (ring0/mod.rs): BSS zero, guarda boot_info_ptr
//!   2. `kernel_main_real`: init temprano + breadcrumb NVRAM
//!   3. `phase_1_RING_0::main`: CPU, memoria, dispositivos, display
//!   4. Welcome shell / Desktop

#![no_std]
#![no_main]
#![allow(static_mut_refs)]

extern crate alloc;

// ── Módulos organizados ─────────────────────────────────────────────

pub mod ring0;     // HAL + boot + x86-64
pub mod bmo_core;  // BEF, shims Linux/Win32, GUI
pub mod cabina;    // Diagnóstico interno
pub mod defense;   // ByteDefender — escáner de seguridad pre-exec + runtime
pub mod timeback;  // TimeBack — checkpoints, snapshots, rollback
pub mod userland;  // Ring 3 — apps de usuario (futuro)

// Re-export para acceso directo desde cualquier módulo.
pub use ring0::*;
