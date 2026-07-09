use crate::hal;

pub fn pet_fch_watchdog() {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.pet_fch_watchdog)(); }
}
pub fn disarm() {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.watchdog_disarm)(); }
}
