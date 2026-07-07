use crate::hal;

pub fn serial_write(s: &str) {
    if let Some(h) = unsafe { hal::HAL.as_ref() } {
        (h.serial_write)(s);
    }
}

pub fn serial_write_u64(v: u64, w: usize) {
    if let Some(h) = unsafe { hal::HAL.as_ref() } {
        (h.serial_write_u64)(v, w);
    }
}
