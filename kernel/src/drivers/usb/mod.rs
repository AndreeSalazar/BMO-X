//! Stack USB de FastOS — base para teclado, ratón y almacenamiento masivo.
//!
//! Capas:
//! - `xhci`          — host controller (eXtensible Host Controller Interface 1.2)
//! - `msc`           — clase de almacenamiento masivo USB (Mass Storage Class / SCSI)
//! - `descriptors`   — parsing de Device/Config/Interface/Endpoint descriptors
//! - `hid`           — clase HID 1.11 (Boot Protocol + Report Protocol)
//! - `audio_class`   — clase USB Audio Class 2.0 (UAC2) para output isócrono

#![allow(dead_code)]

pub mod descriptors;
pub mod xhci;
pub mod msc;
pub mod hid;
pub mod audio_class;

use crate::drivers::serial;

/// Identificador único de un device USB conectado (asignado por el driver).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UsbDeviceId(pub u16);

/// Velocidad negociada de un device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbSpeed {
    Low,
    Full,
    High,
    Super,
    SuperPlus,
}

/// Clase USB declarada en el Device/Interface descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum UsbClass {
    PerInterface = 0x00,
    Audio        = 0x01,
    Hid          = 0x03,
    Hub          = 0x09,
    Video        = 0x0E,
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

pub const REDRAGON_VID: u16 = 0x0C45;

/// Inicializa el stack USB completo (xHCI + clases). Llamado desde `kernel_main`
/// **después** de que PCI haya enumerado los controladores.
pub fn init() -> Result<(), &'static str> {
    serial::serial_write("[USB] Inicializando subsistema USB...\n");

    // 1. Detectar controlador xHCI
    if let Some(mut controller) = xhci::XhciController::detect() {
        serial::serial_write("[USB] Controlador xHCI inicializado correctamente.\n");
        
        // Enlistar los puertos activos
        let _ = controller.enumerate_ports();

        // 2. Instanciar y registrar el dispositivo de almacenamiento USB MSC
        // Endpoint 1 = Bulk In (0x81), Endpoint 2 = Bulk Out (0x02) en Slot 1 por defecto
        let mut msc_dev = msc::UsbMscDevice::new(1, 0x81, 0x02);
        
        if msc_dev.init_device().is_ok() {
            unsafe {
                msc::ACTIVE_USB_DISK = Some(msc_dev);
            }
            serial::serial_write("[USB] Dispositivo USB Mass Storage registrado como ACTIVE_USB_DISK.\n");
        } else {
            serial::serial_write("[USB] WARN: Falló inicialización SCSI en dispositivo USB.\n");
        }
    } else {
        serial::serial_write("[USB] WARN: No se detectó ningún controlador xHCI compatible.\n");
        serial::serial_write("[USB] Se activará emulación fallback para sistemas sin controladora física.\n");
        
        // Registrar disco virtual fallback para que el arranque BMO-FS funcione en cualquier PC/VM
        let mut fallback_dev = msc::UsbMscDevice::new(0, 0, 0);
        fallback_dev.total_blocks = 204800; // 100 MB
        unsafe {
            msc::ACTIVE_USB_DISK = Some(fallback_dev);
        }
    }

    Ok(())
}
