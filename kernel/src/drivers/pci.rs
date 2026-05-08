//! PCI bus scanning via ECAM (Enhanced Configuration Access Mechanism).

const NVIDIA_VENDOR: u16 = 0x10DE;

/// ECAM base address (set by init_ecam before scanning).
static mut ECAM_BASE: u64 = 0;
static mut ECAM_END_BUS: u8 = 0;

/// Initialize ECAM — must be called before any PCI access.
pub fn init_ecam(base: u64, end_bus: u8) {
    unsafe {
        ECAM_BASE = base;
        ECAM_END_BUS = end_bus;
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PciDevice {
    pub bus: u8,
    pub device: u8,
    pub function: u8,
    pub vendor_id: u16,
    pub device_id: u16,
    pub class_code: u8,
    pub subclass: u8,
    pub bar0: u32,
    pub bar1: u32,
}

pub struct PciScanResult {
    pub devices: [PciDevice; 64],
    pub count: usize,
}

impl PciScanResult {
    fn new() -> Self {
        Self {
            devices: [PciDevice {
                bus: 0, device: 0, function: 0,
                vendor_id: 0, device_id: 0,
                class_code: 0, subclass: 0,
                bar0: 0, bar1: 0,
            }; 64],
            count: 0,
        }
    }

    pub fn find_nvidia_gpu(&self) -> Option<&PciDevice> {
        self.devices[..self.count].iter().find(|d| {
            d.vendor_id == NVIDIA_VENDOR && d.class_code == 0x03
        })
    }

    pub fn find_device(&self, vendor: u16, device: u16) -> Option<&PciDevice> {
        self.devices[..self.count].iter().find(|d| {
            d.vendor_id == vendor && d.device_id == device
        })
    }
}

/// Read 32 bits from PCI config space via ECAM MMIO.
pub fn pci_read32(bus: u8, dev: u8, func: u8, off: u16) -> u32 {
    let base = unsafe { ECAM_BASE };
    if base == 0 { return 0xFFFF_FFFF; }
    let offset = ((bus as u64) << 20)
        | ((dev as u64) << 15)
        | ((func as u64) << 12)
        | ((off as u64) & 0xFFC);
    let addr = base + offset;
    unsafe { core::ptr::read_volatile(addr as *const u32) }
}

/// Write 32 bits to PCI config space via ECAM MMIO.
pub fn pci_write32(bus: u8, dev: u8, func: u8, off: u16, val: u32) {
    let base = unsafe { ECAM_BASE };
    if base == 0 { return; }
    let offset = ((bus as u64) << 20)
        | ((dev as u64) << 15)
        | ((func as u64) << 12)
        | ((off as u64) & 0xFFC);
    let addr = base + offset;
    unsafe { core::ptr::write_volatile(addr as *mut u32, val); }
}

pub fn scan_pci_bus() -> PciScanResult {
    let mut r = PciScanResult::new();
    let end_bus = unsafe { ECAM_END_BUS };

    for bus in 0..=end_bus {
        for dev in 0..32u8 {
            let vd = pci_read32(bus, dev, 0, 0x00);
            let vendor = (vd & 0xFFFF) as u16;
            if vendor == 0xFFFF { continue; }

            let device_id = ((vd >> 16) & 0xFFFF) as u16;
            let cr = pci_read32(bus, dev, 0, 0x08);
            let hdr = pci_read32(bus, dev, 0, 0x0C);
            let multi = (hdr >> 16) & 0x80 != 0;

            let bar0 = pci_read32(bus, dev, 0, 0x10);
            let bar1 = pci_read32(bus, dev, 0, 0x14);

            if r.count < 64 {
                r.devices[r.count] = PciDevice {
                    bus, device: dev, function: 0,
                    vendor_id: vendor, device_id,
                    class_code: ((cr >> 24) & 0xFF) as u8,
                    subclass: ((cr >> 16) & 0xFF) as u8,
                    bar0, bar1,
                };
                r.count += 1;
            }

            if multi {
                for func in 1..8u8 {
                    let vd2 = pci_read32(bus, dev, func, 0x00);
                    let v2 = (vd2 & 0xFFFF) as u16;
                    if v2 == 0xFFFF { continue; }
                    let cr2 = pci_read32(bus, dev, func, 0x08);
                    let b0 = pci_read32(bus, dev, func, 0x10);
                    let b1 = pci_read32(bus, dev, func, 0x14);
                    if r.count < 64 {
                        r.devices[r.count] = PciDevice {
                            bus, device: dev, function: func,
                            vendor_id: v2,
                            device_id: ((vd2 >> 16) & 0xFFFF) as u16,
                            class_code: ((cr2 >> 24) & 0xFF) as u8,
                            subclass: ((cr2 >> 16) & 0xFF) as u8,
                            bar0: b0, bar1: b1,
                        };
                        r.count += 1;
                    }
                }
            }
        }
    }
    r
}
