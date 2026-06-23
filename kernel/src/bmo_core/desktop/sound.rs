//! Sound — PC speaker beep via bmo_audio.

/// Play a tone at `freq_hz` for `duration_ms` milliseconds using the safe bmo_audio driver.
pub fn beep(freq_hz: u32, duration_ms: u32) {
    bmo_audio::beep(freq_hz, duration_ms);
}
