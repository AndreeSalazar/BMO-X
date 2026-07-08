//! FAT32 filesystem reader — minimal read-only implementation.
//!
//! Reads the BIOS Parameter Block (BPB), locates the root directory,
//! finds files by name, and reads their clusters via the FAT chain.

#![allow(dead_code)]

use crate::dev::ahci;

/// FAT32 BPB (BIOS Parameter Block) at sector 0, offset 11.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct FatBpb {
    pub jmp: [u8; 3],
    pub oem: [u8; 8],
    pub bytes_per_sector: u16,
    pub sectors_per_cluster: u8,
    pub reserved_sectors: u16,
    pub num_fats: u8,
    _root_entries: u16,        // unused in FAT32
    _total_sectors16: u16,     // unused in FAT32
    pub media: u8,
    _fat_size16: u16,          // unused in FAT32
    pub sectors_per_track: u16,
    pub num_heads: u16,
    pub hidden_sectors: u32,
    pub total_sectors: u32,
    pub fat_size: u32,
    pub ext_flags: u16,
    pub fs_version: u16,
    pub root_cluster: u32,
    pub fs_info: u16,
    pub backup_boot_sector: u16,
    _reserved: [u8; 12],
    pub drive_number: u8,
    _reserved1: u8,
    pub boot_sig: u8,
    pub volume_id: u32,
    pub volume_label: [u8; 11],
    pub fs_type: [u8; 8],
}

/// FAT32 directory entry (short name).
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct DirEntry {
    pub name: [u8; 11],        // 8.3 format
    pub attr: u8,
    _nt_reserved: u8,
    _create_time_tenth: u8,
    pub create_time: u16,
    pub create_date: u16,
    pub last_access: u16,
    pub first_cluster_hi: u16,
    pub write_time: u16,
    pub write_date: u16,
    pub first_cluster_lo: u16,
    pub file_size: u32,
}

/// Mounted FAT32 volume.
pub struct FatVolume {
    pub port: u8,
    bytes_per_sector: u16,
    sectors_per_cluster: u8,
    fat_start: u32,
    data_start: u32,
    root_cluster: u32,
    buf: [u8; 512],
    fat_cache: [u8; 512],
}

/// Try to mount a FAT32 volume on AHCI port.
pub fn mount(port: u8) -> Option<FatVolume> {
    let mut buf = [0u8; 512];
    unsafe {
        let read = ahci::read_sectors(port, 0, 1, buf.as_mut_ptr());
        if read != 1 { return None; }
    }

    // Parse BPB
    let bpb = unsafe { &*(buf.as_ptr() as *const FatBpb) };

    // Verify FAT32 signature
    if bpb.bytes_per_sector != 512 { return None; }
    if bpb.boot_sig != 0x29 && bpb.boot_sig != 0x28 { return None; }

    let fat_start = bpb.reserved_sectors as u32;
    let fat_size_sectors = bpb.fat_size;
    let data_start = fat_start + (bpb.num_fats as u32) * fat_size_sectors;

    crate::dev::console::serial_write("[fat32] mounted: sectors/cluster=");
    crate::dev::console::serial_write_u64(bpb.sectors_per_cluster as u64, 10);
    crate::dev::console::serial_write(" root_cluster=");
    crate::dev::console::serial_write_u64(bpb.root_cluster as u64, 10);
    crate::dev::console::serial_write("\n");

    Some(FatVolume {
        port,
        bytes_per_sector: bpb.bytes_per_sector,
        sectors_per_cluster: bpb.sectors_per_cluster,
        fat_start,
        data_start,
        root_cluster: bpb.root_cluster,
        buf: [0u8; 512],
        fat_cache: [0u8; 512],
    })
}

impl FatVolume {
    /// Cluster number → LBA (sector number).
    fn cluster_to_lba(&self, cluster: u32) -> u64 {
        (self.data_start as u64) + ((cluster as u64 - 2) * self.sectors_per_cluster as u64)
    }

    /// Read the FAT entry for a cluster.
    fn read_fat_entry(&mut self, cluster: u32) -> Option<u32> {
        let fat_offset = cluster * 4; // FAT32 entries are 4 bytes
        let fat_sector = self.fat_start + (fat_offset / 512);
        let fat_index = (fat_offset % 512) as usize;

        unsafe {
            let read = ahci::read_sectors(self.port, fat_sector as u64, 1, self.fat_cache.as_mut_ptr());
            if read != 1 { return None; }
        }
        let entry = u32::from_le_bytes([
            self.fat_cache[fat_index],
            self.fat_cache[fat_index + 1],
            self.fat_cache[fat_index + 2],
            self.fat_cache[fat_index + 3],
        ]) & 0x0FFF_FFFF; // FAT32: top 4 bits reserved

        match entry {
            0x0000_0000 => None,           // free
            0x0FFF_FFF7 => None,           // bad cluster
            n if n >= 0x0FFF_FFF8 => None, // end of chain
            n => Some(n),                  // next cluster
        }
    }

