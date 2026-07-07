use crate::hal;

pub fn tick() {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.hud_tick)(); }
}
