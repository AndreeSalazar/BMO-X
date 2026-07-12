//! PC Speaker Driver — beep via SYS_BEEP.
//!
//! Uses the PIT channel 2 connected to the internal speaker.
//! Basic square wave at any frequency.

#![no_std]

use ring3_foundation;

/// Play a beep at `freq` Hz for `duration_ms` milliseconds.
pub fn beep(freq: u32, duration_ms: u32) {
    ring3_foundation::sys_beep(freq, duration_ms);
}

/// Play a short confirmation tone.
pub fn beep_ok() {
    beep(880, 80);
}

/// Play a short error tone.
pub fn beep_err() {
    beep(220, 200);
}

/// Boot chime — ascending arpeggio.
pub fn chime_boot() {
    beep(523, 60);  // C5
    beep(659, 60);  // E5
    beep(784, 100); // G5
}
