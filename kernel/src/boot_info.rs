//! Boot info globals — stores the BootInfo pointer from the bootloader.

/// Global pointer to the BootInfo structure passed by the bootloader.
pub static mut BOOT_INFO: *const fastos_boot_protocol::BootInfo = core::ptr::null();

/// Global GSP firmware info (populated from BootInfo for backward compat).
pub static mut GSP_FW_ADDR: u64 = 0;
pub static mut GSP_FW_SIZE: u64 = 0;
