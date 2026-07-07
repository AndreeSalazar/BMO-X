use crate::hal;

pub fn FONT8x16() -> &'static [u8; 4096] {
    if let Some(h) = unsafe { hal::HAL.as_ref() } {
        unsafe { &*h.FONT8x16 }
    } else {
        panic!("HAL not initialized");
    }
}
