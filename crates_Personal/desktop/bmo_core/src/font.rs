use crate::hal;

/// Built-in fallback font (8x16, all zeros = blank).
/// Used when HAL is not initialized yet.
static FALLBACK_FONT: [u8; 4096] = [0; 4096];

pub fn FONT8x16() -> &'static [u8; 4096] {
    if let Some(h) = unsafe { hal::HAL.as_ref() } {
        unsafe { &*h.FONT8x16 }
    } else {
        &FALLBACK_FONT
    }
}
