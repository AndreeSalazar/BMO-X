//! Block I/O Layer (Ring 0 HAL).
//!
//! Unified block device abstraction for storage. File systems consume
//! this API — they don't know about AHCI or NVMe details.
//!
//! Architecture:
//!   - `BlockDevice` trait: read/write sectors to any backend
//!   - `BlockRequest`: queued I/O request with status tracking
//!   - Multiple backends: AHCI, NVMe, RAM disk (for testing)
//!
//! The block I/O layer sits between:
//!   Storage drivers (AHCI/NVMe) ← → Block I/O ← → File systems (FAT32, etc.)

/// Default sector size (512 bytes for AHCI, configurable for NVMe).
pub const SECTOR_SIZE: u32 = 512;

/// Block I/O request type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockOp {
    Read,
    Write,
}

/// Block I/O request status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockStatus {
    Pending,
    InProgress,
    Complete,
    Error,
}

/// A single block I/O request.
#[derive(Debug, Clone, Copy)]
pub struct BlockRequest {
    pub id: u32,
    pub op: BlockOp,
    pub lba: u64,
    pub sector_count: u32,
    pub buffer_phys: u64,
    pub status: BlockStatus,
}

/// Trait for block devices. Implemented by AHCI/NVMe/RAM disk drivers.
pub trait BlockDevice {
    /// Device name (e.g., "sda", "nvme0n1").
    fn name(&self) -> &str;

    /// Total sectors on the device.
    fn total_sectors(&self) -> u64;

    /// Sector size in bytes.
    fn sector_size(&self) -> u32;

    /// Read sectors from the device.
    /// `lba`: starting logical block address
    /// `count`: number of sectors
    /// `buffer_phys`: physical address of destination buffer
    fn read_sectors(&self, lba: u64, count: u32, buffer_phys: u64) -> Result<(), BlockError>;

    /// Write sectors to the device.
    fn write_sectors(&self, lba: u64, count: u32, buffer_phys: u64) -> Result<(), BlockError>;

    /// Flush any pending writes to the device.
    fn flush(&self) -> Result<(), BlockError>;
}

/// Block I/O error codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockError {
    NotFound,
    IOError,
    Timeout,
    InvalidLBA,
    InvalidSectorCount,
    DeviceFull,
    DeviceBusy,
}

/// Registered block devices.
static mut DEVICES: [Option<&'static dyn BlockDevice>; 16] = [None; 16];
static mut DEVICE_COUNT: u32 = 0;

/// Register a block device.
pub fn register(device: &'static dyn BlockDevice) -> Result<u32, BlockError> {
    unsafe {
        let count = DEVICE_COUNT as usize;
        if count >= 16 {
            return Err(BlockError::DeviceFull);
        }
        DEVICES[count] = Some(device);
        DEVICE_COUNT += 1;
        Ok(count as u32)
    }
}

/// Get total number of registered block devices.
pub fn device_count() -> u32 {
    unsafe { DEVICE_COUNT }
}

/// Get a block device by index.
pub fn get_device(index: u32) -> Option<&'static dyn BlockDevice> {
    unsafe {
        if index as usize >= DEVICE_COUNT as usize {
            return None;
        }
        DEVICES[index as usize]
    }
}

/// Find a block device by name.
pub fn find_device(name: &str) -> Option<&'static dyn BlockDevice> {
    unsafe {
        for i in 0..DEVICE_COUNT as usize {
            if let Some(dev) = DEVICES[i] {
                if dev.name() == name {
                    return Some(dev);
                }
            }
        }
        None
    }
}

/// Read sectors from a block device by name.
pub fn read(name: &str, lba: u64, count: u32, buffer_phys: u64) -> Result<(), BlockError> {
    let dev = find_device(name).ok_or(BlockError::NotFound)?;
    dev.read_sectors(lba, count, buffer_phys)
}

/// Write sectors to a block device by name.
pub fn write(name: &str, lba: u64, count: u32, buffer_phys: u64) -> Result<(), BlockError> {
    let dev = find_device(name).ok_or(BlockError::NotFound)?;
    dev.write_sectors(lba, count, buffer_phys)
}
