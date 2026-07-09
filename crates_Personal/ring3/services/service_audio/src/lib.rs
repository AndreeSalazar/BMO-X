//! Audio Service — tone generation over PC speaker.
//!
//! Provides musical notes, simple melodies, and sound effects.
//! When HDA driver is available, this layer will mix multiple streams.

#![no_std]

use driver_audio_pc;

/// Musical note frequencies (A4 = 440 Hz).
pub mod notes {
    pub const C4: u32 = 262;
    pub const D4: u32 = 294;
    pub const E4: u32 = 330;
    pub const F4: u32 = 349;
    pub const G4: u32 = 392;
    pub const A4: u32 = 440;
    pub const B4: u32 = 494;
    pub const C5: u32 = 523;
    pub const D5: u32 = 587;
    pub const E5: u32 = 659;
    pub const F5: u32 = 698;
    pub const G5: u32 = 784;
    pub const A5: u32 = 880;
}

/// Play a single note for `ms` milliseconds.
pub fn note(freq: u32, ms: u32) {
    driver_audio_pc::beep(freq, ms);
}

/// Play the boot chime.
pub fn boot_chime() {
    driver_audio_pc::chime_boot();
}

/// Click sound for UI feedback.
pub fn ui_click() {
    driver_audio_pc::beep_ok();
}

/// Error sound.
pub fn ui_error() {
    driver_audio_pc::beep_err();
}
