//! `vendor/amd/gpu/rdna4/pci.rs` — RDNA4 PCIe device identity.
//!
//! v1.8.8: skeleton. Defines the PCI vendor/device IDs for the RDNA4
//! family and provides a function to detect if a given PCI device is
//! an RDNA4 GPU.
//!
//! Reference: AMD RDNA4 (Navi 4x) — RX 9060 XT, RX 9070, RX 9080.
//! PCI device IDs are preliminary (need confirmation from AMD).

#![allow(dead_code)]

/// AMD PCI vendor ID (0x1002).
pub const PCI_VENDOR_ID_AMD: u16 = 0x1002;

/// RDNA4 device IDs (preliminary — confirm with AMD BKDG when public).
/// These are placeholders; the real IDs will come from the AMD NDA BKDG.
pub const PCI_DEVICE_ID_RDNA4_NAVI48_XT: u16 = 0x7480;  // RX 9060 XT (placeholder)
pub const PCI_DEVICE_ID_RDNA4_NAVI48: u16 = 0x7481;    // RX 9060 (placeholder)
pub const PCI_DEVICE_ID_RDNA4_NAVI44: u16 = 0x7490;    // RX 9070 (placeholder)

/// List of known RDNA4 device IDs.
pub const RDNA4_DEVICE_IDS: &[u16] = &[
    PCI_DEVICE_ID_RDNA4_NAVI48_XT,
    PCI_DEVICE_ID_RDNA4_NAVI48,
    PCI_DEVICE_ID_RDNA4_NAVI44,
];

/// Returns true if (vendor, device) is a known RDNA4 GPU.
/// v1.8.8: not const because PartialEq on u16 in const fn is unstable.
pub fn is_rDNA4(vendor: u16, device: u16) -> bool {
    if vendor != PCI_VENDOR_ID_AMD {
        return false;
    }
    let mut i = 0;
    while i < RDNA4_DEVICE_IDS.len() {
        if RDNA4_DEVICE_IDS[i] == device {
            return true;
        }
        i += 1;
    }
    false
}

/// Human-readable name for a given RDNA4 device ID.
pub fn device_name(device_id: u16) -> &'static str {
    if device_id == PCI_DEVICE_ID_RDNA4_NAVI48_XT {
        "AMD Radeon RX 9060 XT"
    } else if device_id == PCI_DEVICE_ID_RDNA4_NAVI48 {
        "AMD Radeon RX 9060"
    } else if device_id == PCI_DEVICE_ID_RDNA4_NAVI44 {
        "AMD Radeon RX 9070"
    } else {
        "Unknown AMD RDNA4 GPU"
    }
}
