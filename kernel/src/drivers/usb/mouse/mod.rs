//! USB HID Mouse Driver.
//!
//! Driver modular para ratones gaming USB modernos. Soporta reportes de
//! alta resolución y deltas de 16 bits para un control total.

#![allow(dead_code)]

use super::UsbDeviceInfo;

/// Boot mouse report (3 bytes, USB 2.0 §B.2)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy, Default)]
pub struct MouseBootReport {
    /// Buttons: bit 0=Left, 1=Right, 2=Middle, 3=Back, 4=Forward
    pub buttons: u8,
    pub dx: i8,
    pub dy: i8,
}

/// Mouse report extendido (con wheel + 16-bit deltas para gaming)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy, Default)]
pub struct MouseHighResReport {
    pub buttons: u8,
    pub dx: i16,
    pub dy: i16,
    pub wheel_v: i8,
    pub wheel_h: i8,
}

/// Inicializa y registra un ratón USB conectado.
pub fn attach(_info: UsbDeviceInfo) -> Result<(), &'static str> {
    crate::drivers::serial::serial_write("[USB-Mouse] Inicializando ratón gaming USB...\n");
    Ok(())
}
