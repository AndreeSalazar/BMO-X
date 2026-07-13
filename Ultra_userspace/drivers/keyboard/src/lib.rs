//! Keyboard driver (Ring 3) — stub.
//!
//! The Ring 0 keyboard driver (`Ultra_kernel_x86-64/kernel/src/ring0/irq/keyboard.rs`)
//! pushes scancodes into a shared ring buffer. This userland driver
//! reads them via syscall and translates to X11/Wayland-style key events.

#![no_std]

pub fn init() {}
pub fn pump_events() -> usize { 0 }
