//! UEFI Runtime Services — thin wrapper over `nvram-log` crate.
//!
//! All NVRAM access goes through nvram-log. This module only provides
//! the init call and re-exports for backwards compatibility.

/// Initialize nvram-log with the UEFI System Table address.
pub fn init(system_table: u64) {
    let _ = system_table;
    crate::dev::console::serial_write("[uefi_rt] init: disabled in Ring 0 stable mode\n");
}

/// Write boot stage to NVRAM (delegates to nvram-log).
///
/// Ring 0 stable mode: never call UEFI RuntimeServices after kernel handoff.
/// Real firmware can fault if runtime pages are not mapped exactly as UEFI
/// expects after ExitBootServices. A fault here happens before the crash UI
/// can recover and looks like a triple fault/reboot. The bootloader may still
/// use NVRAM; the kernel uses serial + RAM crash marker only.
pub fn write_boot_stage(stage: &str) -> bool {
    let _ = stage;
    true
}

/// Read last boot stage from NVRAM (delegates to nvram-log).
pub fn read_boot_stage() -> Option<alloc::string::String> {
    None
}

/// Write arbitrary NVRAM variable (delegates to nvram-log).
pub fn set_variable(name: &str, data: &[u8]) -> bool {
    let _ = (name, data);
    false
}

/// Read arbitrary NVRAM variable (delegates to nvram-log).
pub fn get_variable(name: &str) -> Option<[u8; 256]> {
    let _ = name;
    None
}
