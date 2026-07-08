//! FAT32 and exFAT filesystem reader/writer — minimal implementation.
//!
//! Supports both FAT32 (S: FASTOS-EFI) and exFAT (T: FastOS-Data, X: Commit-Real).
//! Reads BPB, locates root directory, finds files by 8.3 name,
//! and reads clusters via the FAT chain. Uses `bmo_ahci::read_sectors/write_sectors`.

#![no_std]

/// Filesystem type detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsType {
    Fat32,
    ExFat,
}

/// exFAT BIOS Parameter Block at sector 0, offset 0.
/// exFAT has a different layout than FAT32 — see exFAT spec section 3.1.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ExFatBpb {
    pub jump: [u8; 3],
    pub fs_name: [u8; 8],       // "EXFAT   "
    pub must_be_zero: [u8; 53],
    pub partition_offset: u64,
    pub volume_length: u64,
    pub fat_offset: u32,
    pub fat_length: u32,
    pub cluster_heap_offset: u32,
    pub cluster_count: u32,
    pub first_cluster_of_root_directory: u32,
    pub volume_serial_number: u32,
    pub fs_revision: u16,
    pub volume_flags: u16,
    pub bytes_per_sector_shift: u8,
    pub sectors_per_cluster_shift: u8,
    pub number_of_fats: u8,
    pub drive_select: u8,
    pub percent_in_use: u8,
    pub reserved: [u8; 7],
    pub boot_code: [u8; 390],
    pub boot_signature: u16,
}

/// FAT32 BIOS Parameter Block at sector 0, offset 11.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct FatBpb {
    pub jmp: [u8; 3],
    pub oem: [u8; 8],
    pub bytes_per_sector: u16,
    pub sectors_per_cluster: u8,
    pub reserved_sectors: u16,
    pub num_fats: u8,
    _root_entries: u16,
    _total_sectors16: u16,
    pub media: u8,
    _fat_size16: u16,
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

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct DirEntry {
    pub name: [u8; 11],
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

/// exFAT File Directory Entry (type 0x85)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ExFatFileEntry {
    pub entry_type: u8,      // 0x85
    pub secondary_count: u8,
    pub set_checksum: u16,
    pub file_attributes: u16,
    _reserved1: u16,
    pub create_timestamp: u32,
    pub last_modified_timestamp: u32,
    pub last_accessed_timestamp: u32,
    _create_millis: u8,
    _last_modified_millis: u8,
    _create_utc_offset: u8,
    _last_modified_utc_offset: u8,
    _last_accessed_utc_offset: u8,
    _reserved2: [u8; 7],
}

/// exFAT Stream Extension Entry (type 0xC0) — follows File Entry
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ExFatStreamEntry {
    pub entry_type: u8,      // 0xC0
    pub general_secondary_flags: u8,
    _reserved1: u8,
    _reserved2: u8,
    pub name_length: u8,
    pub name_hash: u16,
    _reserved3: u16,
    pub valid_data_length: u64,
    _reserved4: u32,
    pub first_cluster: u32,
    pub data_length: u64,
}

/// exFAT Filename Entry (type 0xC1) — follows Stream Entry
/// Contains up to 15 UTF-16 characters of the filename
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ExFatNameEntry {
    pub entry_type: u8,      // 0xC1
    pub general_secondary_flags: u8,
    pub name_string: [u16; 15],  // UTF-16LE filename (up to 15 chars)
}

pub struct FatVolume {
    pub port: u8,
    pub fs_type: FsType,
    bytes_per_sector: u16,
    sectors_per_cluster: u8,
    num_fats: u8,
    fat_start: u32,
    fat_size_sectors: u32,
    data_start: u32,
    root_cluster: u32,
    buf: [u8; 512],
    fat_cache: [u8; 512],
}

