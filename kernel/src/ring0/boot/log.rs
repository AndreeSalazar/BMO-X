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
use crate::dev::console;
use super::visual;

/// Log an info-level boot message. `phase` and `msg` must be `'static`.
pub fn info(phase: &'static str, msg: &'static str) {
    visual::log(phase, msg, visual::color::OK);
    diag::info(phase, msg);
    console::serial_write("[FastOS] ");
    console::serial_write(msg);
    console::serial_write("\n");
}

/// Log a recoverable warning.
pub fn warn(phase: &'static str, msg: &'static str) {
    visual::log(phase, msg, visual::color::WARN);
    diag::warn(phase, msg);
    console::serial_write("[FastOS] WARN: ");
    console::serial_write(msg);
    console::serial_write("\n");
}

/// Log an unrecoverable fault and halt. Never returns.
pub fn fault(phase: &'static str, msg: &'static str) -> ! {
    visual::log(phase, msg, visual::color::FAULT);
    diag::fault(phase, msg);
    console::serial_write("[FastOS] FATAL: ");
    console::serial_write(msg);
    console::serial_write("\n");
    loop {
        unsafe { core::arch::asm!("hlt"); }
    }
}

/// Log an info message plus a u64 value rendered as 0xHEX.
pub fn info_u64(phase: &'static str, msg: &'static str, val: u64) {
    visual::log(phase, msg, visual::color::OK);
    diag::info_u64(phase, msg, val);
    console::serial_write("[FastOS] ");
    crate::dev::console::serial_write(msg);
    crate::dev::console::serial_write(": ");
    super::serial::hex(val);
    console::serial_write("\n");
}
