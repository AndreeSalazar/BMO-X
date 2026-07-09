//! CPU Vendor Profile — hardware-specific detection, configuration, and patching.
//!
//! v1.8.8: AMD Ryzen 5 5600X (Vermeer, Zen 3, Family 19h Model 01h).
//! Future: Intel Alder Lake, ARM Cortex-X, etc.

#![no_std]

pub mod amd;

/// ── Diagnostic logging callbacks (set by kernel at init) ────────────────
///
/// When linked into the kernel, these function pointers are set to the real
/// `crate::dev::console::serial_write` etc. When running standalone (e.g. in
/// a test harness or bootloader), they default to no-ops.
pub static mut LOG_WRITE_STR: Option<fn(&str)> = None;
pub static mut LOG_WRITE_U64: Option<fn(u64, usize)> = None;
pub static mut LOG_BOOT_STAGE: Option<fn(&str)> = None;

/// Wrapper used internally by the vendor modules. Replacements for:
/// - `crate::dev::console::serial_write`  → `serial_write`
/// - `crate::serial_write_u64` → `serial_write_u64`
/// - `crate::write_boot_stage` → `write_boot_stage`

#[inline]
pub fn serial_write(s: &str) {
    if let Some(f) = unsafe { LOG_WRITE_STR } { f(s); }
}

#[inline]
pub fn serial_write_u64(v: u64, radix: usize) {
    if let Some(f) = unsafe { LOG_WRITE_U64 } { f(v, radix); }
}

#[inline]
pub fn write_boot_stage(s: &str) {
    if let Some(f) = unsafe { LOG_BOOT_STAGE } { f(s); }
}
