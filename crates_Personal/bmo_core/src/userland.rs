use crate::hal;

pub fn init() {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.userland_init)(); }
}
