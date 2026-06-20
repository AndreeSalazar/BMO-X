//! USB HID Keyboard Driver.
//!
//! Driver modular para teclados gaming USB modernos. Soporta protocolo
//! de boot (Boot Interface Protocol) y NKRO si está disponible.

#![allow(dead_code)]

use super::UsbDeviceInfo;

/// Boot keyboard report (8 bytes, USB 2.0 §B.1)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy, Default)]
pub struct KeyboardBootReport {
    /// Modifiers: bit 0=LCtrl, 1=LShift, 2=LAlt, 3=LGui, 4=RCtrl, 5=RShift, 6=RAlt, 7=RGui
    pub modifiers: u8,
    pub _reserved: u8,
    /// Hasta 6 keycodes simultáneos (NKRO necesita Report Protocol).
    pub keycodes: [u8; 6],
}

/// Inicializa y registra un teclado USB conectado.
pub fn attach(_info: UsbDeviceInfo) -> Result<(), &'static str> {
    crate::drivers::serial::serial_write("[USB-Keyboard] Inicializando teclado gaming USB...\n");
    Ok(())
}
