use crate::hal;

pub fn rdtsc() -> u64 {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.rdtsc)() } else { 0 }
}

pub fn tsc_per_sec() -> u64 {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.tsc_per_sec)() } else { 0 }
}

pub fn busy_wait_ms(ms: u64) {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.busy_wait_ms)(ms); }
}

pub fn halt() {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.halt)(); }
}

pub mod tsc {
    use crate::hal;
    pub fn calibrate() -> u64 { if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.tsc_calibrate)() } else { 0 } }
    pub fn busy_wait_ms(ms: u64, freq: u64) { if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.tsc_busy_wait_ms)(ms, freq); } }
}
