//! PC Speaker — PIT channel 2 beeper.
//!
//! Uses the Programmable Interval Timer (PIT) channel 2 to generate
//! a square wave at a given frequency, routed to the internal speaker
//! via port 0x61 bit 0.
//!
//! ## Usage
//!
//! ```rust
//! pc_speaker::beep(1000, 200); // 1 kHz for 200 ms
//! ```

use core::arch::asm;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

const PIT_FREQ: u32 = 1_193_182;

/// Milliseconds remaining on the current beep. Timer ISR decrements this.
static BEEP_MS: AtomicU32 = AtomicU32::new(0);
static BEEP_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Start a beep at `freq` Hz for `duration_ms` milliseconds.
/// If `freq` is 0, the speaker is silenced immediately.
pub fn beep(freq: u32, duration_ms: u32) {
    if freq == 0 || duration_ms == 0 {
        speaker_off();
        return;
    }

    let divisor = PIT_FREQ / freq;
    if divisor == 0 || divisor > 0xFFFF { return; }

    unsafe {
        // Program PIT channel 2 to mode 3 (square wave)
        let div = divisor as u16;
        asm!("out 0x43, al", in("al") 0xB6u8, options(nostack, nomem));
        asm!("out 0x42, al", in("al") (div & 0xFF) as u8, options(nostack, nomem));
        asm!("out 0x42, al", in("al") (div >> 8) as u8, options(nostack, nomem));

        // Enable speaker: port 0x61 bits 0 and 1
        let val: u8;
        asm!("in al, 0x61", out("al") val, options(nostack, nomem));
        asm!("out 0x61, al", in("al") val | 0x03, options(nostack, nomem));
    }

    BEEP_MS.store(duration_ms, Ordering::Relaxed);
    BEEP_ACTIVE.store(true, Ordering::Relaxed);
}

/// Called from timer ISR each tick (~1ms at 1kHz). Decrements the beep timer.
pub fn tick() {
    if !BEEP_ACTIVE.load(Ordering::Relaxed) { return; }
    let remaining = BEEP_MS.fetch_sub(1, Ordering::Relaxed);
    if remaining <= 1 {
        BEEP_ACTIVE.store(false, Ordering::Relaxed);
        speaker_off();
    }
}

/// Silence the speaker.
pub fn speaker_off() {
    unsafe {
        let val: u8;
        asm!("in al, 0x61", out("al") val, options(nostack, nomem));
        asm!("out 0x61, al", in("al") val & !0x03, options(nostack, nomem));
    }
    BEEP_ACTIVE.store(false, Ordering::Relaxed);
}

/// Check if a beep is currently playing.
pub fn is_beeping() -> bool {
    BEEP_ACTIVE.load(Ordering::Relaxed)
}