    /// Find a file by name in a directory cluster, reading the full chain.
    /// Returns (first_cluster, file_size) or None.
    pub fn find_file(&mut self, name: &[u8]) -> Option<(u32, u32)> {
        let mut cluster = self.root_cluster;
        let sectors_per_cluster = self.sectors_per_cluster as u64;

        loop {
            let lba = self.cluster_to_lba(cluster);
            for s in 0..sectors_per_cluster {
                unsafe {
                    let read = ahci::read_sectors(self.port, lba + s, 1, self.buf.as_mut_ptr());
                    if read != 1 { continue; }
                }
                let entries = self.buf.as_ptr() as *const DirEntry;
                for i in 0..(512 / 32) {
                    let de = unsafe { &*entries.add(i) };
                    if de.name[0] == 0 { return None; } // end of dir
                    if de.name[0] == 0xE5 { continue; } // deleted

                    // Compare name (case-insensitive, 8.3)
                    if name_match(&de.name, name) {
                        let first_cluster = (de.first_cluster_hi as u32) << 16 | (de.first_cluster_lo as u32);
                        return Some((first_cluster, de.file_size));
                    }
                }
            }
            // Follow FAT chain to next directory cluster
            cluster = match self.read_fat_entry(cluster) {
                Some(c) => c,
                None => return None,
            };
        }
    }

    /// Read an entire file into a buffer.
    /// Returns number of bytes read.
    pub fn read_file(&mut self, first_cluster: u32, file_size: u32, dst: &mut [u8]) -> usize {
        let mut cluster = first_cluster;
        let mut offset = 0usize;
        let sectors_per_cluster = self.sectors_per_cluster as u64;

        while offset < file_size as usize && offset < dst.len() {
            let lba = self.cluster_to_lba(cluster);
            for s in 0..sectors_per_cluster {
                if offset >= file_size as usize || offset >= dst.len() { break; }
                let sector_start = offset;
                let sector_end = (sector_start + 512).min(file_size as usize).min(dst.len());
                let count = sector_end - sector_start;
                if count > 0 {
                    unsafe {
                        let read = ahci::read_sectors(
                            self.port, lba + s, 1,
                            self.buf.as_mut_ptr(),
                        );
                        if read == 1 {
                            dst[sector_start..sector_start + count]
                                .copy_from_slice(&self.buf[..count]);
                        }
                    }
                }
                offset += count;
            }
            cluster = match self.read_fat_entry(cluster) {
                Some(c) => c,
                None => break,
            };
        }
        offset
    }

    /// Find a free cluster in the FAT. Returns the cluster number.
    fn find_free_cluster(&mut self) -> Option<u32> {
        let fat_size_sectors = unsafe {
            let bpb = self.read_bpb();
            bpb.fat_size
        };
        for sector in 0..fat_size_sectors {
            unsafe {
                let read = ahci::read_sectors(self.port, (self.fat_start + sector) as u64, 1, self.fat_cache.as_mut_ptr());
                if read != 1 { continue; }
            }
            for i in 0..(512 / 4) {
                let entry = u32::from_le_bytes([
                    self.fat_cache[i * 4],
                    self.fat_cache[i * 4 + 1],
                    self.fat_cache[i * 4 + 2],
                    self.fat_cache[i * 4 + 3],
                ]) & 0x0FFF_FFFF;
                if entry == 0 {
                    let cluster = sector * (512 / 4) + i as u32;
                    if cluster >= 2 { return Some(cluster); }
                }
            }
        }
        None
    }

    /// Mark a cluster as end-of-chain in the FAT.
    fn mark_cluster_eoc(&mut self, cluster: u32) -> bool {
        let fat_offset = cluster * 4;
        let fat_sector = self.fat_start + (fat_offset / 512);
        let fat_index = (fat_offset % 512) as usize;

        unsafe {
            let read = ahci::read_sectors(self.port, fat_sector as u64, 1, self.fat_cache.as_mut_ptr());
            if read != 1 { return false; }
        }
        let eoc = 0x0FFF_FFFFu32;
        self.fat_cache[fat_index] = (eoc & 0xFF) as u8;
        self.fat_cache[fat_index + 1] = ((eoc >> 8) & 0xFF) as u8;
        self.fat_cache[fat_index + 2] = ((eoc >> 16) & 0xFF) as u8;
        self.fat_cache[fat_index + 3] = ((eoc >> 24) & 0xFF) as u8;

        unsafe {
            let written = ahci::write_sectors(self.port, fat_sector as u64, 1, self.fat_cache.as_ptr());
            written == 1
        }
    }

