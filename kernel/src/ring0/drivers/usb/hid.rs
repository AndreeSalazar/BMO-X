//! USB HID 1.11 — Human Interface Device generic router.
//!
//! Routers raw HID data to the specialized keyboard and mouse drivers.

#![allow(dead_code)]

use super::UsbDeviceInfo;
use super::keyboard::KeyboardBootReport;
use super::mouse::MouseHighResReport;

/// Subclase HID dentro del Interface Descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum HidSubclass {
    None = 0,
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

/// HID Class Descriptor.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct HidClassDescriptor {
    pub b_length: u8,
    pub b_descriptor_type: u8,
    pub bcd_hid: u16,
    pub b_country_code: u8,
    pub b_num_descriptors: u8,
    pub b_report_descriptor_type: u8,
    pub w_report_descriptor_length: u16,
}

/// HID Event structure.
#[derive(Debug, Clone, Copy)]
pub enum HidEvent {
    Keyboard(KeyboardBootReport),
    Mouse(MouseHighResReport),
    HeadsetConsumer { usage: u16, pressed: bool },
}

/// Central routing point for HID devices.
pub fn attach(info: UsbDeviceInfo) -> Result<(), &'static str> {
    crate::drivers::serial::serial_write("[USB-HID] Enrutando dispositivo HID...\n");
    // Check if it is a keyboard or mouse or generic headset buttons
    match info.class {
        _ => {
            // Decidir por descriptor
            // En un driver completo leeríamos el subclass/protocolo
            Ok(())
        }
    }
}

pub fn poll_events(_buf: &mut [HidEvent]) -> Result<usize, &'static str> {
    Err("hid::poll_events no implementado todavía")
}
