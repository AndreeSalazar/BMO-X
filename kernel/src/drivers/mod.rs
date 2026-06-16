//! Hardware drivers.
//!
//! Modular driver architecture:
//!   - pci: PCI ECAM configuration space
//!   - serial: Serial port (115200 baud, COM1)
//!   - storage: Unified storage backend (NVMe + AHCI + RAM)
//!   - nvme: NVMe SSD driver
//!   - ahci: AHCI/SATA driver
//!   - net: Network stack (RTL8168 + ARP + IP + ICMP + UDP + DHCP)
//!   - gop: UEFI GOP framebuffer
//!   - usb: USB stack (xHCI + HID + audio)

pub mod pci;
pub mod serial;
pub mod storage;
pub mod nvme;
pub mod ahci;
pub mod net;
pub mod gop;
// GPU experimental (NVIDIA RTX 3060) — removed. FastOS uses UEFI GOP/framebuffer.
pub mod usb;
