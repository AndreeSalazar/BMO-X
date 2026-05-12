//! Hardware drivers.

pub mod pci;
pub mod serial;
pub mod nvme;
pub mod ahci;
pub mod gpu;
// Stack USB para teclado, ratón y headset Redragon (xHCI + HID + UAC2).
pub mod usb;
