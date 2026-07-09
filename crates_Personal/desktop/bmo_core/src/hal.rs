//! HAL bridge — re-exports bmo-hal-defs types and manages the global HAL instance.

pub use bmo_hal_defs::{HalServices, InputEvent, InputEventKind, ModuleHeader, ModuleInitRegs, FMOD_MAGIC};

pub static mut HAL: Option<HalServices> = None;

pub fn init(h: HalServices) {
    unsafe {
        crate::info::FB_ADDR = h.fb_addr;
        crate::info::FB_WIDTH = h.fb_width;
        crate::info::FB_HEIGHT = h.fb_height;
        crate::info::FB_STRIDE = h.fb_stride;
        crate::info::FB_PIXEL_FORMAT = h.fb_pixel_format;
        crate::info::BOOT_INFO = h.boot_info as *const bmo_boot_protocol::BootInfo;
        HAL = Some(h);
    }
}
