//! exFAT filesystem driver for FastOS.
//!
//! exFAT (Extended File Allocation Table) is used for the user data
//! partition. It supports large files (>4GB), long filenames, and is
//! cross-platform compatible.
//!
//! Status: Stub — awaiting disk driver wiring.

#![allow(dead_code)]

use crate::dev::console;

const SECTOR_SIZE: usize = 512;
const MAX_OPEN_FILES: usize = 32;

/// exFAT error type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExfatError {
    IoError,
    BadSignature,
    InvalidCluster,
    NoSpace,
    FileNotFound,
    NotADirectory,
    TooManyOpen,
}

/// On-disk exFAT volume header (sector 0).
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ExfatVolumeHeader {
    pub jump_boot: [u8; 3],
    pub fs_name: [u8; 8],
    pub must_be_zero: [u8; 53],
    pub partition_offset: u64,
    pub volume_length: u64,
    pub fat_offset: u32,
    pub fat_length: u32,
    pub cluster_heap_offset: u32,
    pub cluster_count: u32,
    pub first_cluster: u32,
    pub volume_flags: u16,
    pub bytes_per_sector_shift: u8,
    pub sectors_per_cluster_shift: u8,
    pub num_fats: u8,
    pub drive_select: u8,
    percent_in_use: u8,
    reserved: [u8; 7],
    pub boot_code: [u8; 390],
    pub boot_sign: u16,
}

impl ExfatVolumeHeader {
    pub fn is_valid(&self) -> bool {
        &self.fs_name == b"EXFAT   "
    }

    pub fn sector_size(&self) -> usize {
        1usize << self.bytes_per_sector_shift
    }

    pub fn cluster_size(&self) -> usize {
        self.sector_size() << self.sectors_per_cluster_shift
    }
}

/// Directory entry for exFAT files.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ExfatDirEntry {
    pub entry_type: u8,
    pub general_flags: u8,
    reserved1: [u8; 18],
    pub first_cluster: u32,
    pub data_length: u64,
}

impl ExfatDirEntry {
    pub fn is_file(&self) -> bool {
        (self.entry_type & 0x30) == 0x10
    }

    pub fn is_dir(&self) -> bool {
        (self.entry_type & 0x30) == 0x20
    }

    pub fn is_end(&self) -> bool {
        self.entry_type == 0x00
    }
}

/// Open file descriptor.
#[derive(Debug, Clone, Copy)]
struct ExfatOpenFile {
    in_use: bool,
    first_cluster: u32,
    file_size: u64,
    position: u64,
}

static mut VOLUME_HEADER: ExfatVolumeHeader = ExfatVolumeHeader {
    jump_boot: [0; 3],
    fs_name: [0; 8],
    must_be_zero: [0; 53],
    partition_offset: 0,
    volume_length: 0,
    fat_offset: 0,
    fat_length: 0,
    cluster_heap_offset: 0,
    cluster_count: 0,
    first_cluster: 0,
    volume_flags: 0,
    bytes_per_sector_shift: 0,
    sectors_per_cluster_shift: 0,
    num_fats: 0,
    drive_select: 0,
    percent_in_use: 0,
    reserved: [0; 7],
    boot_code: [0; 390],
    boot_sign: 0,
};

static mut OPEN_FILES: [ExfatOpenFile; MAX_OPEN_FILES] = [ExfatOpenFile {
    in_use: false,
    first_cluster: 0,
    file_size: 0,
    position: 0,
}; MAX_OPEN_FILES];

static mut MOUNTED: bool = false;

/// Initialize exFAT from a disk reader at the given LBA.
pub fn init_from_disk(
    _lba: u64,
    _disk: &mut dyn crate::bmo_core::fs::DiskReader,
) -> Result<(), ExfatError> {
    // TODO: read sector 0, validate signature, populate VOLUME_HEADER
    console::serial_write("[exfat] Stub: init_from_disk not yet implemented\n");
    Ok(())
}

/// Mount the exFAT volume.
pub fn mount() -> Result<(), ExfatError> {
    if unsafe { MOUNTED } {
        return Err(ExfatError::IoError);
    }
    unsafe { MOUNTED = true; }
    console::serial_write("[exfat] Mounted (stub)\n");
    Ok(())
}

/// Unmount.
pub fn unmount() -> Result<(), ExfatError> {
    if !unsafe { MOUNTED } {
        return Err(ExfatError::IoError);
    }
    unsafe { MOUNTED = false; }
    console::serial_write("[exfat] Unmounted\n");
    Ok(())
}

pub fn is_mounted() -> bool {
    unsafe { MOUNTED }
}
