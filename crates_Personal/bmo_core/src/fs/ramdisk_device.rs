//! RAM-backed block device for testing filesystem drivers.
//!
//! Provides a 8 MB in-memory block device implementing DiskReader + DiskWriter.

#![allow(dead_code)]

use crate::fs::{DiskError, DiskReader, DiskWriter};
use crate::dev::console;

const DEVICE_SECTORS: usize = 16384;
const SECTOR_SIZE: usize = 512;

static mut DEVICE_DATA: [[u8; SECTOR_SIZE]; DEVICE_SECTORS] = [[0u8; SECTOR_SIZE]; DEVICE_SECTORS];
static mut DEVICE_INITIALIZED: bool = false;
static mut READ_COUNT: u64 = 0;
static mut WRITE_COUNT: u64 = 0;

pub struct RamDiskDevice {
    sector_count: u64,
}

impl RamDiskDevice {
    pub fn new() -> Self {
        Self { sector_count: DEVICE_SECTORS as u64 }
    }

    pub fn sector_count(&self) -> u64 { self.sector_count }
    pub fn read_total(&self) -> u64 { unsafe { READ_COUNT } }
    pub fn write_total(&self) -> u64 { unsafe { WRITE_COUNT } }

    pub fn init_with_exfat_image(&mut self) {
        unsafe {
            if DEVICE_INITIALIZED { return; }
            DEVICE_INITIALIZED = true;

            DEVICE_DATA[0][3..11].copy_from_slice(b"EXFAT   ");
            DEVICE_DATA[0][72..80].copy_from_slice(&(DEVICE_SECTORS as u64).to_le_bytes());

            let fat_offset: u32 = 8;
            let fat_length: u32 = 8;
            let cluster_heap_offset: u32 = fat_offset + fat_length;
            let cluster_count: u32 = 128;
            let first_cluster: u32 = 2;

            DEVICE_DATA[0][84..88].copy_from_slice(&fat_offset.to_le_bytes());
            DEVICE_DATA[0][88..92].copy_from_slice(&fat_length.to_le_bytes());
            DEVICE_DATA[0][92..96].copy_from_slice(&cluster_heap_offset.to_le_bytes());
            DEVICE_DATA[0][96..100].copy_from_slice(&cluster_count.to_le_bytes());
            DEVICE_DATA[0][100..104].copy_from_slice(&first_cluster.to_le_bytes());
            DEVICE_DATA[0][108] = 9;
            DEVICE_DATA[0][109] = 0;
            DEVICE_DATA[0][110] = 1;
            DEVICE_DATA[0][510..512].copy_from_slice(&0xAA55u16.to_le_bytes());

            let eoc: u32 = 0xFFFFFFF8;
            let fat_lba = 8usize;
            let root_off = (2usize) * 4;
            DEVICE_DATA[fat_lba][root_off..root_off + 4].copy_from_slice(&eoc.to_le_bytes());

            let root_lba = cluster_heap_offset as usize;
            DEVICE_DATA[root_lba][0] = 0x85;
            DEVICE_DATA[root_lba][1] = 0x20;
            DEVICE_DATA[root_lba][2..13].copy_from_slice(b"DATOS   MD");
            let cl3: u32 = 3;
            DEVICE_DATA[root_lba][20..24].copy_from_slice(&cl3.to_le_bytes());

            let file_content = b"# BMO - Datos\n\nThis file is stored on the exFAT data partition.\n";
            let data_lba = cluster_heap_offset as usize + 1;
            let content_len = file_content.len().min(512);
            DEVICE_DATA[data_lba][..content_len].copy_from_slice(&file_content[..content_len]);
            let file_size = content_len as u64;
            DEVICE_DATA[root_lba][24..32].copy_from_slice(&file_size.to_le_bytes());

            console::serial_write("[ramdisk_dev] Initialized with exFAT image\n");
        }
    }
}

impl DiskReader for RamDiskDevice {
    fn read_sectors(&mut self, lba: u64, count: u32, buf: &mut [u8]) -> Result<(), DiskError> {
        let start = lba as usize;
        let end = start + count as usize;
        if end > DEVICE_SECTORS { return Err(DiskError::InvalidArgument); }
        let mut offset = 0usize;
        for i in start..end {
            let copy_len = SECTOR_SIZE.min(buf.len() - offset);
            unsafe {
                buf[offset..offset + copy_len].copy_from_slice(&DEVICE_DATA[i][..copy_len]);
                READ_COUNT += 1;
            }
            offset += copy_len;
            if offset >= buf.len() { break; }
        }
        Ok(())
    }
}

impl DiskWriter for RamDiskDevice {
    fn write_sectors(&mut self, lba: u64, count: u32, buf: &[u8]) -> Result<(), DiskError> {
        let start = lba as usize;
        let end = start + count as usize;
        if end > DEVICE_SECTORS { return Err(DiskError::InvalidArgument); }
        let mut offset = 0usize;
        for i in start..end {
            let copy_len = SECTOR_SIZE.min(buf.len() - offset);
            unsafe {
                DEVICE_DATA[i][..copy_len].copy_from_slice(&buf[offset..offset + copy_len]);
                WRITE_COUNT += 1;
            }
            offset += copy_len;
            if offset >= buf.len() { break; }
        }
        Ok(())
    }
}

