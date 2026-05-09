//! GPT Partition Table Parser — finds the Windows NTFS partition on a GPT disk.
//!
//! Reads GPT header at LBA 1, then scans partition entries to find
//! the Microsoft Basic Data partition (GUID: EBD0A0A2-B9E5-4433-87C0-68B6B72699C7).

use crate::fs::{DiskReader, DiskError};

/// Microsoft Basic Data partition type GUID (little-endian mixed encoding)
/// GUID: EBD0A0A2-B9E5-4433-87C0-68B6B72699C7
const MS_BASIC_DATA_GUID: [u8; 16] = [
    0xA2, 0xA0, 0xD0, 0xEB, // time_low (LE)
    0xE5, 0xB9,             // time_mid (LE)
    0x33, 0x44,             // time_hi (LE)
    0x87, 0xC0,             // clock_seq (BE)
    0x68, 0xB6, 0xB7, 0x26, 0x99, 0xC7, // node (BE)
];

/// EFI System Partition type GUID
/// GUID: C12A7328-F81F-11D2-BA4B-00A0C93EC93B
const EFI_SYSTEM_GUID: [u8; 16] = [
    0x28, 0x73, 0x2A, 0xC1,
    0x1F, 0xF8,
    0xD2, 0x11,
    0xBA, 0x4B,
    0x00, 0xA0, 0xC9, 0x3E, 0xC9, 0x3B,
];

/// Result of GPT partition scan.
pub struct GptPartition {
    pub type_guid: [u8; 16],
    pub first_lba: u64,
    pub last_lba: u64,
    pub name: [u16; 36],
}

impl GptPartition {
    pub fn is_ms_basic_data(&self) -> bool {
        self.type_guid == MS_BASIC_DATA_GUID
    }

    pub fn is_efi_system(&self) -> bool {
        self.type_guid == EFI_SYSTEM_GUID
    }

    pub fn is_empty(&self) -> bool {
        self.type_guid == [0u8; 16]
    }

    /// Size in sectors.
    pub fn size_sectors(&self) -> u64 {
        self.last_lba - self.first_lba + 1
    }

    /// Simple name extraction (first 16 ASCII chars from UTF-16LE name).
    pub fn name_ascii(&self) -> [u8; 36] {
        let mut out = [0u8; 36];
        for (i, &wch) in self.name.iter().enumerate() {
            if wch == 0 { break; }
            out[i] = if wch < 128 { wch as u8 } else { b'?' };
        }
        out
    }
}

/// Scan the GPT partition table and find the first NTFS (Microsoft Basic Data) partition.
/// Returns the starting LBA of the Windows partition.
pub fn find_ntfs_partition<D: DiskReader>(disk: &mut D) -> Result<u64, DiskError> {
    // Read LBA 1 (GPT Header)
    let mut header = alloc::vec::Vec::with_capacity(512);
    header.resize(512, 0);
    disk.read_sectors(1, 1, &mut header)?;

    // Validate GPT signature "EFI PART"
    if &header[0..8] != b"EFI PART" {
        return Err(DiskError::ControllerError);
    }

    let partition_entry_lba = read_u64(&header, 72);
    let num_entries = read_u32(&header, 80) as usize;
    let entry_size = read_u32(&header, 84) as usize;

    // Sanity checks
    if entry_size < 128 || num_entries == 0 || num_entries > 256 {
        return Err(DiskError::ControllerError);
    }

    // Read partition entries (typically at LBA 2, 128 bytes each, 128 entries)
    let entries_per_sector = 512 / entry_size;
    let sectors_needed = (num_entries + entries_per_sector - 1) / entries_per_sector;

    let mut sector_buf = alloc::vec::Vec::with_capacity(512);
    sector_buf.resize(512, 0);
    let mut best_ntfs_lba: Option<u64> = None;
    let mut best_size: u64 = 0;

    for sec in 0..sectors_needed {
        disk.read_sectors(partition_entry_lba + sec as u64, 1, &mut sector_buf)?;

        for idx in 0..entries_per_sector {
            let entry_offset = idx * entry_size;
            if entry_offset + 128 > 512 {
                break;
            }

            let entry = &sector_buf[entry_offset..entry_offset + entry_size.min(512 - entry_offset)];

            // Parse type GUID (bytes 0-15)
            let mut type_guid = [0u8; 16];
            type_guid.copy_from_slice(&entry[0..16]);

            // Skip empty entries
            if type_guid == [0u8; 16] {
                continue;
            }

            let first_lba = read_u64(entry, 32);
            let last_lba = read_u64(entry, 40);

            // Check if this is a Microsoft Basic Data partition
            if type_guid == MS_BASIC_DATA_GUID {
                let size = last_lba - first_lba + 1;
                // Pick the largest NTFS partition (usually the main Windows partition)
                if size > best_size {
                    best_size = size;
                    best_ntfs_lba = Some(first_lba);
                }
            }
        }
    }

    best_ntfs_lba.ok_or(DiskError::ControllerError)
}

/// Scan GPT and return all non-empty partitions (for diagnostic display).
pub fn scan_all_partitions<D: DiskReader>(disk: &mut D) -> Result<alloc::vec::Vec<GptPartition>, DiskError> {
    let mut parts = alloc::vec::Vec::new();

    let mut header = alloc::vec::Vec::with_capacity(512);
    header.resize(512, 0);
    disk.read_sectors(1, 1, &mut header)?;

    if &header[0..8] != b"EFI PART" {
        return Err(DiskError::ControllerError);
    }

    let partition_entry_lba = read_u64(&header, 72);
    let num_entries = read_u32(&header, 80) as usize;
    let entry_size = read_u32(&header, 84) as usize;

    if entry_size < 128 || num_entries == 0 || num_entries > 256 {
        return Err(DiskError::ControllerError);
    }

    let entries_per_sector = 512 / entry_size;
    let sectors_needed = (num_entries + entries_per_sector - 1) / entries_per_sector;

    let mut sector_buf = alloc::vec::Vec::with_capacity(512);
    sector_buf.resize(512, 0);

    for sec in 0..sectors_needed {
        disk.read_sectors(partition_entry_lba + sec as u64, 1, &mut sector_buf)?;

        for idx in 0..entries_per_sector {
            let off = idx * entry_size;
            if off + 128 > 512 { break; }
            let entry = &sector_buf[off..];

            let mut type_guid = [0u8; 16];
            type_guid.copy_from_slice(&entry[0..16]);

            if type_guid == [0u8; 16] { continue; }

            let first_lba = read_u64(entry, 32);
            let last_lba = read_u64(entry, 40);

            let mut name = [0u16; 36];
            for i in 0..36 {
                let o = 56 + i * 2;
                if o + 1 < entry_size.min(512 - off) {
                    name[i] = u16::from_le_bytes([entry[o], entry[o + 1]]);
                }
            }

            parts.push(GptPartition {
                type_guid,
                first_lba,
                last_lba,
                name,
            });
        }
    }

    Ok(parts)
}

fn read_u32(data: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([data[off], data[off+1], data[off+2], data[off+3]])
}

fn read_u64(data: &[u8], off: usize) -> u64 {
    u64::from_le_bytes([
        data[off], data[off+1], data[off+2], data[off+3],
        data[off+4], data[off+5], data[off+6], data[off+7],
    ])
}
