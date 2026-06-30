//! CABINA wrapper: re-exports cabina_daemon + adds stub functions
//! that bmo_core expects but cabina_daemon doesn't provide.
//! TODO: implement real overlay, boot_ready, etc.

#![allow(dead_code)]

// Re-export all public items from cabina_daemon
pub use cabina_daemon::*;

/// Mark boot as ready. Stub — cabina_daemon has no equivalent.
pub fn boot_ready() {
    info("cabina", "boot_ready (stub)");
}

/// Log with a u64 value suffix.
pub fn info_u64(module: &str, msg: &str, val: u64) {
    info(module, &alloc::format!("{} {}", msg, val));
}

/// Warn with a u64 value suffix.
pub fn warn_u64(module: &str, msg: &str, val: u64) {
    warn(module, &alloc::format!("{} {}", msg, val));
}

/// Fault with a u64 value suffix.
pub fn fault_u64(module: &str, msg: &str, val: u64) {
    fault(module, &alloc::format!("{} {}", msg, val));
}

/// Trace with a u64 value suffix.
pub fn trace_u64(module: &str, msg: &str, val: u64) {
    trace(module, &alloc::format!("{} {}", msg, val));
}

static mut OVERLAY_ENABLED: bool = false;

pub fn is_overlay_enabled() -> bool {
    unsafe { OVERLAY_ENABLED }
}

pub fn set_overlay_enabled(on: bool) {
    unsafe { OVERLAY_ENABLED = on; }
}

pub fn cycle_tab() {}
pub fn cycle_query() {}

pub fn paint_overlay() {}
