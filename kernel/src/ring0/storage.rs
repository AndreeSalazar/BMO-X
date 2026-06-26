//! Storage subsystem — SSD access via vendor crate.
//!
//! Provides:
//! - AHCI disk detection and initialization
//! - Block read/write operations
//! - Kernel logging to SSD

/// Global AHCI disk instance.
static mut DISK: Option<vendor::ahci::AhciDisk> = None;

/// Initialize AHCI disk from PCI BAR5 address.
pub unsafe fn init_ahci(mmio: usize) -> bool {
    crate::dev::console::serial_write("[storage] Initializing AHCI at 0x");
    crate::dev::console::serial_write(&alloc::format!("{:x}", mmio));
    crate::dev::console::serial_write("\n");

    match vendor::ahci::AhciDisk::try_new(mmio) {
        Some(disk) => {
            crate::dev::console::serial_write("[storage] AHCI disk found! capacity=");
            crate::dev::console::serial_write(&alloc::format!("{}", disk.capacity()));
            crate::dev::console::serial_write(" block_size=");
            crate::dev::console::serial_write(&alloc::format!("{}", disk.block_size()));
            crate::dev::console::serial_write("\n");
            DISK = Some(disk);
            true
        }
        None => {
            crate::dev::console::serial_write("[storage] No SATA device found on AHCI\n");
            false
        }
    }
}

/// Read sectors from SSD.
pub unsafe fn read_sectors(lba: u64, count: u16, buf: &mut [u8]) -> bool {
    if let Some(ref mut disk) = DISK {
        disk.read(lba, buf)
    } else {
        false
    }
}

/// Write sectors to SSD.
pub unsafe fn write_sectors(lba: u64, count: u16, buf: &[u8]) -> bool {
    if let Some(ref mut disk) = DISK {
        disk.write(lba, buf)
    } else {
        false
    }
}

/// Check if SSD is available.
pub fn is_available() -> bool {
    unsafe { DISK.is_some() }
}

/// Get disk capacity in sectors.
pub fn max_lba() -> u64 {
    unsafe { DISK.as_ref().map(|d| d.capacity()).unwrap_or(0) }
}

/// Get disk block size.
pub fn block_size() -> usize {
    unsafe { DISK.as_ref().map(|d| d.block_size()).unwrap_or(512) }
}

/// Write a log message to SSD.
pub unsafe fn write_log(msg: &str) -> bool {
    if !is_available() {
        return false;
    }

    let data = msg.as_bytes();
    let bs = block_size();
    let sectors = ((data.len() + bs - 1) / bs) as u64;

    if sectors == 0 { return true; }

    let mut buf = alloc::vec![0u8; sectors as usize * bs];
    buf[..data.len()].copy_from_slice(data);

    // Write to LBA 0x10000 (safe area away from partition table)
    write_sectors(0x10000, sectors as u16, &buf)
}
