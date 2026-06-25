//! UEFI Runtime Services — thin wrapper over `usb-log` crate.
//!
//! All NVRAM access goes through usb-log. This module only provides
//! the init call and re-exports for backwards compatibility.

/// Initialize usb-log with the UEFI System Table address.
pub fn init(system_table: u64) {
    usb_log::init(system_table);
    crate::dev::console::serial_write("[uefi_rt] init: delegated to usb-log crate\n");
}

/// Write boot stage to NVRAM (delegates to usb-log).
pub fn write_boot_stage(stage: &str) {
    usb_log::write_boot_stage(stage);
}

/// Read last boot stage from NVRAM (delegates to usb-log).
pub fn read_boot_stage() -> Option<alloc::string::String> {
    usb_log::read_boot_stage()
}

/// Write arbitrary NVRAM variable (delegates to usb-log).
pub fn set_variable(name: &str, data: &[u8]) -> bool {
    usb_log::set_variable(name, data)
}

/// Read arbitrary NVRAM variable (delegates to usb-log).
pub fn get_variable(name: &str) -> Option<[u8; 256]> {
    usb_log::get_variable(name)
}
