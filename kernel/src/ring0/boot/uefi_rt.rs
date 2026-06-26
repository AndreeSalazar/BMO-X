//! UEFI Runtime Services — thin wrapper over `nvram-log` crate.
//!
//! All NVRAM access goes through nvram-log. This module only provides
//! the init call and re-exports for backwards compatibility.

/// Initialize nvram-log with the UEFI System Table address.
pub fn init(system_table: u64) {
    nvram_log::init(system_table);
    crate::dev::console::serial_write("[uefi_rt] init: delegated to nvram-log crate\n");
}

/// Write boot stage to NVRAM (delegates to nvram-log).
pub fn write_boot_stage(stage: &str) {
    nvram_log::write_boot_stage(stage);
}

/// Read last boot stage from NVRAM (delegates to nvram-log).
pub fn read_boot_stage() -> Option<alloc::string::String> {
    nvram_log::read_boot_stage()
}

/// Write arbitrary NVRAM variable (delegates to nvram-log).
pub fn set_variable(name: &str, data: &[u8]) -> bool {
    nvram_log::set_variable(name, data)
}

/// Read arbitrary NVRAM variable (delegates to nvram-log).
pub fn get_variable(name: &str) -> Option<[u8; 256]> {
    nvram_log::get_variable(name)
}
