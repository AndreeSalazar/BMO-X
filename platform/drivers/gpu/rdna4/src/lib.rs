//! BMO GPU driver profile — AMD RDNA 4.
//!
//! Reserved slot: the bench has no RDNA 4 card yet. This crate holds the
//! profile contract so that, the day the GPU arrives, bringing it up is a
//! profile fill-in — the same professional-profile rule the CPU follows
//! (`cpu_vendor/profile.rs` in the kernel: swapping hardware is a profile
//! swap, never a kernel edit).
//!
//! Like every BMO driver, this runs in **Ring 3** as a BEX server behind
//! a BMO Channel estuary. It will hold DEVICE + DISPLAY capabilities for
//! its own MMIO ranges only. The Ring 0 kernel never gains GPU code.
//!
//! Planned surface (F5+): command-ring setup, framebuffer/scanout for
//! the compositor, and a display-surface protocol over `Estuary<T>`.

#![no_std]

/// GPU profile descriptor, mirroring the CPU profile philosophy.
pub struct GpuProfile {
    pub vendor: &'static str,
    pub microarch: &'static str,
    /// PCI vendor id (AMD).
    pub pci_vendor: u16,
    /// PCI device ids this profile claims. Empty until the exact card
    /// (Navi 4x SKU) is on the bench.
    pub pci_devices: &'static [u16],
}

pub static PROFILE: GpuProfile = GpuProfile {
    vendor: "AMD",
    microarch: "RDNA 4 (Navi 4x)",
    pci_vendor: 0x1002,
    pci_devices: &[],
};

/// Whether the profile can claim `device_id`. Always false until the
/// SKU list is filled in with real hardware.
pub fn claims(vendor: u16, device: u16) -> bool {
    vendor == PROFILE.pci_vendor && PROFILE.pci_devices.contains(&device)
}
