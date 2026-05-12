//! Stack USB de FastOS — base para teclado, ratón y headset Redragon.
//!
//! Capas:
//! - `xhci`          — host controller (eXtensible Host Controller Interface 1.2)
//! - `descriptors`   — parsing de Device/Config/Interface/Endpoint descriptors
//! - `hid`           — clase HID 1.11 (Boot Protocol + Report Protocol)
//! - `audio_class`   — clase USB Audio Class 2.0 (UAC2) para output isócrono
//!
//! Filosofía: polling MSI-X event-driven, zero-copy DMA buffers, expuesto
//! a `barex::input` y `barex::audio` mediante el BMO ABI.

#![allow(dead_code)]

pub mod descriptors;
pub mod xhci;
pub mod hid;
pub mod audio_class;

/// Identificador único de un device USB conectado (asignado por el driver).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UsbDeviceId(pub u16);

/// Velocidad negociada de un device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbSpeed {
    /// 1.5 Mbps (USB 1.1) — algunos teclados/ratones antiguos.
    Low,
    /// 12 Mbps (USB 1.1).
    Full,
    /// 480 Mbps (USB 2.0) — la mayoría de headsets y dispositivos HID.
    High,
    /// 5 Gbps (USB 3.0/3.1 Gen 1).
    Super,
    /// 10 Gbps (USB 3.1 Gen 2).
    SuperPlus,
}

/// Clase USB declarada en el Device/Interface descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum UsbClass {
    /// 0x00 — definida a nivel de interface.
    PerInterface = 0x00,
    /// 0x01 — Audio (incluye UAC1, UAC2, UAC3).
    Audio        = 0x01,
    /// 0x03 — HID (Human Interface Device).
    Hid          = 0x03,
    /// 0x09 — Hub.
    Hub          = 0x09,
    /// 0x0E — Video class (cámaras web, no usado aquí).
    Video        = 0x0E,
    /// 0xFF — específico del vendor.
    VendorSpec   = 0xFF,
}

/// Información mínima visible para los subsistemas (audio/input).
#[derive(Debug, Clone, Copy)]
pub struct UsbDeviceInfo {
    pub id: UsbDeviceId,
    pub vendor_id: u16,
    pub product_id: u16,
    pub class: UsbClass,
    pub speed: UsbSpeed,
}

/// Vendor ID de Redragon — los headsets/ratones/teclados de la marca usan
/// varios PIDs distintos pero comparten esta firma.
pub const REDRAGON_VID: u16 = 0x0C45;

/// Inicializa el stack USB completo (xHCI + clases). Llamado desde `kernel_main`
/// **después** de que PCI haya enumerado los controladores.
pub fn init() -> Result<(), &'static str> {
    // TODO: 1) localizar xHCI vía PCI class 0x0C03/0x30
    //       2) reset + configurar event ring
    //       3) enumerar puertos y attach automático
    Err("usb::init no implementado todavía")
}
