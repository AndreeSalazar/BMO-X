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

use crate::{diag, drivers::serial};
use super::{serial as boot_serial, visual};

/// Log an info-level boot message. `phase` and `msg` must be `'static`.
pub fn info(phase: &'static str, msg: &'static str) {
    diag::info(phase, msg);
    serial::serial_write("[FastOS] ");
    serial::serial_write(msg);
    serial::serial_write("\n");
    visual::log(phase, msg, visual::color::OK);
}

/// Log a recoverable warning.
pub fn warn(phase: &'static str, msg: &'static str) {
    diag::warn(phase, msg);
    serial::serial_write("[FastOS] WARN: ");
    serial::serial_write(msg);
    serial::serial_write("\n");
    visual::log(phase, msg, visual::color::WARN);
}

/// Log an unrecoverable fault and halt. Never returns.
pub fn fault(phase: &'static str, msg: &'static str) -> ! {
    diag::fault(phase, msg);
    serial::serial_write("[FastOS] FATAL: ");
    serial::serial_write(msg);
    serial::serial_write("\n");
    visual::log(phase, msg, visual::color::FAULT);
    loop {
        unsafe { core::arch::asm!("hlt"); }
    }
}

/// Log an info message plus a u64 value rendered as 0xHEX.
pub fn info_u64(phase: &'static str, msg: &'static str, val: u64) {
    diag::info_u64(phase, msg, val);
    serial::serial_write("[FastOS] ");
    serial::serial_write(msg);
    serial::serial_write(": ");
    boot_serial::hex(val);
    serial::serial_write("\n");
}
