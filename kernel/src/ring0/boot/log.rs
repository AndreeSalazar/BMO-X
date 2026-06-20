//! Boot logger — single path for diagnostic + serial + visual overlay output.
//!
//! All kernel boot messages must flow through this module. There is exactly
//! one call per logical event: `info`, `warn`, `fault`, `info_u64`.
//!
//! - `info`     : normal phase progress, green text in overlay
//! - `warn`     : recoverable issue, yellow text
//! - `fault`    : unrecoverable, halts the CPU with a red overlay
//! - `info_u64` : info + a u64 value rendered as decimal or hex
//!
//! Each call goes to:
//!   1. `diag`        — kernel log buffer (for `dmesg` / post-boot inspection)
//!   2. `serial`      — COM1 (hardware capture during early boot)
//!   3. `visual`      — GOP framebuffer overlay (only if framebuffer is up)

use crate::bmo_core::diag;
use crate::device::serial;
use super::visual;

/// Log an info-level boot message. `phase` and `msg` must be `'static`.
pub fn info(phase: &'static str, msg: &'static str) {
    visual::log(phase, msg, visual::color::OK);
    diag::info(phase, msg);
    serial::serial_write("[FastOS] ");
    serial::serial_write(msg);
    serial::serial_write("\n");
}

/// Log a recoverable warning.
pub fn warn(phase: &'static str, msg: &'static str) {
    visual::log(phase, msg, visual::color::WARN);
    diag::warn(phase, msg);
    serial::serial_write("[FastOS] WARN: ");
    serial::serial_write(msg);
    serial::serial_write("\n");
}

/// Log an unrecoverable fault and halt. Never returns.
pub fn fault(phase: &'static str, msg: &'static str) -> ! {
    visual::log(phase, msg, visual::color::FAULT);
    diag::fault(phase, msg);
    serial::serial_write("[FastOS] FATAL: ");
    serial::serial_write(msg);
    serial::serial_write("\n");
    loop {
        unsafe { core::arch::asm!("hlt"); }
    }
}

/// Log an info message plus a u64 value rendered as 0xHEX.
pub fn info_u64(phase: &'static str, msg: &'static str, val: u64) {
    visual::log(phase, msg, visual::color::OK);
    diag::info_u64(phase, msg, val);
    serial::serial_write("[FastOS] ");
    serial::serial_write(msg);
    serial::serial_write(": ");
    super::serial::hex(val);
    serial::serial_write("\n");
}
