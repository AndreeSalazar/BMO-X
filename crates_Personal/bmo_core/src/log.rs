use crate::hal;

pub fn log_write(level: u8, msg: &str) {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.log_write)(level, msg); }
}