/// Detect FAT32 vs exFAT and mount accordingly.
pub fn mount(port: u8) -> Option<FatVolume> {
    let mut buf = [0u8; 512];
    unsafe {
        if bmo_ahci::read_sectors(port, 0, 1, buf.as_mut_ptr()) != 1 { return None; }
    }

    // Check for exFAT signature ("EXFAT   ") at offset 3
    let fs_name = &buf[3..11];
    if fs_name == b"EXFAT   " {
        return mount_exfat(port, &buf);
    }

    // Otherwise try FAT32
    let bpb = unsafe { &*(buf.as_ptr() as *const FatBpb) };
    if bpb.bytes_per_sector != 512 { return None; }
    if bpb.boot_sig != 0x29 && bpb.boot_sig != 0x28 { return None; }
    let fat_start = bpb.reserved_sectors as u32;
    let fat_size_sectors = bpb.fat_size;
    let num_fats = bpb.num_fats;
    let data_start = fat_start + (num_fats as u32) * fat_size_sectors;
    Some(FatVolume { port, fs_type: FsType::Fat32, bytes_per_sector: bpb.bytes_per_sector, sectors_per_cluster: bpb.sectors_per_cluster,
        num_fats, fat_start, fat_size_sectors, data_start, root_cluster: bpb.root_cluster, buf: [0; 512], fat_cache: [0; 512] })
}

fn mount_exfat(port: u8, buf: &[u8; 512]) -> Option<FatVolume> {
    let epb = unsafe { &*(buf.as_ptr() as *const ExFatBpb) };
    if epb.boot_signature != 0xAA55 { return None; }
    let bps_shift = epb.bytes_per_sector_shift;
    let bytes_per_sector: u16 = 1u16 << bps_shift;
    let spc_shift = epb.sectors_per_cluster_shift;
    let sectors_per_cluster: u8 = 1u8 << spc_shift;
    let fat_start = epb.fat_offset;
    let fat_size_sectors = epb.fat_length;
    let data_start = epb.cluster_heap_offset;
    let root_cluster = epb.first_cluster_of_root_directory;
    let num_fats = epb.number_of_fats;

    log::info!("[bmo_fat32] exFAT detected: bps={} spc={} data_start={} root_cluster={} fats={}",
        bytes_per_sector, sectors_per_cluster, data_start, root_cluster, num_fats);

    Some(FatVolume { port, fs_type: FsType::ExFat, bytes_per_sector, sectors_per_cluster,
        num_fats, fat_start, fat_size_sectors, data_start, root_cluster, buf: [0; 512], fat_cache: [0; 512] })
}

impl FatVolume {
    fn cluster_to_lba(&self, cluster: u32) -> u64 {
        self.data_start as u64 + (cluster as u64 - 2) * self.sectors_per_cluster as u64
    }

    fn read_fat_entry(&mut self, cluster: u32) -> Option<u32> {
        let fat_offset = cluster * 4;
        let fat_sector = self.fat_start + (fat_offset / 512);
        let fat_index = (fat_offset % 512) as usize;
        unsafe {
            if bmo_ahci::read_sectors(self.port, fat_sector as u64, 1, self.fat_cache.as_mut_ptr()) != 1 { return None; }
        }
        let entry = u32::from_le_bytes([self.fat_cache[fat_index], self.fat_cache[fat_index+1],
            self.fat_cache[fat_index+2], self.fat_cache[fat_index+3]]) & 0x0FFF_FFFF;
        match entry {
            0 => None,
            n if n >= 0x0FFF_FFF7 => None,
            n => Some(n),
        }
    }

    pub fn find_file(&mut self, name: &[u8]) -> Option<(u32, u32)> {
        match self.fs_type {
            FsType::Fat32 => self.find_file_fat32(name),
            FsType::ExFat => self.find_file_exfat(name),
        }
    }

