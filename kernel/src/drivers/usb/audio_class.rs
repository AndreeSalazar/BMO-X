//! USB Audio Class 2.0 (UAC2) — output isócrono para el headset Redragon.
//!
//! El headset USB Redragon (modelos H510 Zeus, H320, H120, etc.) se enumera
//! como **un solo device USB con dos interfaces**:
//!   - Interface 0: AudioControl (clase 0x01, subclass 0x01)
//!   - Interface 1: AudioStreaming OUT (clase 0x01, subclass 0x02) — playback
//!   - Interface 2: AudioStreaming IN  (subclass 0x02) — micrófono (algunos)
//!   - Interface 3: HID (clase 0x03) — botones de volumen/mute
//!
//! El stream OUT usa endpoint isócrono (transfer type Isoch) con frame size
//! variable según sample rate. La spec de FastOS objetivo: **48 kHz / 16-bit
//! stereo / 1 ms isoch period** = 192 bytes/frame.

#![allow(dead_code)]

use super::UsbDeviceInfo;

/// Subclases del Audio Class 0x01.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AudioSubclass {
    Undefined        = 0x00,
    AudioControl     = 0x01,
    AudioStreaming   = 0x02,
    MidiStreaming    = 0x03,
}

/// Protocolo (1 = UAC1 legacy, 0x20 = UAC2, 0x30 = UAC3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AudioProtocol {
    Uac1 = 0x00,
    Uac2 = 0x20,
    Uac3 = 0x30,
}

/// Class-Specific AC Interface Header (UAC2 §4.7.2).
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct AcInterfaceHeader {
    pub b_length: u8,
    pub b_descriptor_type: u8,    // 0x24 (CS_INTERFACE)
    pub b_descriptor_subtype: u8, // 0x01 (HEADER)
    pub bcd_adc: u16,             // 0x0200 para UAC2
    pub b_category: u8,
    pub w_total_length: u16,
    pub bm_controls: u8,
}

/// Format Type I — PCM lineal (UAC2 §2.3.1.1).
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct FormatType1 {
    pub b_length: u8,
    pub b_descriptor_type: u8,    // 0x24
    pub b_descriptor_subtype: u8, // 0x02 (FORMAT_TYPE)
    pub b_format_type: u8,        // 0x01
    pub b_subslot_size: u8,       // 2 = 16-bit, 3 = 24-bit, 4 = 32-bit
    pub b_bit_resolution: u8,     // 16, 24, 32
}

#[derive(Debug, Clone, Copy)]
pub struct StreamFormat {
    pub sample_rate: u32,         // 48000 típico
    pub channels: u8,             // 2 stereo
    pub bits_per_sample: u8,      // 16 típico
    /// Tamaño de frame del isoch endpoint en bytes:
    /// `bytes_per_sample * channels * (rate / 1000)` para HighSpeed @ 1 ms.
    pub frame_bytes: u32,
}

impl StreamFormat {
    pub const REDRAGON_DEFAULT: Self = Self {
        sample_rate: 48_000,
        channels: 2,
        bits_per_sample: 16,
        frame_bytes: 192, // 2 ch · 2 B · 48 samples/ms
    };

    pub const HIRES_96K: Self = Self {
        sample_rate: 96_000,
        channels: 2,
        bits_per_sample: 24,
        frame_bytes: 576, // 2 ch · 3 B · 96 samples/ms
    };
}

/// Endpoint isócrono OUT preparado para playback.
pub struct IsochOutEndpoint {
    pub device: UsbDeviceId,
    pub ep_address: u8,
    pub max_packet_size: u16,
    pub interval_us: u32,    // 125 µs (HighSpeed micro-frame) o 1000 µs (Full-Speed)
    pub format: StreamFormat,
}

/// Llamado por `xhci::enumerate_ports` al detectar class 0x01.
pub fn attach(_info: UsbDeviceInfo) -> Result<(), &'static str> {
    // TODO: parsear AC + AS interfaces, encontrar Format Type I 48 kHz stereo,
    //       configurar Iso TRBs en anillo, exponer a `barex::audio`.
    Err("audio_class::attach no implementado todavía")
}

/// Empuja un buffer PCM al endpoint isócrono OUT (latencia < 2 ms en realtime).
pub fn submit_pcm(_ep: &IsochOutEndpoint, _samples: &[i16]) -> Result<(), &'static str> {
    Err("audio_class::submit_pcm no implementado todavía")
}

/// Detección heurística de un headset Redragon por VID/PID.
pub fn is_redragon_headset(info: &UsbDeviceInfo) -> bool {
    use super::REDRAGON_VID;
    info.vendor_id == REDRAGON_VID
}
