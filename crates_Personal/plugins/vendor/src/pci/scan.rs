use super::config::{pci_read32, pci_read_bar5};

/// A discovered PCI device.
#[derive(Debug, Clone, Copy)]
pub struct PciDevice {
    pub bus: u8,
    pub device: u8,
    pub function: u8,
    pub vendor_id: u16,
    pub device_id: u16,
    pub class_code: u8,
    pub subclass: u8,
    pub prog_if: u8,
    pub bar0: u32,
    pub bar1: u32,
}

/// Result of a PCI bus scan.
pub struct PciScanResult {
    pub devices: [PciDevice; 64],
    pub count: usize,
}

impl PciScanResult {
    pub fn new() -> Self {
        Self {
            devices: [PciDevice {
                bus: 0, device: 0, function: 0,
                vendor_id: 0, device_id: 0,
                class_code: 0, subclass: 0, prog_if: 0,
                bar0: 0, bar1: 0,
            }; 64],
            count: 0,
        }
    }

    /// Find a device by vendor_id + device_id.
    pub fn find_device(&self, vendor: u16, device: u16) -> Option<&PciDevice> {
        self.devices[..self.count].iter().find(|d| {
            d.vendor_id == vendor && d.device_id == device
        })
    }

    /// Find first device by class/subclass.
    pub fn find_class(&self, class: u8, subclass: u8) -> Option<&PciDevice> {
        self.devices[..self.count].iter().find(|d| {
            d.class_code == class && d.subclass == subclass
        })
    }

    /// Check if any device matches class/subclass.
    pub fn has_class(&self, class: u8, subclass: u8) -> bool {
        self.find_class(class, subclass).is_some()
    }

    /// Get AHCI MMIO base (BAR5) from first AHCI controller.
    pub fn ahci_mmio(&self) -> Option<u64> {
        self.find_class(0x01, 0x06).map(|d| {
            pci_read_bar5(d.bus, d.device, d.function)
        })
    }
}

/// Scan the PCI bus using legacy IO ports.
pub fn scan_pci_bus() -> PciScanResult {
    let mut r = PciScanResult::new();

    for bus in 0..=255u16 {
        for dev in 0..32u8 {
            let vd = pci_read32(bus as u8, dev, 0, 0x00);
            let vendor = (vd & 0xFFFF) as u16;
            if vendor == 0xFFFF { continue; }
            let device_id = ((vd >> 16) & 0xFFFF) as u16;
            let cr = pci_read32(bus as u8, dev, 0, 0x08);
            let hdr = pci_read32(bus as u8, dev, 0, 0x0C);
            let multi = (hdr >> 16) & 0x80 != 0;
            let bar0 = pci_read32(bus as u8, dev, 0, 0x10);
            let bar1 = pci_read32(bus as u8, dev, 0, 0x14);
            let prog_if = ((cr >> 8) & 0xFF) as u8;

            if r.count < 64 {
                r.devices[r.count] = PciDevice {
                    bus: bus as u8, device: dev, function: 0,
                    vendor_id: vendor, device_id,
                    class_code: ((cr >> 24) & 0xFF) as u8,
                    subclass: ((cr >> 16) & 0xFF) as u8,
                    prog_if,
                    bar0, bar1,
                };
                r.count += 1;
            }

            if multi {
                for func in 1..8u8 {
                    let vd2 = pci_read32(bus as u8, dev, func, 0x00);
                    let v2 = (vd2 & 0xFFFF) as u16;
                    if v2 == 0xFFFF { continue; }
                    let cr2 = pci_read32(bus as u8, dev, func, 0x08);
                    let b0 = pci_read32(bus as u8, dev, func, 0x10);
                    let b1 = pci_read32(bus as u8, dev, func, 0x14);
                    if r.count < 64 {
                        r.devices[r.count] = PciDevice {
                            bus: bus as u8, device: dev, function: func,
                            vendor_id: v2,
                            device_id: ((vd2 >> 16) & 0xFFFF) as u16,
                            class_code: ((cr2 >> 24) & 0xFF) as u8,
                            subclass: ((cr2 >> 16) & 0xFF) as u8,
                            prog_if: ((cr2 >> 8) & 0xFF) as u8,
                            bar0: b0, bar1: b1,
                        };
                        r.count += 1;
                    }
                }
            }

            if bus > 0 && dev == 0 && r.count == 0 { break; }
        }
        if bus > 0 && r.count == 0 { break; }
    }

    r
}
