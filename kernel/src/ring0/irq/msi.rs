//! MSI/MSI-X — Message Signaled Interrupts for PCI devices.
//!
//! Modern PCIe devices (XHCI, AHCI, NVMe) use MSI/MSI-X instead
//! of legacy INTx# pins. This module configures MSI capability
//! structures in PCI config space to route interrupts to LAPIC vectors.

use crate::dev::pcie::PciDevice;

/// Enable MSI for a PCI device. Writes the message address
/// (LAPIC base + 0xFEE00000) and data (vector number) to the
/// device's MSI capability registers.
pub fn enable_msi(_dev: &PciDevice, _vector: u8) {
    crate::dev::console::serial_write("[msi] enable stub\n");
}

/// Enable MSI-X for a PCI device. MSI-X uses a BAR-based table
/// instead of config space capability registers.
pub fn enable_msix(_dev: &PciDevice, _vector: u8) {
    crate::dev::console::serial_write("[msix] enable stub\n");
}

/// Initialize MSI subsystem (called once at boot).
pub fn init() {
    crate::dev::console::serial_write("[msi] init stub\n");
}
