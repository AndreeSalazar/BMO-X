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
///
/// v1.8.16: Re-enabled. Previous Opus 4.8 disabled this because
/// SetVariable was fragile on AMD firmware. The actual root cause was
/// NVRAM_ATTRS including BOOTSERVICE_ACCESS (0x02) after ExitBootServices.
/// Fixed to NON_VOLATILE | RUNTIME_ACCESS (0x05). NVRAM is MORE reliable
/// than the physical RAM marker at 0x90000 because the AMD FCH watchdog
/// clears RAM on reset but NVRAM persists.
pub fn write_boot_stage(stage: &str) -> bool {
    nvram_log::write_boot_stage(stage)
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
