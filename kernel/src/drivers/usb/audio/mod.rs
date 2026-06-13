//! USB Audio Class 2.0 (UAC2) — output isócrono para el headset Redragon.
//!
//! El headset USB Redragon se enumera como un solo device con multiples interfaces:
//!   - Interface 0: AudioControl
//!   - Interface 1: AudioStreaming OUT (playback)
//!   - Interface 2: AudioStreaming IN  (micrófono)
//!   - Interface 3: HID (botones multimedia)

#![allow(dead_code)]

use super::{UsbDeviceInfo, UsbDeviceId};

/// Subclases del Audio Class 0x01.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AudioSubclass {
    Undefined        = 0x00,
    AudioControl     = 0x01,
    AudioStreaming   = 0x02,
    MidiStreaming    = 0x03,
}

/// Protocolo (1 = UAC1, 0x20 = UAC2, 0x30 = UAC3).
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
    pub b_subslot_size: u8,       // 2 = 16-bit
    pub b_bit_resolution: u8,     // 16
}

#[derive(Debug, Clone, Copy)]
pub struct StreamFormat {
    pub sample_rate: u32,         // 48000 típico
    pub channels: u8,             // 2 stereo
    pub bits_per_sample: u8,      // 16 típico
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
        frame_bytes: 576,
    };
}

/// Endpoint isócrono OUT preparado para playback.
pub struct IsochOutEndpoint {
    pub device: UsbDeviceId,
    pub ep_address: u8,
    pub max_packet_size: u16,
    pub interval_us: u32,
    pub format: StreamFormat,
}

/// Llamado por xhci al detectar class 0x01 (Audio).
pub fn attach(_info: UsbDeviceInfo) -> Result<(), &'static str> {
    crate::drivers::serial::serial_write("[USB-Audio] Conectando dispositivo de audio...\n");
    Err("audio::attach no implementado todavía")
}

/// Empuja un buffer PCM al endpoint isócrono OUT
pub fn submit_pcm(_ep: &IsochOutEndpoint, _samples: &[i16]) -> Result<(), &'static str> {
    Err("audio::submit_pcm no implementado todavía")
}

/// Detección del headset Redragon por VID/PID.
pub fn is_redragon_headset(info: &UsbDeviceInfo) -> bool {
    use super::REDRAGON_VID;
    info.vendor_id == REDRAGON_VID
}
