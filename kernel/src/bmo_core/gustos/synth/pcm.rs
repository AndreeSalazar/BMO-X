//! PCM buffer + output to PC speaker (PIT-based).
//!
//! v1.5.0: dos destinos:
//!   1. **PC speaker** (PIT channel 2) — siempre disponible, baja calidad
//!   2. **PCM buffer** — para USB audio (futuro)
//!
//! Para v1.5.0 activamos **solo PC speaker** porque es lo único que
//! tiene hardware garantizado en Ryzen 5 5600X.

use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/// PIT base frequency: 1.193182 MHz.
const PIT_BASE_HZ: u32 = 1_193_182;
const PIT_COMMAND_PORT: u16 = 0x43;
const PIT_CHANNEL_2_PORT: u16 = 0x42;
const SPEAKER_PORT: u16 = 0x61;

/// Estado del speaker (encendido/apagado).
static SPEAKER_ENABLED: AtomicBool = AtomicBool::new(false);

/// Frecuencia actual del PIT channel 2.
static CURRENT_FREQ: AtomicU32 = AtomicU32::new(0);

/// Modo de salida actual.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    /// PIT + PC speaker (square wave, freq fija).
    PcSpeaker,
    /// PCM buffer para USB audio (futuro).
    PcmBuffer,
}

static OUTPUT_MODE: AtomicU32 = AtomicU32::new(0); // 0 = PcSpeaker

/// Fija el modo de salida.
pub fn set_output_mode(mode: OutputMode) {
    OUTPUT_MODE.store(mode as u32, Ordering::Relaxed);
}

/// Emite un sample (f32 en [-1.0, 1.0]).
///
/// En modo PC speaker, esto actualiza la frecuencia del PIT según
/// el valor absoluto del sample (representa la amplitud/periodo).
///
/// En modo PCM buffer, lo almacena para futura transmisión.
pub fn emit(sample: f32) {
    match OUTPUT_MODE.load(Ordering::Relaxed) {
        0 => emit_pc_speaker(sample),
        _ => emit_pcm_buffer(sample),
    }
}

/// Output to PC speaker via PIT channel 2.
fn emit_pc_speaker(sample: f32) {
    // Mapeo: sample > 0 → tono, sample < 0 → silencio
    // Frecuencia: 200 Hz (sample positivo) o 0 Hz (silencio)
    let freq = if sample > 0.1 {
        200 + (sample.abs() * 800.0) as u32
    } else {
        0
    };
    set_pit_frequency(freq);
}

fn set_pit_frequency(freq: u32) {
    if freq == CURRENT_FREQ.load(Ordering::Relaxed) {
        return; // Sin cambio
    }

    unsafe {
        if freq == 0 {
            // Apagar speaker
            let prev = inb(SPEAKER_PORT);
            outb(SPEAKER_PORT, prev & !0x03);
            SPEAKER_ENABLED.store(false, Ordering::Relaxed);
        } else {
            // Programar PIT channel 2: lobyte/hibyte, mode 3 (square wave)
            outb(PIT_COMMAND_PORT, 0xB6);
            let divisor = (PIT_BASE_HZ / freq) as u16;
            outb(PIT_CHANNEL_2_PORT, (divisor & 0xFF) as u8);
            outb(PIT_CHANNEL_2_PORT, ((divisor >> 8) & 0xFF) as u8);

            // Activar speaker
            let prev = inb(SPEAKER_PORT);
            outb(SPEAKER_PORT, prev | 0x03);
            SPEAKER_ENABLED.store(true, Ordering::Relaxed);
        }
    }

    CURRENT_FREQ.store(freq, Ordering::Relaxed);
}

fn emit_pcm_buffer(_sample: f32) {
    // TODO: cuando USB audio esté integrado, llenar un buffer
    // y enviarlo al endpoint de salida.
}

#[inline]
unsafe fn outb(port: u16, value: u8) {
    core::arch::asm!("out dx, al", in("dx") port, in("al") value, options(nomem, preserves_flags));
}

#[inline]
unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    core::arch::asm!("in al, dx", out("al") value, in("dx") port, options(nomem, preserves_flags));
    value
}

/// Apaga el speaker.
pub fn silence() {
    set_pit_frequency(0);
}
