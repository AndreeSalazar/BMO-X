//! Ring 0 Logger — unified logging for serial + visual overlay.
//!
//! Provides two APIs:
//! 1. `Logger` struct — per-subsystem logger (e.g. `Logger::new("pci")`)
//! 2. Module functions — `info()`, `warn()`, `fault()`, `info_u64()` for boot phases

#![allow(dead_code)]

use crate::dev::console;
use super::visual;

// ── Per-subsystem Logger ────────────────────────────────────────────────────

#[derive(Copy, Clone)]
pub struct Logger {
    subsystem: &'static str,
}

impl Logger {
    pub const fn new(subsystem: &'static str) -> Self {
        Self { subsystem }
    }

    pub fn debug(&self, msg: &str) {
        self.info(msg);
    }

    pub fn info(&self, msg: &str) {
        if visual::is_active() {
            visual::log(self.subsystem, msg, visual::color::OK);
        }
        self.write_serial(msg, "INFO");
    }

    pub fn warn(&self, msg: &str) {
        if visual::is_active() {
            visual::log(self.subsystem, msg, visual::color::WARN);
        }
        self.write_serial(msg, "WARN");
    }

    pub fn error(&self, msg: &str) {
        if visual::is_active() {
            visual::log(self.subsystem, msg, visual::color::WARN);
        }
        self.write_serial(msg, "ERROR");
    }

    pub fn error_u64(&self, msg: &str, val: u64) {
        if visual::is_active() {
            visual::log(self.subsystem, msg, visual::color::WARN);
        }
        console::serial_write("[");
        console::serial_write(self.subsystem);
        console::serial_write("] ERROR: ");
        console::serial_write(msg);
        console::serial_write(": 0x");
        console::serial_write_u64(val, 16);
        console::serial_write("\n");
    }

    pub fn info_u64(&self, msg: &str, val: u64) {
        self.info(msg);
        console::serial_write("  0x");
        console::serial_write_u64(val, 16);
        console::serial_write("\n");
    }

    pub fn fatal(&self, msg: &str) -> ! {
        if visual::is_active() {
            visual::log(self.subsystem, msg, visual::color::FAULT);
        }
        console::serial_write("[");
        console::serial_write(self.subsystem);
        console::serial_write("] FATAL: ");
        console::serial_write(msg);
        console::serial_write("\n");
        loop {
            unsafe { core::arch::asm!("hlt"); }
        }
    }

    fn write_serial(&self, msg: &str, level: &str) {
        console::serial_write("[");
        console::serial_write(self.subsystem);
        console::serial_write("] ");
        console::serial_write(level);
        console::serial_write(": ");
        console::serial_write(msg);
        console::serial_write("\n");
    }
}

pub static KERNEL: Logger = Logger::new("kernel");

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
