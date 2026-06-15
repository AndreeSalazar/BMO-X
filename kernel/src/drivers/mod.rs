//! Hardware drivers.

pub mod pci;
pub mod serial;
pub mod nvme;
pub mod ahci;
pub mod gop;
// GPU acelerada experimental archivada en kernel/src/legacy/gpu/.
// FastOS usa UEFI GOP/framebuffer como backend estable.
// Stack USB para teclado, ratón y headset Redragon (xHCI + HID + UAC2).
pub mod usb;
