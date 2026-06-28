//! Ring 0 Logger — module-level logging for boot phases.

#![allow(dead_code)]

use crate::dev::console;
use super::visual;

// ── Boot phase logging (module-level functions) ─────────────────────────────

pub fn info(phase: &'static str, msg: &'static str) {
    visual::log(phase, msg, visual::color::OK);
    console::serial_write("[FastOS] ");
    console::serial_write(msg);
    console::serial_write("\n");
}

pub fn warn(phase: &'static str, msg: &'static str) {
    visual::log(phase, msg, visual::color::WARN);
    console::serial_write("[FastOS] WARN: ");
    console::serial_write(msg);
    console::serial_write("\n");
}

pub fn fault(phase: &'static str, msg: &'static str) -> ! {
    visual::log(phase, msg, visual::color::FAULT);
    console::serial_write("[FastOS] FATAL: ");
    console::serial_write(msg);
    console::serial_write("\n");
    loop {
        unsafe { core::arch::asm!("hlt"); }
    }
}

pub fn info_u64(phase: &'static str, msg: &'static str, val: u64) {
    visual::log(phase, msg, visual::color::OK);
    console::serial_write("[FastOS] ");
    console::serial_write(msg);
    console::serial_write(": ");
    super::serial::hex(val);
    console::serial_write("\n");
}