    fn find_file_fat32(&mut self, name: &[u8]) -> Option<(u32, u32)> {
        let mut cluster = self.root_cluster;
        let spc = self.sectors_per_cluster as u64;
        loop {
            let lba = self.cluster_to_lba(cluster);
            for s in 0..spc {
                unsafe {
                    if bmo_ahci::read_sectors(self.port, lba + s, 1, self.buf.as_mut_ptr()) != 1 { continue; }
                }
                let entries = self.buf.as_ptr() as *const DirEntry;
                for i in 0..(512/32) {
                    let de = unsafe { &*entries.add(i) };
                    if de.name[0] == 0 { return None; }
                    if de.name[0] == 0xE5 { continue; }
                    if name_match(&de.name, name) {
                        let fc = (de.first_cluster_hi as u32) << 16 | de.first_cluster_lo as u32;
                        return Some((fc, de.file_size));
                    }
                }
            }
            cluster = match self.read_fat_entry(cluster) { Some(c) => c, None => return None };
        }
    }

    fn find_file_exfat(&mut self, name: &[u8]) -> Option<(u32, u32)> {
        let mut cluster = self.root_cluster;
        let spc = self.sectors_per_cluster as u64;
        let mut entry_buf = [0u8; 32];
        loop {
            let lba = self.cluster_to_lba(cluster);
            for s in 0..spc {
                unsafe {
                    if bmo_ahci::read_sectors(self.port, lba + s, 1, self.buf.as_mut_ptr()) != 1 { continue; }
                }
                // Scan 16 entries per 512-byte sector (each entry = 32 bytes)
                for i in 0..16 {
                    let entry_offset = i * 32;
                    let entry_type = self.buf[entry_offset];
                    if entry_type == 0x00 { return None; } // end of directory
                    if entry_type == 0x05 { continue; }   // deleted
                    if entry_type == 0x85 {
                        // File Entry — next entries are Stream + Filename
                        let file_entry = unsafe {
                            &*(self.buf[entry_offset..].as_ptr() as *const ExFatFileEntry)
                        };
                        let secondary_count = file_entry.secondary_count;
                        // Walk secondary entries in subsequent slots
                        for sec in 1..=secondary_count {
                            let sec_offset = entry_offset + (sec as usize) * 32;
                            if sec_offset + 32 > 512 { break; }
                            let sec_type = self.buf[sec_offset];
                            if sec_type == 0xC0 {
                                // Stream Extension — has first_cluster and name_length
                                let stream = unsafe {
                                    &*(self.buf[sec_offset..].as_ptr() as *const ExFatStreamEntry)
                                };
                                let first_cluster = stream.first_cluster;
                                let name_len = stream.name_length as usize;
                                let data_len = stream.valid_data_length as u32;
                                // Next entry should be Filename (0xC1)
                                if sec + 1 <= secondary_count {
                                    let name_offset = entry_offset + ((sec + 1) as usize) * 32;
                                    if name_offset + 32 <= 512 && self.buf[name_offset] == 0xC1 {
                                        let name_entry = unsafe {
                                            &*(self.buf[name_offset..].as_ptr() as *const ExFatNameEntry)
                                        };
                                        // Convert UTF-16LE name to 8.3 for comparison
                                        let mut fat_name = [0u8; 11];
                                        let mut pos = 0;
                                        for ci in 0..name_len.min(15) {
                                            let ch = name_entry.name_string[ci] as u8;
                                            if ch == b'.' {
                                                // Handle extension
                                                while pos < 8 { fat_name[pos] = b' '; pos += 1; }
                                                continue;
                                            }
                                            if pos < 11 {
                                                fat_name[pos] = ch.to_ascii_uppercase();
                                                pos += 1;
                                            }
                                        }
                                        while pos < 11 { fat_name[pos] = b' '; pos += 1; }
                                        if name_match(&fat_name, name) {
                                            return Some((first_cluster, data_len));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            cluster = match self.read_fat_entry(cluster) { Some(c) => c, None => return None };
        }
    }

    pub fn read_file(&mut self, first_cluster: u32, file_size: u32, dst: &mut [u8]) -> usize {
        let mut cluster = first_cluster;
        let mut offset = 0;
        let spc = self.sectors_per_cluster as u64;
        while offset < file_size as usize && offset < dst.len() {
            let lba = self.cluster_to_lba(cluster);
            for s in 0..spc {
                if offset >= file_size as usize || offset >= dst.len() { break; }
                let start = offset;
                let end = (start + 512).min(file_size as usize).min(dst.len());
                let count = end - start;
                if count > 0 {
                    unsafe {
                        if bmo_ahci::read_sectors(self.port, lba + s, 1, self.buf.as_mut_ptr()) == 1 {
                            dst[start..start+count].copy_from_slice(&self.buf[..count]);
                        }
                    }
                }
                offset += count;
            }
            cluster = match self.read_fat_entry(cluster) { Some(c) => c, None => break };
        }
        offset
    }

    /// Find a free cluster in the FAT.
    fn find_free_cluster(&mut self) -> Option<u32> {
        for sector in 0..self.fat_size_sectors {
            unsafe {
                if bmo_ahci::read_sectors(self.port, (self.fat_start + sector) as u64, 1, self.fat_cache.as_mut_ptr()) != 1 { continue; }
            }
            for i in 0..(512/4) {
                let entry = u32::from_le_bytes([
                    self.fat_cache[i*4], self.fat_cache[i*4+1],
                    self.fat_cache[i*4+2], self.fat_cache[i*4+3],
                ]) & 0x0FFF_FFFF;
                if entry == 0 {
                    let cluster = sector * (512/4) as u32 + i as u32;
                    if cluster >= 2 { return Some(cluster); }
                }
            }
        }
        None
    }

    /// Mark a cluster as end-of-chain in ALL FAT copies.
    fn mark_cluster_eoc(&mut self, cluster: u32) -> bool {
        let fat_offset = cluster * 4;
        let fat_index_in_sector = (fat_offset % 512) as usize;
        let sectors_from_fat_start = fat_offset / 512;
        let eoc: u32 = 0x0FFF_FFFF;

        let mut ok = true;
        for copy in 0..self.num_fats as u32 {
            let fat_sector = self.fat_start + copy * self.fat_size_sectors + sectors_from_fat_start;
            unsafe {
                if bmo_ahci::read_sectors(self.port, fat_sector as u64, 1, self.fat_cache.as_mut_ptr()) != 1 { ok = false; continue; }
            }
            self.fat_cache[fat_index_in_sector] = eoc as u8;
            self.fat_cache[fat_index_in_sector+1] = (eoc >> 8) as u8;
            self.fat_cache[fat_index_in_sector+2] = (eoc >> 16) as u8;
            self.fat_cache[fat_index_in_sector+3] = (eoc >> 24) as u8;
            unsafe {
                if bmo_ahci::write_sectors(self.port, fat_sector as u64, 1, self.fat_cache.as_ptr()) != 1 { ok = false; }
            }
        }
        ok
    }

    /// Find a free directory entry in a directory (by first cluster).
    /// Returns (sector_lba, byte_offset_in_sector).
    fn find_free_dir_entry_in(&mut self, dir_cluster: u32) -> Option<(u64, usize)> {
        match self.fs_type {
            FsType::Fat32 => self.find_free_dir_entry_fat32(dir_cluster),
            FsType::ExFat => self.find_free_dir_entry_exfat(dir_cluster),
        }
    }

    fn find_free_dir_entry_fat32(&mut self, dir_cluster: u32) -> Option<(u64, usize)> {
        let mut cluster = dir_cluster;
        let spc = self.sectors_per_cluster as u64;
        loop {
            let lba = self.cluster_to_lba(cluster);
            for s in 0..spc {
                unsafe {
                    if bmo_ahci::read_sectors(self.port, lba + s, 1, self.buf.as_mut_ptr()) != 1 { continue; }
                }
                let entries = self.buf.as_ptr() as *const DirEntry;
                for i in 0..(512/32) {
                    unsafe {
                        let de = &*entries.add(i);
                        if de.name[0] == 0 || de.name[0] == 0xE5 {
                            return Some((lba + s, i * 32));
                        }
                    }
                }
            }
            cluster = match self.read_fat_entry(cluster) { Some(c) => c, None => return None };
        }
    }

    /// exFAT: find 3 consecutive free entry slots for File + Stream + Filename
    fn find_free_dir_entry_exfat(&mut self, dir_cluster: u32) -> Option<(u64, usize)> {
        let mut cluster = dir_cluster;
        let spc = self.sectors_per_cluster as u64;
        loop {
            let lba = self.cluster_to_lba(cluster);
            for s in 0..spc {
                unsafe {
                    if bmo_ahci::read_sectors(self.port, lba + s, 1, self.buf.as_mut_ptr()) != 1 { continue; }
                }
                // Need 3 consecutive free slots (File=0x85, Stream=0xC0, Name=0xC1)
                for i in 0..(512/32 - 2) {
                    let offset = i * 32;
                    let t0 = self.buf[offset];
                    let t1 = self.buf[offset + 32];
                    let t2 = self.buf[offset + 64];
                    if (t0 == 0x00 || t0 == 0x05) && (t1 == 0x00 || t1 == 0x05) && (t2 == 0x00 || t2 == 0x05) {
                        return Some((lba + s, offset));
                    }
                }
            }
            cluster = match self.read_fat_entry(cluster) { Some(c) => c, None => return None };
        }
    }

    /// Find a subdirectory by name in the root directory.
    /// Returns the first cluster of the subdirectory.
    pub fn find_subdir(&mut self, name: &[u8]) -> Option<u32> {
        self.find_subdir_in(name, self.root_cluster)
    }

    /// Find a subdirectory by name in a specific directory (by first cluster).
    pub fn find_subdir_in(&mut self, name: &[u8], dir_cluster: u32) -> Option<u32> {
        match self.fs_type {
            FsType::Fat32 => self.find_subdir_fat32(name, dir_cluster),
            FsType::ExFat => self.find_subdir_exfat(name, dir_cluster),
        }
    }

    fn find_subdir_fat32(&mut self, name: &[u8], dir_cluster: u32) -> Option<u32> {
        let mut cluster = dir_cluster;
        let spc = self.sectors_per_cluster as u64;
        loop {
            let lba = self.cluster_to_lba(cluster);
            for s in 0..spc {
                unsafe {
                    if bmo_ahci::read_sectors(self.port, lba + s, 1, self.buf.as_mut_ptr()) != 1 { continue; }
                }
                let entries = self.buf.as_ptr() as *const DirEntry;
                for i in 0..(512/32) {
                    unsafe {
                        let de = &*entries.add(i);
                        if de.name[0] == 0 { return None; }
                        if de.name[0] == 0xE5 { continue; }
                        if de.attr & 0x10 == 0 { continue; } // not a directory
                        if name_match(&de.name, name) {
                            let fc = (de.first_cluster_hi as u32) << 16 | de.first_cluster_lo as u32;
                            return Some(fc);
                        }
                    }
                }
            }
            cluster = match self.read_fat_entry(cluster) { Some(c) => c, None => return None };
        }
    }

    fn find_subdir_exfat(&mut self, name: &[u8], dir_cluster: u32) -> Option<u32> {
        let mut cluster = dir_cluster;
        let spc = self.sectors_per_cluster as u64;
        loop {
            let lba = self.cluster_to_lba(cluster);
            for s in 0..spc {
                unsafe {
                    if bmo_ahci::read_sectors(self.port, lba + s, 1, self.buf.as_mut_ptr()) != 1 { continue; }
                }
                for i in 0..16 {
                    let entry_offset = i * 32;
                    let entry_type = self.buf[entry_offset];
                    if entry_type == 0x00 { return None; }
                    if entry_type == 0x05 { continue; }
                    if entry_type == 0x85 {
                        let file_entry = unsafe {
                            &*(self.buf[entry_offset..].as_ptr() as *const ExFatFileEntry)
                        };
                        let secondary_count = file_entry.secondary_count;
                        let is_dir = file_entry.file_attributes & 0x10 != 0;
                        for sec in 1..=secondary_count {
                            let sec_offset = entry_offset + (sec as usize) * 32;
                            if sec_offset + 32 > 512 { break; }
                            let sec_type = self.buf[sec_offset];
                            if sec_type == 0xC0 {
                                let stream = unsafe {
                                    &*(self.buf[sec_offset..].as_ptr() as *const ExFatStreamEntry)
                                };
                                let first_cluster = stream.first_cluster;
                                let name_len = stream.name_length as usize;
                                if sec + 1 <= secondary_count {
                                    let name_offset = entry_offset + ((sec + 1) as usize) * 32;
                                    if name_offset + 32 <= 512 && self.buf[name_offset] == 0xC1 {
                                        let name_entry = unsafe {
                                            &*(self.buf[name_offset..].as_ptr() as *const ExFatNameEntry)
                                        };
                                        let mut fat_name = [0u8; 11];
                                        let mut pos = 0;
                                        for ci in 0..name_len.min(15) {
                                            let ch = name_entry.name_string[ci] as u8;
                                            if ch == b'.' {
                                                while pos < 8 { fat_name[pos] = b' '; pos += 1; }
                                                continue;
                                            }
                                            if pos < 11 {
                                                fat_name[pos] = ch.to_ascii_uppercase();
                                                pos += 1;
                                            }
                                        }
                                        while pos < 11 { fat_name[pos] = b' '; pos += 1; }
                                        if is_dir && name_match(&fat_name, name) {
                                            return Some(first_cluster);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            cluster = match self.read_fat_entry(cluster) { Some(c) => c, None => return None };
        }
    }

    /// Get the root directory's first cluster.
    pub fn root_cluster(&self) -> u32 { self.root_cluster }

    /// Create a file in a specific directory (by first cluster).
    /// `name_8_3` must be an 11-byte space-padded 8.3 name.
    pub fn create_file_in_dir(&mut self, dir_cluster: u32, name_8_3: &[u8; 11], data: &[u8]) -> bool {
        match self.fs_type {
            FsType::Fat32 => self.create_file_fat32(dir_cluster, name_8_3, data),
            FsType::ExFat => self.create_file_exfat(dir_cluster, name_8_3, data),
        }
    }

    fn create_file_fat32(&mut self, dir_cluster: u32, name_8_3: &[u8; 11], data: &[u8]) -> bool {
        // Find a free cluster
        let cluster = match self.find_free_cluster() {
            Some(c) => c, None => return false,
        };

        // Write data to the cluster
        let lba = self.cluster_to_lba(cluster);
        let spc = self.sectors_per_cluster as u64;
        let total_sectors = (data.len() as u64 + 511) / 512;
        let write_n = total_sectors.min(spc);

        let mut temp = [0u8; 512];
        for s in 0..write_n {
            let off = (s * 512) as usize;
            let count = core::cmp::min(512, data.len().saturating_sub(off));
            temp[..count].copy_from_slice(&data[off..off + count]);
            unsafe {
                if bmo_ahci::write_sectors(self.port, lba + s, 1, temp.as_ptr()) != 1 { return false; }
            }
        }
        // Zero remaining sectors in the cluster
        for s in write_n..spc {
            unsafe {
                bmo_ahci::write_sectors(self.port, lba + s, 1, temp.as_ptr());
            }
        }

        // Mark cluster as end-of-chain
        if !self.mark_cluster_eoc(cluster) { return false; }

        // Find a free directory entry
        let (dir_lba, dir_off) = match self.find_free_dir_entry_in(dir_cluster) {
            Some(v) => v, None => return false,
        };

        // Read directory sector
        unsafe {
            if bmo_ahci::read_sectors(self.port, dir_lba, 1, self.buf.as_mut_ptr()) != 1 { return false; }
        }

        // Write directory entry
        let de = unsafe { &mut *(self.buf.as_mut_ptr().add(dir_off) as *mut DirEntry) };
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

        unsafe {
            bmo_ahci::write_sectors(self.port, dir_lba, 1, self.buf.as_ptr()) == 1
        }
    }

    /// exFAT: create file with 3 entries: File(0x85) + Stream(0xC0) + Filename(0xC1)
    fn create_file_exfat(&mut self, dir_cluster: u32, name_8_3: &[u8; 11], data: &[u8]) -> bool {
        let cluster = match self.find_free_cluster() {
            Some(c) => c, None => return false,
        };

        // Write data to cluster
        let lba = self.cluster_to_lba(cluster);
        let spc = self.sectors_per_cluster as u64;
        let total_sectors = (data.len() as u64 + 511) / 512;
        let write_n = total_sectors.min(spc);

        let mut temp = [0u8; 512];
        for s in 0..write_n {
            let off = (s * 512) as usize;
            let count = core::cmp::min(512, data.len().saturating_sub(off));
            temp[..count].copy_from_slice(&data[off..off + count]);
            unsafe {
                if bmo_ahci::write_sectors(self.port, lba + s, 1, temp.as_ptr()) != 1 { return false; }
            }
        }
        for s in write_n..spc {
            unsafe { bmo_ahci::write_sectors(self.port, lba + s, 1, temp.as_ptr()); }
        }

        if !self.mark_cluster_eoc(cluster) { return false; }

        // Find 3 consecutive free slots
        let (dir_lba, dir_off) = match self.find_free_dir_entry_in(dir_cluster) {
            Some(v) => v, None => return false,
        };

        // Read directory sector
        unsafe {
            if bmo_ahci::read_sectors(self.port, dir_lba, 1, self.buf.as_mut_ptr()) != 1 { return false; }
        }

        // Convert 8.3 name to UTF-16LE (up to 15 chars)
        let mut utf16_name = [0u16; 15];
        let mut name_len: usize = 0;
        for &b in name_8_3.iter() {
            if b == b' ' || b == 0 { break; }
            utf16_name[name_len] = b as u16;
            name_len += 1;
        }

        let zero32 = [0u8; 32];

        // Entry 1: File Directory Entry (0x85)
        let mut file_entry = ExFatFileEntry {
            entry_type: 0x85,
            secondary_count: 2,
            set_checksum: 0,
            file_attributes: 0x20, // Archive
            _reserved1: 0,
            create_timestamp: 0,
            last_modified_timestamp: 0,
            last_accessed_timestamp: 0,
            _create_millis: 0,
            _last_modified_millis: 0,
            _create_utc_offset: 0,
            _last_modified_utc_offset: 0,
            _last_accessed_utc_offset: 0,
            _reserved2: [0; 7],
        };
        self.buf[dir_off..dir_off + 32].copy_from_slice(unsafe {
            core::slice::from_raw_parts(&file_entry as *const _ as *const u8, 32)
        });

        // Entry 2: Stream Extension Entry (0xC0)
        let mut stream_entry = ExFatStreamEntry {
            entry_type: 0xC0,
            general_secondary_flags: 0x01,
            _reserved1: 0,
            _reserved2: 0,
            name_length: name_len as u8,
            name_hash: 0,
            _reserved3: 0,
            valid_data_length: data.len() as u64,
            _reserved4: 0,
            first_cluster: cluster,
            data_length: data.len() as u64,
        };
        self.buf[dir_off + 32..dir_off + 64].copy_from_slice(unsafe {
            core::slice::from_raw_parts(&stream_entry as *const _ as *const u8, 32)
        });

        // Entry 3: Filename Entry (0xC1)
        let mut name_entry = ExFatNameEntry {
            entry_type: 0xC1,
            general_secondary_flags: 0x01,
            name_string: utf16_name,
        };
        self.buf[dir_off + 64..dir_off + 96].copy_from_slice(unsafe {
            core::slice::from_raw_parts(&name_entry as *const _ as *const u8, 32)
        });

        unsafe {
            bmo_ahci::write_sectors(self.port, dir_lba, 1, self.buf.as_ptr()) == 1
        }
    }
}

fn name_match(entry: &[u8; 11], query: &[u8]) -> bool {
    if query.len() > 11 { return false; }
    for i in 0..query.len() {
        let e = if i < 11 { entry[i].to_ascii_uppercase() } else { 0x20 };
        let qb = query[i].to_ascii_uppercase();
        if e != qb && !(qb == b' ' && e == 0x20) { return false; }
    }
    for i in query.len()..11 {
        if entry[i] != 0x20 && entry[i] != 0 { return false; }
    }
    true
}
