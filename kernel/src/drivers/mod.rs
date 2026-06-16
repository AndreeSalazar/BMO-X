//! Hardware drivers.

pub mod pci;
pub mod serial;
pub mod nvme;
pub mod ahci;
pub mod gop;
// GPU experimental (NVIDIA RTX 3060) — removed. FastOS uses UEFI GOP/framebuffer.
// USB stack for keyboard, mouse, and headset (xHCI + HID + UAC2).
pub mod usb;
