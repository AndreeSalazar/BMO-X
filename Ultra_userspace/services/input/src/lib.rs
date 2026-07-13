//! Input service — multiplexes keyboard/mouse to focused window.
//!
//! In the Ring 3 base, this is a stub. The Ring 0 keyboard/mouse
//! drivers (in `Ultra_kernel_x86-64/kernel/src/ring0/irq/`) push scancodes
//! into a ring buffer; this service will poll them and dispatch
//! to the focused process via IPC.

#![no_std]

pub fn init() {}
pub fn poll_keyboard() -> Option<u8> { None }
pub fn poll_mouse() -> Option<u32> { None }
