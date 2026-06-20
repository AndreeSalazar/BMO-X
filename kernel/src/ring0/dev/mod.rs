//! Device API (Ring 0 HAL).
//!
//! Drivers de hardware (un archivo por device, plano sin sub-módulos):
//!   - console:      COM1 115200 baud (logging + debug)
//!   - pcie:         PCI Express scan
//!   - framebuffer:  UEFI GOP framebuffer + backbuffer
//!   - watchdog:     Hardware watchdog
//!   - audio:        Audio DSP math (sin/cos/exp/log)
//!   - acpi:         ACPI control (sleep, reboot) — tables en `platform`
//!
//! Cualquier driver nuevo:
//! 1. Crear `dev/<nombre>.rs` con un trait (si es genérico) + init().
//! 2. Agregar `pub mod <nombre>;` aquí.
//! 3. Si expone syscall, agregar en `crate::arch::syscall::dispatch`.

#![allow(dead_code)]

pub mod console;
pub mod pcie;
pub mod framebuffer;
pub mod watchdog;
pub mod audio;
pub mod acpi;

// ── Init orchestrator ─────────────────────────────────────────────────

/// Initialize all device drivers in dependency order.
pub fn init() {
    crate::dev::console::serial_write("[dev] === Device Init ===\n");
    crate::dev::console::init();
    crate::dev::framebuffer::init();
    crate::dev::pcie::init();
    crate::dev::watchdog::init();
    crate::dev::audio::init();
    crate::dev::acpi::init();
    crate::dev::console::serial_write("[dev] === Device Init Complete ===\n");
}
