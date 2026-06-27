//! Storage Manager (Ring 0 HAL).
//!
//! Pure hardware module — manages all storage controllers:
//!   - AHCI/SATA: Command list, FIS, PRDT, port init
//!   - NVMe: Submission/completion queues, namespaces, LBA R/W
//!   - Block I/O: Unified block device abstraction for filesystems
//!
//! Architecture:
//!   - Each storage driver exposes a `BlockDevice` trait
//!   - Block I/O layer multiplexes requests to any backend
//!   - File systems consume `BlockDevice` — they don't know about AHCI/NVMe
//!
//! Init order (called from phase2_dev):
//!   1. PCI scan finds AHCI/NVMe controllers
//!   2. AHCI/NVMe drivers probe MMIO + IRQ
//!   3. Block I/O registers discovered devices
//!   4. File systems can then open block devices

#![allow(dead_code)]

#![allow(dead_code)]

/// Storage controller type detected by PCI scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageType {
    Ahci,
    Nvme,
    Unknown,
}

/// Information about a discovered storage controller.
#[derive(Debug, Clone, Copy)]
pub struct StorageDevice {
    pub id: u32,
    pub storage_type: StorageType,
    pub pci_bus: u8,
    pub pci_device: u8,
    pub pci_function: u8,
    pub mmio_base: u64,
    pub irq: u8,
    pub port_count: u8,
    pub sectors_total: u64,
}

/// Global storage device table.
static mut DEVICES: [StorageDevice; 16] = {
    const EMPTY: StorageDevice = StorageDevice {
        id: 0,
        storage_type: StorageType::Unknown,
        pci_bus: 0,
        pci_device: 0,
        pci_function: 0,
        mmio_base: 0,
        irq: 0,
        port_count: 0,
        sectors_total: 0,
    };
    [EMPTY; 16]
};
static mut DEVICE_COUNT: u32 = 0;

/// Register a discovered storage controller.
pub unsafe fn register_device(dev: StorageDevice) -> u32 {
    let count = DEVICE_COUNT as usize;
    if count >= 16 { return u32::MAX; }
    DEVICES[count] = dev;
    DEVICE_COUNT += 1;
    dev.id
}

/// Get total number of registered storage devices.
pub fn device_count() -> u32 {
    unsafe { DEVICE_COUNT }
}

/// Get storage device by index.
pub fn get_device(index: u32) -> Option<&'static StorageDevice> {
    unsafe {
        if index as usize >= DEVICE_COUNT as usize { return None; }
        Some(&DEVICES[index as usize])
    }
}

/// Find storage device by type.
pub fn find_by_type(stype: StorageType) -> Option<&'static StorageDevice> {
    unsafe {
        for i in 0..DEVICE_COUNT as usize {
            if DEVICES[i].storage_type == stype {
                return Some(&DEVICES[i]);
            }
        }
        None
    }
}
