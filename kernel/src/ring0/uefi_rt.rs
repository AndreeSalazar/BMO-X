//! UEFI Runtime Services — thin wrapper over `nvram-log` crate.
//!
//! All NVRAM access goes through nvram-log. This module only provides
//! the init call and re-exports for backwards compatibility.
//!
//! SAFETY: NVRAM writes are safe during early boot (before phase1_mem)
//! because UEFI's identity-mapping is still active. After the kernel
//! sets up its own page tables, UEFI Runtime Services physical addresses
//! may not be mapped. We write NVRAM as early as possible and accept
//! that late writes may fault on some firmware.

/// Initialize nvram-log with the UEFI System Table address.
pub fn init(system_table: u64) {
    nvram_log::init(system_table);
    crate::dev::console::serial_write("[uefi_rt] init: nvram-log initialized\n");
}

/// Write boot stage to NVRAM (delegates to nvram-log).
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
