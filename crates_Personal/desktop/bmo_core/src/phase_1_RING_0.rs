use crate::hal;

pub fn write_crash_marker(marker: u32) {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.write_crash_marker)(marker); }
}
pub fn clear_crash_marker() {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.clear_crash_marker)(); }
}