    /// Find a free directory entry in the root directory.
    /// Returns (sector_lba, byte_offset_in_sector).
    fn find_free_dir_entry(&mut self) -> Option<(u64, usize)> {
        let mut cluster = self.root_cluster;
        let sectors_per_cluster = self.sectors_per_cluster as u64;

        loop {
            let lba = self.cluster_to_lba(cluster);
            for s in 0..sectors_per_cluster {
                unsafe {
                    let read = ahci::read_sectors(self.port, lba + s, 1, self.buf.as_mut_ptr());
                    if read != 1 { continue; }
                }
                let entries = self.buf.as_ptr() as *const DirEntry;
                for i in 0..(512 / 32) {
                    unsafe {
                        let de = &*entries.add(i);
                        if de.name[0] == 0 || de.name[0] == 0xE5 {
                            return Some((lba + s, i * 32));
                        }
                    }
                }
            }
            cluster = match self.read_fat_entry(cluster) {
                Some(c) => c,
                None => return None,
            };
        }
    }

    /// Read the BPB from sector 0.
    unsafe fn read_bpb(&self) -> FatBpb {
        let mut buf = [0u8; 512];
        ahci::read_sectors(self.port, 0, 1, buf.as_mut_ptr());
        core::ptr::read_unaligned(buf.as_ptr() as *const FatBpb)
    }

    /// Find a directory by name in the root directory.
    /// Returns the first cluster of the directory, or None.
    pub fn find_dir(&mut self, name: &[u8; 11]) -> Option<u32> {
        let mut cluster = self.root_cluster;
        let sectors_per_cluster = self.sectors_per_cluster as u64;

        loop {
            let lba = self.cluster_to_lba(cluster);
            for s in 0..sectors_per_cluster {
                unsafe {
                    let read = ahci::read_sectors(self.port, lba + s, 1, self.buf.as_mut_ptr());
                    if read != 1 { continue; }
                }
                let entries = self.buf.as_ptr() as *const DirEntry;
                for i in 0..(512 / 32) {
                    unsafe {
                        let de = &*entries.add(i);
                        if de.name[0] == 0 { return None; }
                        if de.name[0] == 0xE5 { continue; }
                        if de.attr & 0x10 == 0 { continue; } // not a directory
                        if &de.name == name {
                            let first_cluster = (de.first_cluster_hi as u32) << 16 | (de.first_cluster_lo as u32);
                            return Some(first_cluster);
                        }
                    }
                }
            }
            cluster = match self.read_fat_entry(cluster) {
                Some(c) => c,
                None => return None,
            };
        }
    }

    /// Find a free directory entry in a specific directory (by first cluster).
    /// Returns (sector_lba, byte_offset_in_sector).
    fn find_free_dir_entry_in(&mut self, dir_cluster: u32) -> Option<(u64, usize)> {
        let mut cluster = dir_cluster;
        let sectors_per_cluster = self.sectors_per_cluster as u64;

        loop {
            let lba = self.cluster_to_lba(cluster);
            for s in 0..sectors_per_cluster {
                unsafe {
                    let read = ahci::read_sectors(self.port, lba + s, 1, self.buf.as_mut_ptr());
                    if read != 1 { continue; }
                }
                let entries = self.buf.as_ptr() as *const DirEntry;
                for i in 0..(512 / 32) {
                    unsafe {
                        let de = &*entries.add(i);
                        if de.name[0] == 0 || de.name[0] == 0xE5 {
                            return Some((lba + s, i * 32));
                        }
                    }
                }
            }
            cluster = match self.read_fat_entry(cluster) {
                Some(c) => c,
                None => return None,
            };
        }
    }

