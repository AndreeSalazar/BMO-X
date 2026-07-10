//! Audio mixer — Ring 0 software mixer for PC speaker + HDA (future).
//!
//! v1.x: only PC speaker is wired. The mixer is structured so that
//! adding HDA / USB audio is just a matter of implementing a
//! `Backend` trait and registering it.
//!
//! ## Channel model
//!
//! A `Channel` is a mono/stereo PCM stream of i16 samples. The mixer
//! runs in the timer ISR (~1 kHz) and advances all playing channels
//! by `ticks_per_ms` samples each tick.
//!
//! At 1 kHz tick rate, this gives a worst-case mixer latency of ~1 ms,
//! which is acceptable for UI sounds. Games should request a higher
//! tick rate (HDA-backed later).

use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/// Maximum number of simultaneous voices.
pub const MAX_VOICES: usize = 8;

/// Sample rate the mixer runs at (must match tick rate × ticks_per_ms).
pub const SAMPLE_RATE: u32 = 22_050;

/// Mono sample.
pub type Sample = i16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Idle,
    Playing,
    Finished,
}

pub struct Voice {
    /// Pointer to sample data (i16 LE).
    pub data: *const Sample,
    /// Number of samples.
    pub len: u32,
    /// Current playhead.
    pub pos: u32,
    /// Volume 0..256.
    pub volume: u32,
    /// State.
    pub state: State,
    /// Loop?
    pub looping: bool,
}

// SAFETY: Voice is accessed under a single-core ISR or a spinlock;
// Send/Sync are required because we store Voices in a static.
unsafe impl Send for Voice {}
unsafe impl Sync for Voice {}

static mut VOICES: [Voice; MAX_VOICES] = [const {
    Voice {
        data: core::ptr::null(),
        len: 0,
        pos: 0,
        volume: 0,
        state: State::Idle,
        looping: false,
    }
}; MAX_VOICES];

/// Master volume 0..256.
static MASTER_VOLUME: AtomicU32 = AtomicU32::new(256);

/// True if the mixer has been initialized.
static INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Initialize the audio mixer. Call once at boot.
pub fn init() {
    unsafe {
        for v in VOICES.iter_mut() {
            *v = Voice {
                data: core::ptr::null(),
                len: 0,
                pos: 0,
                volume: 0,
                state: State::Idle,
                looping: false,
            };
        }
    }
    INITIALIZED.store(true, Ordering::Release);
    let _ = crate::dev::hda::probe();
}

/// Set master volume 0..256.
pub fn set_volume(vol: u32) {
    MASTER_VOLUME.store(vol.min(256), Ordering::Relaxed);
}

/// Play a PCM sample on a voice slot. Returns the voice id used, or
/// None if all slots are busy. The voice is automatically freed when
/// playback completes.
pub fn play(data: *const Sample, len: u32, volume: u32) -> Option<u32> {
    if data.is_null() || len == 0 { return None; }
    unsafe {
        for (i, v) in VOICES.iter_mut().enumerate() {
            if v.state != State::Playing {
                v.data = data;
                v.len = len;
                v.pos = 0;
                v.volume = volume.min(256);
                v.state = State::Playing;
                v.looping = false;
                return Some(i as u32);
            }
        }
    }
    None
}

/// Play a sample in loop mode (background music, ambient).
pub fn play_loop(data: *const Sample, len: u32, volume: u32) -> Option<u32> {
    if data.is_null() || len == 0 { return None; }
    unsafe {
        for (i, v) in VOICES.iter_mut().enumerate() {
            if v.state != State::Playing {
                v.data = data;
                v.len = len;
                v.pos = 0;
                v.volume = volume.min(256);
                v.state = State::Playing;
                v.looping = true;
                return Some(i as u32);
            }
        }
    }
    None
}

/// Stop a voice.
pub fn stop(voice: u32) {
    if (voice as usize) >= MAX_VOICES { return; }
    unsafe {
        let v = &mut VOICES[voice as usize];
        v.state = State::Idle;
        v.data = core::ptr::null();
        v.len = 0;
        v.pos = 0;
    }
}

/// Stop all voices.
pub fn stop_all() {
    unsafe {
        for v in VOICES.iter_mut() {
            v.state = State::Idle;
            v.data = core::ptr::null();
            v.len = 0;
            v.pos = 0;
        }
    }
}

/// Returns true if any voice is currently playing.
pub fn is_playing() -> bool {
    unsafe { VOICES.iter().any(|v| v.state == State::Playing) }
}

/// Mix `n_samples` from all active voices into `dst`. Returns the
/// number of voices that finished during this call.
pub fn mix(dst: &mut [Sample]) -> usize {
    let mut finished = 0;
    let master = MASTER_VOLUME.load(Ordering::Relaxed) as i32;

    unsafe {
        for v in VOICES.iter_mut() {
            if v.state != State::Playing { continue; }
            if v.data.is_null() || v.len == 0 {
                v.state = State::Idle;
                finished += 1;
                continue;
            }

            for (i, s) in dst.iter_mut().enumerate() {
                if v.pos >= v.len {
                    if v.looping {
                        v.pos = 0;
                    } else {
                        v.state = State::Idle;
                        finished += 1;
                        break;
                    }
                }
                let raw = *v.data.add(v.pos as usize);
                v.pos += 1;
                // Scale: raw * (voice_vol * master_vol) / 65536
                let scaled = (raw as i32) * (v.volume as i32) * master / 65536;
                let mixed = (*s as i32) + scaled;
                *s = mixed.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
                let _ = i;
            }
        }
    }
    finished
}

// ═══════════════════════════════════════════════════════════════════
//  Procedural sound effects (used by the desktop for UI)
// ═══════════════════════════════════════════════════════════════════

/// Synthesize a click sound and play it on the PC speaker.
/// `freq_hz` is the click tone, `duration_ms` the length.
pub fn play_click(freq_hz: u32, duration_ms: u32) {
    crate::dev::pc_speaker::beep(freq_hz, duration_ms);
}

/// Play a short tone using a square wave generated in software and
/// fed to the PC speaker. This is the "synthesized" path used when
/// a sampled clip is not available.
pub fn play_tone(freq_hz: u32, duration_ms: u32) {
    crate::dev::pc_speaker::beep(freq_hz, duration_ms);
}

/// Play the Windows logon-style welcome chime (C major arpeggio).
pub fn play_logon_chime() {
    // C5, E5, G5, C6 — short notes with short silence
    let notes: [(u32, u32); 4] = [
        (523, 150),
        (659, 150),
        (784, 150),
        (1046, 300),
    ];
    for (freq, dur) in notes.iter() {
        crate::dev::pc_speaker::beep(*freq, *dur);
        // Crude busy-wait so the next note starts after the previous
        // finished (PC speaker is synchronous).
        for _ in 0..(*dur as u64) * 2000 {
            core::hint::spin_loop();
        }
    }
}
