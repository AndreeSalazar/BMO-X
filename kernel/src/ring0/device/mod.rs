//! Device API (Ring 0 HAL).
//!
//! Drivers de hardware:
//!   - serial:   COM1 115200 baud (logging + debug)
//!   - pci:      PCI IO-port scan (ECAM disabled en este build)
//!   - gop:      UEFI GOP framebuffer
//!   - watchdog: Hardware watchdog (pet'd desde IDT)
//!   - audio:    Audio dsp math (dsp_sin usado por gustos FM synth)
//!   - acpi:     RSDP, MCFG parser
//!
//! Cualquier driver nuevo:
//! 1. Crear `device/<nombre>.rs` con init() + read/write/etc.
//! 2. Agregar `pub mod <nombre>;` aquí.
//! 3. Si expone syscall, agregar en crate::interrupt::syscall.

#![allow(dead_code)]

pub mod serial;
pub mod pci;
pub mod gop;
pub mod watchdog;
pub mod audio;
pub mod acpi;
