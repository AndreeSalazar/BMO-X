//! PCI enumeration — stub.
//!
//! The boot chain's `stage3_dev` fills `ctx.pci_devices[]` and
//! `ctx.ioapic_base` / `ctx.hpet_base`. By the time the kernel runs,
//! there's no need to re-enumerate. This stub exists for API parity.

#[derive(Debug, Clone, Copy, Default)]
pub struct PciDeviceLite {
    pub bus: u8, pub device: u8, pub function: u8,
    pub vendor_id: u16, pub device_id: u16,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PciScan {
    pub count: usize,
}

pub fn init_ecam(_base: u64, _end_bus: u8) {}
pub fn scan_pci_bus() -> PciScan { PciScan { count: 0 } }
pub fn find_xhci_mmio() -> Option<u64> { None }
pub fn find_all_xhci_mmio() -> (Option<u64>, Option<u64>) { (None, None) }
