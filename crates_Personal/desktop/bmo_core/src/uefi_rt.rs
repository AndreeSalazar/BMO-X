use crate::hal;

pub fn write_boot_stage(s: &str) {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.write_boot_stage)(s); }
}
