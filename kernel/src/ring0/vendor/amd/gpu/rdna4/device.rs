//! `vendor/amd/gpu/rdna4/device.rs` — RDNA4 device structure.
//!
//! v1.8.8: skeleton. Defines the per-device state for an RDNA4 GPU:
//! the MMIO base address, the VRAM size, the active rings, etc.
//!
//! When the driver is fully implemented, this struct will be created
//! during PCI enumeration (when we detect vendor=0x1002, device in
//! RDNA4_DEVICE_IDS) and stored in a global.

#![allow(dead_code)]

use super::pci::is_rDNA4;

/// Maximum number of RDNA4 GPUs supported in one system.
pub const MAX_RDNA4_DEVICES: usize = 4;

/// Per-device state for an RDNA4 GPU.
#[derive(Debug, Clone, Copy)]
pub struct Rdna4Device {
    /// Physical base address of the MMIO BAR (BAR0).
    pub mmio_base: u64,
    /// Size of the MMIO region (BAR0 size).
    pub mmio_size: u64,
    /// Physical base address of the VRAM aperture (BAR2).
    pub vram_base: u64,
    /// Total VRAM size in bytes.
    pub vram_size: u64,
    /// PCI device ID (0x7480, 0x7481, 0x7490, etc.).
    pub device_id: u16,
    /// Whether this slot is active.
    pub active: bool,
}

impl Rdna4Device {
    /// Returns an empty (inactive) device.
    pub const fn empty() -> Self {
        Self {
            mmio_base: 0,
            mmio_size: 0,
            vram_base: 0,
            vram_size: 0,
            device_id: 0,
            active: false,
        }
    }

    /// Returns true if this is a valid RDNA4 device.
    pub fn is_valid(&self) -> bool {
        self.active && is_rDNA4(0x1002, self.device_id)
    }
}

/// Global table of detected RDNA4 devices.
static mut RDNA4_DEVICES: [Rdna4Device; MAX_RDNA4_DEVICES] = [Rdna4Device::empty(); MAX_RDNA4_DEVICES];

/// Returns the first active RDNA4 device, if any.
pub fn first_active() -> Option<Rdna4Device> {
    unsafe {
        for i in 0..MAX_RDNA4_DEVICES {
            if RDNA4_DEVICES[i].active {
                return Some(RDNA4_DEVICES[i]);
            }
        }
    }
    None
}

/// Registers an RDNA4 device (called from PCI enumeration).
pub fn register_device(idx: usize, dev: Rdna4Device) {
    if idx < MAX_RDNA4_DEVICES {
        unsafe {
            RDNA4_DEVICES[idx] = dev;
        }
    }
}
