//! USB HID 1.11 — Human Interface Device.
//!
//! Cubre los devices realmente conectados a este equipo:
//!   - Teclado USB (boot interface protocol 1)
//!   - Ratón USB (boot interface protocol 2)
//!   - Botones de control multimedia del headset Redragon (Consumer Page)
//!
//! Polling vía Interrupt IN endpoint con interval típico de 1–8 ms (USB 2.0
//! HighSpeed) o sub-ms con ratones gaming a 1000–8000 Hz.

#![allow(dead_code)]

use super::UsbDeviceInfo;

/// Subclase HID dentro del Interface Descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum HidSubclass {
    /// 0x00 — sin boot interface (usar Report Protocol).
    None = 0,
    /// 0x01 — soporta Boot Interface (teclados/ratones legacy).
    BootInterface = 1,
}

/// Protocolo de la Boot Interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum HidProtocol {
    None     = 0,
    Keyboard = 1,
    Mouse    = 2,
}

/// HID Class Descriptor (sigue al Interface Descriptor de clase 0x03).
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct HidClassDescriptor {
    pub b_length: u8,
    pub b_descriptor_type: u8,  // 0x21
    pub bcd_hid: u16,           // 0x0111 para HID 1.11
    pub b_country_code: u8,
    pub b_num_descriptors: u8,
    pub b_report_descriptor_type: u8, // 0x22
    pub w_report_descriptor_length: u16,
}

// ───── Boot keyboard report (8 bytes, USB 2.0 §B.1) ─────────────────────
#[repr(C, packed)]
#[derive(Debug, Clone, Copy, Default)]
pub struct KeyboardBootReport {
    /// Modifiers: bit 0=LCtrl, 1=LShift, 2=LAlt, 3=LGui, 4=RCtrl, 5=RShift, 6=RAlt, 7=RGui
    pub modifiers: u8,
    pub _reserved: u8,
    /// Hasta 6 keycodes simultáneos (NKRO necesita Report Protocol).
    pub keycodes: [u8; 6],
}

// ───── Boot mouse report (3 bytes, USB 2.0 §B.2) ─────────────────────────
#[repr(C, packed)]
#[derive(Debug, Clone, Copy, Default)]
pub struct MouseBootReport {
    /// Buttons: bit 0=Left, 1=Right, 2=Middle, 3=Back, 4=Forward
    pub buttons: u8,
    pub dx: i8,
    pub dy: i8,
}

// ───── Mouse report extendido (con wheel + 16-bit deltas para gaming) ───
#[repr(C, packed)]
#[derive(Debug, Clone, Copy, Default)]
pub struct MouseHighResReport {
    pub buttons: u8,
    pub dx: i16,
    pub dy: i16,
    pub wheel_v: i8,
    pub wheel_h: i8,
}

/// Estado HID consumido por `barex::input`.
#[derive(Debug, Clone, Copy)]
pub enum HidEvent {
    Keyboard(KeyboardBootReport),
    Mouse(MouseHighResReport),
    /// Botón del headset (mute, vol+, vol-, mic mute).
    HeadsetConsumer { usage: u16, pressed: bool },
}

/// Llamado por `xhci::enumerate_ports` cuando un device de class HID hace attach.
pub fn attach(_info: UsbDeviceInfo) -> Result<(), &'static str> {
    // TODO: leer Report Descriptor, decidir Boot vs Report Protocol,
    //       configurar interrupt IN endpoint con TRBs cíclicos.
    Err("hid::attach no implementado todavía")
}

/// Cola circular de eventos HID consumida por el servicio de input en Ring 3.
pub fn poll_events(_buf: &mut [HidEvent]) -> Result<usize, &'static str> {
    Err("hid::poll_events no implementado todavía")
}
