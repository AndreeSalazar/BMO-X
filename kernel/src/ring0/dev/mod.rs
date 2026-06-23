//! Device API (Ring 0 HAL).
//!
//! Drivers de hardware (un archivo por device, plano sin sub-módulos):
//!   - console:      COM1 115200 baud (logging + debug)
//!   - pcie:         PCI Express scan
//!   - framebuffer:  UEFI GOP framebuffer + backbuffer
//!   - watchdog:     Hardware watchdog
//!   - audio:        Audio DSP math (sin/cos/exp/log) — DEPRECATED, mover a bmo_core
//!   - acpi:         ACPI control (sleep, reboot) — tables en `platform`
//!
//! Cualquier driver nuevo:
//! 1. Crear `dev/<nombre>.rs` con un trait (si es genérico) + init().
//! 2. Agregar `pub mod <nombre>;` aquí.
//! 3. Si expone syscall, agregar en `crate::arch::syscall::dispatch`.
//!
//! NOTA v1.8.7: el orquestador `dev::init()` se eliminó. Los drivers se
//! inicializan directamente desde `boot::phases::p2_dev::run` y desde
//! `coordinator::main` con su orden de dependencia real.

#![allow(dead_code)]

pub mod console;
pub mod pcie;
pub mod framebuffer;
pub mod watchdog;
pub mod acpi;
