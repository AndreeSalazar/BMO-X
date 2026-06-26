//! Per-subsystem logger.
//!
//! Each driver, bus, or kernel subsystem creates a `Logger` with its name
//! and uses it for all messages:
//!
//! ```ignore
//! use crate::log::Logger;
//! static LOG: Logger = Logger::new("amdgpu");
//!
//! fn probe() {
//!     LOG.info("probing PCI device 1002:73bf");
//! }
//! ```
//!
//! Every message goes to three sinks in order:
//!   1. BMO Core's diag buffer (for `dmesg` and post-boot inspection)
//!   2. COM1 serial (for hardware capture during early boot)
//!   3. GOP framebuffer overlay (only if the framebuffer is up)
//!
//! The visual overlay is suppressed after the desktop is up to avoid
//! flickering; only serial + diag are kept.

#![allow(dead_code)]

use crate::cabina as diag;
use crate::dev::console;
use super::boot::visual;

/// A logger bound to a subsystem name. Cheap to construct (zero-sized).
#[derive(Copy, Clone)]
pub struct Logger {
    subsystem: &'static str,
}

impl Logger {
    pub const fn new(subsystem: &'static str) -> Self {
        Self { subsystem }
    }

    /// Log at debug level. Currently aliases to `info` (debug filtering
    /// will be added in v1.8.0).
    pub fn debug(&self, msg: &str) {
        self.info(msg);
    }

    /// Log at info level.
    pub fn info(&self, msg: &str) {
        if visual::is_active() {
            visual::log(self.subsystem, msg, visual::color::OK);
        }
        diag::info(self.subsystem, msg);
        self.write_serial(msg, "INFO");
    }

    /// Log a warning.
    pub fn warn(&self, msg: &str) {
        if visual::is_active() {
            visual::log(self.subsystem, msg, visual::color::WARN);
        }
        diag::warn(self.subsystem, msg);
        self.write_serial(msg, "WARN");
    }

    /// Log an error (recoverable, but the operation failed).
    pub fn error(&self, msg: &str) {
        if visual::is_active() {
            visual::log(self.subsystem, msg, visual::color::WARN);
        }
        diag::fault(self.subsystem, msg);
        self.write_serial(msg, "ERROR");
    }

    /// Log an error with a u64 value (typically an address, IRQ, or
    /// register value).
    pub fn error_u64(&self, msg: &str, val: u64) {
        if visual::is_active() {
            visual::log(self.subsystem, msg, visual::color::WARN);
        }
        diag::fault_u64(self.subsystem, msg, val);
        console::serial_write("[");
        console::serial_write(self.subsystem);
        console::serial_write("] ERROR: ");
        console::serial_write(msg);
        console::serial_write(": 0x");
        console::serial_write_u64(val, 16);
        console::serial_write("\n");
    }

    /// Log info with a u64 value.
    pub fn info_u64(&self, msg: &str, val: u64) {
        self.info(msg);
        console::serial_write("  0x");
        console::serial_write_u64(val, 16);
        console::serial_write("\n");
    }

    /// Log a fatal error and halt. Use only for unrecoverable failures.
    pub fn fatal(&self, msg: &str) -> ! {
        if visual::is_active() {
            visual::log(self.subsystem, msg, visual::color::FAULT);
        }
        diag::fault(self.subsystem, msg);
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

// ── Built-in loggers ────────────────────────────────────────────────────────

/// The catch-all "kernel" logger. Used by code that has no more specific
/// subsystem (e.g. boot phases).
pub static KERNEL: Logger = Logger::new("kernel");

// ── Compatibility shim: legacy `boot::log::*` API ─────────────────────────

/// Backwards-compatible re-export of the old boot::log API. Existing
/// callsites continue to work; new code should use `crate::log::Logger`.
pub mod compat {
    use super::visual;

    pub fn info(phase: &'static str, msg: &'static str) {
        KERNEL_FOR(phase).info(msg);
    }
    pub fn warn(phase: &'static str, msg: &'static str) {
        KERNEL_FOR(phase).warn(msg);
    }
    pub fn fault(phase: &'static str, msg: &'static str) -> ! {
        KERNEL_FOR(phase).fatal(msg);
    }
    pub fn info_u64(phase: &'static str, msg: &'static str, val: u64) {
        KERNEL_FOR(phase).info_u64(msg, val);
    }

    /// Create a logger on the fly for legacy code that passes `phase` as
    /// a `&'static str`. The logger is constructed each call; this is
    /// fine for boot-time use (low frequency) but not for hot paths.
    fn KERNEL_FOR(phase: &'static str) -> Logger {
        Logger::new(phase)
    }

    // Re-export the visual module so old code can keep using
    // `boot::log::visual::*` paths.
    pub use super::super::boot::visual as visual_rebind;
}
