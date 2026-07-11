//! MSI/MSI-X — Message Signaled Interrupts for PCI devices.
//!
//! Modern PCIe devices (XHCI, AHCI, NVMe) use MSI/MSI-X instead
//! of legacy INTx# pins. MSI writes to a special address (0xFEE00000)
//! that the LAPIC intercepts as an interrupt.
//!
//! ## MSI Capability Structure (PCI config space)
//!
//! ```text
//! Offset  Size  Field
//! 0x00    1     Capability ID (0x05)
//! 0x01    1     Next pointer
//! 0x02    2     Message Control
//!               bit 0: MSI Enable
//!               bits 4-6: Multiple Message Capable
//!               bits 20-22: Multiple Message Enable
//! 0x04    4     Message Address (low)
//! 0x08    4     Message Address (high) — only for 64-bit
//! 0x0C    2     Message Data
//! ```
//!
//! ## MSI-X Table (BAR-based)
//!
//! Each entry is 16 bytes:
//! ```text
//! Offset  Size  Field
//! 0x00    4     Message Address (low)
//! 0x04    4     Message Address (high)
//! 0x08    4     Message Data
//! 0x0C    4     Vector Control (bit 0 = masked)
//! ```

use crate::dev::pcie;

/// PCI MSI capability ID.
const PCI_CAP_ID_MSI: u8 = 0x05;
/// PCI MSI-X capability ID.
#[allow(dead_code)]
const PCI_CAP_ID_MSIX: u8 = 0x11;

/// LAPIC MSI address: 0xFEE00000 + (destination_id << 12).
/// Destination ID 0 = BSP (Boot Strap Processor).
const MSI_ADDR_BASE: u64 = 0xFEE0_0000;

/// MSI delivery mode: fixed (000).
/// Vector goes in bits 0-7 of the data register.
/// Delivery mode fixed + edge triggered = data = vector.
fn msi_data(vector: u8) -> u32 {
    // Delivery mode = 0 (fixed), trigger mode = 0 (edge), vector = low byte
    vector as u32
}

/// Walk PCI config space capability list to find MSI or MSI-X.
/// Returns (cap_offset, is_msix) or None.
fn find_cap(bus: u8, dev: u8, func: u8, cap_id: u8) -> Option<u8> {
    // Read Status register (offset 0x04) to check Capabilities List bit
    let status = pcie::pci_read32(bus, dev, func, 0x04);
    if (status >> 16) & (1 << 4) == 0 {
        return None; // No capabilities list
    }
    // Capabilities Pointer is at offset 0x34
    let mut cap_ptr = (pcie::pci_read32(bus, dev, func, 0x34) & 0xFF) as u8;
    for _ in 0..48 {
        if cap_ptr < 0x40 { break; }
        let cap = pcie::pci_read32(bus, dev, func, cap_ptr as u16);
        let id = (cap & 0xFF) as u8;
        if id == cap_id { return Some(cap_ptr); }
        cap_ptr = ((cap >> 8) & 0xFF) as u8;
        if cap_ptr == 0 { break; }
    }
    None
}

/// Enable MSI for a PCI device.
///
/// `bus`, `dev`, `func`: PCI device address.
/// `vector`: LAPIC vector for the interrupt (must be 32-255).
///
/// Returns true on success.
pub fn enable_msi(bus: u8, dev: u8, func: u8, vector: u8) -> bool {
    let cap = match find_cap(bus, dev, func, PCI_CAP_ID_MSI) {
        Some(c) => c,
        None => { crate::dev::console::serial_write("[msi] no MSI cap\n"); return false; }
    };

    let msi_addr: u32 = (MSI_ADDR_BASE & 0xFFFF_FFFF) as u32;
    let data = msi_data(vector);

    // Read Message Control (offset + 2)
        let ctrl = pcie::pci_read32(bus, dev, func, (cap as u16).wrapping_add(2));
        let is_64bit = (ctrl >> 23) & 1 != 0;

        // Write Message Address (offset + 4)
        pcie::pci_write32(bus, dev, func, (cap as u16).wrapping_add(4), msi_addr);

        if is_64bit {
            // Write upper address = 0
            pcie::pci_write32(bus, dev, func, (cap as u16).wrapping_add(8), 0);
            // Write Message Data (offset + 12)
            pcie::pci_write32(bus, dev, func, (cap as u16).wrapping_add(12), data);
        } else {
            // Write Message Data (offset + 8)
            pcie::pci_write32(bus, dev, func, (cap as u16).wrapping_add(8), data);
        }

        // Set MSI Enable bit (bit 0 of Message Control)
        pcie::pci_write32(bus, dev, func, (cap as u16).wrapping_add(2), ctrl | 1);

    true
}

/// Enable MSI-X for a PCI device.
/// Returns true on success.
pub fn enable_msix(_bus: u8, _dev: u8, _func: u8, _vector: u8) -> bool {
    // MSI-X requires BAR-based table access. Stub for now.
    crate::dev::console::serial_write("[msi] MSI-X stub\n");
    false
}

/// Initialize MSI subsystem.
pub fn init() {
    // Nothing to init globally — MSI is per-device
}

/// Convenience: enable MSI for a PciDevice from the scan result.
pub fn enable_for_device(device: &pcie::PciDevice, vector: u8) -> bool {
    enable_msi(device.bus, device.device, device.function, vector)
}
