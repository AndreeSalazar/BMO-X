//! PCI bus scanning via I/O ports 0xCF8/0xCFC.

const PCI_CONFIG_ADDR: u16 = 0x0CF8;
const PCI_CONFIG_DATA: u16 = 0x0CFC;

const NVIDIA_VENDOR: u16 = 0x10DE;

#[derive(Debug, Clone, Copy)]
pub struct PciDevice {
    pub bus: u8,
    pub device: u8,
    pub function: u8,
    pub vendor_id: u16,
    pub device_id: u16,
    pub class_code: u8,
    pub subclass: u8,
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
            }; 64],
            count: 0,
        }
    }

    pub fn find_nvidia_gpu(&self) -> Option<&PciDevice> {
        self.devices[..self.count].iter().find(|d| {
            d.vendor_id == NVIDIA_VENDOR && d.class_code == 0x03
        })
    }
}

fn outl(port: u16, val: u32) {
    unsafe { core::arch::asm!("out dx, eax", in("dx") port, in("eax") val); }
}

fn inl(port: u16) -> u32 {
    let v: u32;
    unsafe { core::arch::asm!("in eax, dx", in("dx") port, out("eax") v); }
    v
}

pub fn pci_read32(bus: u8, dev: u8, func: u8, off: u8) -> u32 {
    let addr = 0x8000_0000u32
        | ((bus as u32) << 16)
        | ((dev as u32) << 11)
        | ((func as u32) << 8)
        | ((off as u32) & 0xFC);
    outl(PCI_CONFIG_ADDR, addr);
    inl(PCI_CONFIG_DATA)
}

pub fn scan_pci_bus() -> PciScanResult {
    let mut r = PciScanResult::new();

    for bus in 0..=255u8 {
        for dev in 0..32u8 {
            let vd = pci_read32(bus, dev, 0, 0x00);
            let vendor = (vd & 0xFFFF) as u16;
            if vendor == 0xFFFF { continue; }

            let device_id = ((vd >> 16) & 0xFFFF) as u16;
            let cr = pci_read32(bus, dev, 0, 0x08);

            if r.count < 64 {
                r.devices[r.count] = PciDevice {
                    bus, device: dev, function: 0,
                    vendor_id: vendor, device_id,
                    class_code: ((cr >> 24) & 0xFF) as u8,
                    subclass: ((cr >> 16) & 0xFF) as u8,
                };
                r.count += 1;
            }

            for func in 1..8u8 {
                let vd2 = pci_read32(bus, dev, func, 0x00);
                let v2 = (vd2 & 0xFFFF) as u16;
                if v2 == 0xFFFF { continue; }
                let cr2 = pci_read32(bus, dev, func, 0x08);
                if r.count < 64 {
                    r.devices[r.count] = PciDevice {
                        bus, device: dev, function: func,
                        vendor_id: v2,
                        device_id: ((vd2 >> 16) & 0xFFFF) as u16,
                        class_code: ((cr2 >> 24) & 0xFF) as u8,
                        subclass: ((cr2 >> 16) & 0xFF) as u8,
                    };
                    r.count += 1;
                }
            }
        }
    }
    r
}
