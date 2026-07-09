use bmo_boot_protocol::BootInfo;

pub static mut FB_ADDR: u64 = 0;
pub static mut FB_WIDTH: u32 = 0;
pub static mut FB_HEIGHT: u32 = 0;
pub static mut FB_STRIDE: u32 = 0;
pub static mut FB_PIXEL_FORMAT: u32 = 0;
pub static mut BOOT_INFO: *const BootInfo = core::ptr::null();
