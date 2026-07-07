use crate::hal;

pub fn register_cabina_sink() {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.register_cabina_sink)(); }
}
