use crate::hal;

pub fn init(tsc_freq: u64) {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.audio_init)(tsc_freq); }
}

pub fn play(tone: u32) {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.audio_play)(tone); }
}

pub fn play_logon_chime() {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.audio_play_logon_chime)(); }
}

pub fn beep(freq_hz: u32, duration_ms: u32) {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.audio_beep)(freq_hz, duration_ms); }
}

pub fn set_volume(val: u32) {
    if let Some(h) = unsafe { hal::HAL.as_ref() } { (h.audio_set_volume)(val); }
}