    /// Create a file in a specific directory (given by first cluster) with the given 8.3 name and data.
    pub fn create_file_in_dir(&mut self, dir_cluster: u32, name_8_3: &[u8; 11], data: &[u8]) -> bool {
        // Find a free cluster
        let cluster = match self.find_free_cluster() {
            Some(c) => c,
            None => return false,
        };

        // Write data to the cluster
        let lba = self.cluster_to_lba(cluster);
        let sectors_per_cluster = self.sectors_per_cluster as u64;
        let total_sectors = (data.len() as u64 + 511) / 512;
        let write_sectors = total_sectors.min(sectors_per_cluster);

        let mut temp_buf = [0u8; 512];
        for s in 0..write_sectors {
            let offset = (s * 512) as usize;
            let count = core::cmp::min(512, data.len().saturating_sub(offset));
            temp_buf[..count].copy_from_slice(&data[offset..offset + count]);
            if count < 512 {
                for i in count..512 { temp_buf[i] = 0; }
            }
            unsafe {
                let written = ahci::write_sectors(self.port, lba + s, 1, temp_buf.as_ptr());
                if written != 1 { return false; }
            }
        }
        for s in write_sectors..sectors_per_cluster {
            for i in 0..512 { temp_buf[i] = 0; }
            unsafe {
                ahci::write_sectors(self.port, lba + s, 1, temp_buf.as_ptr());
            }
        }

        // Mark cluster as end-of-chain in the FAT
        if !self.mark_cluster_eoc(cluster) { return false; }

        // Find a free directory entry in the target directory
        let (dir_lba, dir_offset) = match self.find_free_dir_entry_in(dir_cluster) {
            Some(v) => v,
            None => return false,
        };

        // Read the directory sector
        unsafe {
            let read = ahci::read_sectors(self.port, dir_lba, 1, self.buf.as_mut_ptr());
            if read != 1 { return false; }
        }

        // Write the directory entry
        let de = unsafe { &mut *(self.buf.as_mut_ptr().add(dir_offset) as *mut DirEntry) };
        de.name = *name_8_3;
        de.attr = 0x20; // Archive
        de._nt_reserved = 0;
        de._create_time_tenth = 0;
        de.create_time = 0;
        de.create_date = 0;
        de.last_access = 0;
        de.first_cluster_hi = (cluster >> 16) as u16;
        de.write_time = 0;
        de.write_date = 0;
        de.first_cluster_lo = (cluster & 0xFFFF) as u16;
        de.file_size = data.len() as u32;

        // Write the directory sector back
        unsafe {
            let written = ahci::write_sectors(self.port, dir_lba, 1, self.buf.as_ptr());
            written == 1
        }
    }

    /// Create a file in the root directory with the given 8.3 name and data.
    pub fn create_file(&mut self, name_8_3: &[u8; 11], data: &[u8]) -> bool {
        // Find a free cluster
        let cluster = match self.find_free_cluster() {
            Some(c) => c,
            None => return false,
        };

        // Write data to the cluster
        let lba = self.cluster_to_lba(cluster);
        let sectors_per_cluster = self.sectors_per_cluster as u64;
        let total_sectors = (data.len() as u64 + 511) / 512;
        let write_sectors = total_sectors.min(sectors_per_cluster);

        let mut temp_buf = [0u8; 512];
        for s in 0..write_sectors {
            let offset = (s * 512) as usize;
            let count = core::cmp::min(512, data.len().saturating_sub(offset));
            temp_buf[..count].copy_from_slice(&data[offset..offset + count]);
            if count < 512 {
                for i in count..512 { temp_buf[i] = 0; }
            }
            unsafe {
                let written = ahci::write_sectors(self.port, lba + s, 1, temp_buf.as_ptr());
                if written != 1 { return false; }
            }
        }
        // Zero remaining sectors in the cluster
        for s in write_sectors..sectors_per_cluster {
            for i in 0..512 { temp_buf[i] = 0; }
            unsafe {
                ahci::write_sectors(self.port, lba + s, 1, temp_buf.as_ptr());
            }
        }

        // Mark cluster as end-of-chain in the FAT
        if !self.mark_cluster_eoc(cluster) { return false; }

        // Find a free directory entry
        let (dir_lba, dir_offset) = match self.find_free_dir_entry() {
            Some(v) => v,
            None => return false,
        };

        // Read the directory sector
        unsafe {
            let read = ahci::read_sectors(self.port, dir_lba, 1, self.buf.as_mut_ptr());
            if read != 1 { return false; }
        }

        // Write the directory entry
        let de = unsafe { &mut *(self.buf.as_mut_ptr().add(dir_offset) as *mut DirEntry) };
        de.name = *name_8_3;
        de.attr = 0x20; // Archive
        de._nt_reserved = 0;
        de._create_time_tenth = 0;
        de.create_time = 0;
        de.create_date = 0;
        de.last_access = 0;
        de.first_cluster_hi = (cluster >> 16) as u16;
        de.write_time = 0;
        de.write_date = 0;
        de.first_cluster_lo = (cluster & 0xFFFF) as u16;
        de.file_size = data.len() as u32;

        // Write the directory sector back
        unsafe {
            let written = ahci::write_sectors(self.port, dir_lba, 1, self.buf.as_ptr());
            written == 1
        }
    }
}

/// Case-insensitive 8.3 name match.
fn name_match(entry: &[u8; 11], query: &[u8]) -> bool {
    // Simple match: compare padded name
    let q = query;
    if q.len() > 11 { return false; }
    for i in 0..q.len() {
        let e = if i < 11 { entry[i].to_ascii_uppercase() } else { 0x20 };
        let qb = q[i].to_ascii_uppercase();
        if e != qb && !(qb == b' ' && e == 0x20) { return false; }
    }
    // Remaining entry chars must be spaces
    for i in q.len()..11 {
        if entry[i] != 0x20 && entry[i] != 0 { return false; }
    }
    true
}
