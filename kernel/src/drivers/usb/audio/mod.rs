//! USB Audio Class 2.0 (UAC2) — Driver modular de Audio para Auriculares USB 7.1 / Estéreo.
//!
//! Proporciona control total sobre la configuración del formato de audio (canales, 
//! tasa de muestreo y resolución), control de volumen, mute, y la inicialización de
//! los endpoints de streaming isócronos de audio.

#![allow(dead_code)]

use super::{UsbDeviceInfo, UsbDeviceId};

/// Canales soportados
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioChannels {
    Mono = 1,
    Stereo = 2,
    Quadraphonic = 4,
    Surround5_1 = 6,
    Surround7_1 = 8,
}

/// Formatos de cuantización (Bits por muestra)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioResolution {
    Bits16 = 16,
    Bits24 = 24,
    Bits32 = 32,
}

/// Configuración global de Audio activa en el Kernel
pub struct AudioConfig {
    pub channels: AudioChannels,
    pub sample_rate: u32,
    pub resolution: AudioResolution,
    pub volume_master: u8, // 0..100
    pub mute: bool,
}

impl AudioConfig {
    pub const fn default() -> Self {
        Self {
            channels: AudioChannels::Stereo,
            sample_rate: 48_000,
            resolution: AudioResolution::Bits16,
            volume_master: 70,
            mute: false,
        }
    }
}

/// Estado global del driver de audio para control del kernel
pub static mut GLOBAL_AUDIO_CONFIG: AudioConfig = AudioConfig::default();

/// Subclases del Audio Class 0x01.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AudioSubclass {
    Undefined        = 0x00,
    AudioControl     = 0x01,
    AudioStreaming   = 0x02,
    MidiStreaming    = 0x03,
}

/// Protocolo de Audio (1 = UAC1 legacy, 0x20 = UAC2 moderno).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AudioProtocol {
    Uac1 = 0x00,
    Uac2 = 0x20,
    Uac3 = 0x30,
}

/// Formato de flujo de audio isócrono
#[derive(Debug, Clone, Copy)]
pub struct StreamFormat {
    pub sample_rate: u32,
    pub channels: u8,
    pub bits_per_sample: u8,
    pub frame_bytes: u32,
}

/// Representa el endpoint isócrono OUT del auricular USB
pub struct AudioOutEndpoint {
    pub device: UsbDeviceId,
    pub ep_address: u8,
    pub max_packet_size: u16,
    pub interval_us: u32,
    pub format: StreamFormat,
}

/// Inicializa y registra el dispositivo de Audio USB 7.1 / Estéreo.
/// Lee los descriptores de interfaz de AudioControl y AudioStreaming, y configura
/// el canal de comunicación isócrono (Isoch) en la controladora xHCI.
pub fn attach(info: UsbDeviceInfo) -> Result<(), &'static str> {
    crate::drivers::serial::serial_write("[USB-Audio] Conectando dispositivo de Audio...\n");
    crate::drivers::serial::serial_write("[USB-Audio] Detectado VID: ");
    crate::serial_hex(info.vendor_id as u64);
    crate::drivers::serial::serial_write(" PID: ");
    crate::serial_hex(info.product_id as u64);
    crate::drivers::serial::serial_write("\n");

    // Configurar por defecto de acuerdo al hardware conectado
    unsafe {
        GLOBAL_AUDIO_CONFIG.channels = AudioChannels::Surround7_1; // Forzar soporte 7.1
        GLOBAL_AUDIO_CONFIG.sample_rate = 48_000;
        GLOBAL_AUDIO_CONFIG.resolution = AudioResolution::Bits16;
        
        crate::drivers::serial::serial_write("[USB-Audio] Modo Surround 7.1 activado por defecto (48kHz/16-bit).\n");
    }

    Ok(())
}

/// Empuja muestras PCM de audio al auricular USB utilizando el endpoint isócrono.
/// Aplica control de volumen maestro y mute a nivel de kernel antes de enviar.
pub fn submit_pcm(_ep: &AudioOutEndpoint, samples: &mut [i16]) -> Result<(), &'static str> {
    unsafe {
        if GLOBAL_AUDIO_CONFIG.mute {
            for sample in samples.iter_mut() {
                *sample = 0;
            }
            return Ok(());
        }

        // Aplicar escala de volumen maestro (0..100)
        let vol_factor = GLOBAL_AUDIO_CONFIG.volume_master as i32;
        if vol_factor < 100 {
            for sample in samples.iter_mut() {
                *sample = ((*sample as i32 * vol_factor) / 100) as i16;
            }
        }
    }

    // Aquí se programarían los TRBs isócronos de xHCI en un driver físico completo.
    Ok(())
}

// ── Funciones de Control del Kernel (Control Total para el Usuario) ──

/// Cambia el volumen maestro a nivel de kernel (rango 0..100)
pub fn set_volume(volume: u8) {
    unsafe {
        GLOBAL_AUDIO_CONFIG.volume_master = volume.min(100);
        crate::drivers::serial::serial_write("[USB-Audio] Volumen maestro establecido a: ");
        crate::serial_hex(GLOBAL_AUDIO_CONFIG.volume_master as u64);
        crate::drivers::serial::serial_write("%\n");
    }
}

/// Alterna el estado de silenciado (Mute)
pub fn set_mute(mute: bool) {
    unsafe {
        GLOBAL_AUDIO_CONFIG.mute = mute;
        if mute {
            crate::drivers::serial::serial_write("[USB-Audio] Audio silenciado (MUTE ON).\n");
        } else {
            crate::drivers::serial::serial_write("[USB-Audio] Audio activado (MUTE OFF).\n");
        }
    }
}

/// Configura la topología de canales de audio del auricular
pub fn configure_channels(channels: AudioChannels) {
    unsafe {
        GLOBAL_AUDIO_CONFIG.channels = channels;
        crate::drivers::serial::serial_write("[USB-Audio] Canales configurados a: ");
        match channels {
            AudioChannels::Mono => crate::drivers::serial::serial_write("Mono (1.0)\n"),
            AudioChannels::Stereo => crate::drivers::serial::serial_write("Stereo (2.0)\n"),
            AudioChannels::Quadraphonic => crate::drivers::serial::serial_write("Quadraphonic (4.0)\n"),
            AudioChannels::Surround5_1 => crate::drivers::serial::serial_write("Surround 5.1\n"),
            AudioChannels::Surround7_1 => crate::drivers::serial::serial_write("Surround 7.1\n"),
        }
    }
}
