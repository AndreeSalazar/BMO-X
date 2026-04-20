//! BootInfo structure passed from bootloader to kernel

/// Boot info from stage2.asm (at 0x9100).
#[repr(C)]
pub struct BootInfo {
    pub magic: u64,
    pub memory_map_addr: u64,
    pub memory_map_count: u64,
    pub cpu_features_addr: u64,
    pub framebuffer_addr: u64,
    pub kernel_start: u64,
    pub kernel_size: u64,
    pub fb_pitch: u64,
    pub vbe_mode: u64,
    pub gpu_fw_addr: u64,
    pub gpu_fw_size: u64,
}

// Global BootInfo pointer for access from tests
pub static mut BOOT_INFO_PTR: *const BootInfo = core::ptr::null();

// Global GSP firmware info (passed via registers from bootloader)
pub static mut GSP_FW_ADDR: u64 = 0;
pub static mut GSP_FW_SIZE: u64 = 0;
