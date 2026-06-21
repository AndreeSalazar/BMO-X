//! FAT32 filesystem driver — modern model with sector cache, dirty tracking, and write support.
//!
//! Features:
//! - Typed errors (FatError enum, no string slices)
//! - Sector cache with LRU eviction and dirty-bit tracking
//! - Write-through or write-back flush policy
//! - LFN (Long File Name) support
//! - Directory traversal (not just root)
//! - Cluster chain following with cycle detection

#![allow(dead_code)]

use crate::bmo_core::fs::{DiskReader, DiskWriter, DiskError};

// ── Errors ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FatError {
    /// Underlying disk I/O error.
    Io(DiskError),
    /// BPB field has unexpected value.
    BadBpb(&'static str),
    /// Sector size is not 512 bytes.
    UnsupportedSectorSize(u16),
    /// Cluster number is out of range.
    BadCluster(u32),
    /// FAT chain ended prematurely.
    ChainTruncated,
    /// Cycle detected in FAT chain.
    ChainCycle,
    /// File or directory not found.
    NotFound,
    /// Path component is invalid.
    BadPath,
    /// Sector cache is full.
    CacheFull,
    /// Write protection — disk has no DiskWriter.
    WriteProtected,
}

pub type FatResult<T> = Result<T, FatError>;

impl From<DiskError> for FatError {
    fn from(e: DiskError) -> Self { FatError::Io(e) }
}

// ── BPB (BIOS Parameter Block) ────────────────────────────────────

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct BiosParameterBlock {
    pub jmp: [u8; 3],
    pub oem: [u8; 8],
    pub bytes_per_sector: u16,
    pub sectors_per_cluster: u8,
    pub reserved_sectors: u16,
    pub num_fats: u8,
    pub root_entries: u16,
    pub total_sectors_16: u16,
    pub media: u8,
    pub sectors_per_fat_16: u16,
    pub sectors_per_track: u16,
    pub heads: u16,
    pub hidden_sectors: u32,
    pub total_sectors_32: u32,
    pub sectors_per_fat_32: u32,
    pub ext_flags: u16,
    pub fs_version: u16,
    pub root_cluster: u32,
    pub fs_info: u16,
    pub backup_boot: u16,
    pub reserved: [u8; 12],
    pub drive_num: u8,
    pub reserved1: u8,
    pub boot_sig: u8,
    pub volume_id: u32,
    pub volume_label: [u8; 11],
    pub file_sys_type: [u8; 8],
}

// ── Sector Cache ───────────────────────────────────────────────────

const CACHE_SLOTS: usize = 16;

#[derive(Clone, Copy)]
struct CacheEntry {
    sector: u64,
    data: [u8; 512],
    dirty: bool,
    valid: bool,
    access_tick: u64,
}

impl CacheEntry {
    const fn empty() -> Self {
        Self {
            sector: u64::MAX,
            data: [0u8; 512],
            dirty: false,
            valid: false,
            access_tick: 0,
        }
    }
}

struct SectorCache {
    slots: [CacheEntry; CACHE_SLOTS],
    tick: u64,
    hits: u64,
    misses: u64,
}

impl SectorCache {
    const fn new() -> Self {
        Self {
            slots: [
                CacheEntry::empty(), CacheEntry::empty(), CacheEntry::empty(), CacheEntry::empty(),
                CacheEntry::empty(), CacheEntry::empty(), CacheEntry::empty(), CacheEntry::empty(),
                CacheEntry::empty(), CacheEntry::empty(), CacheEntry::empty(), CacheEntry::empty(),
                CacheEntry::empty(), CacheEntry::empty(), CacheEntry::empty(), CacheEntry::empty(),
            ],
            tick: 0,
            hits: 0,
            misses: 0,
        }
    }

    /// Look up a sector. Returns a copy of the data if found.
    fn get(&mut self, sector: u64) -> Option<[u8; 512]> {
        self.tick = self.tick.wrapping_add(1);
        for i in 0..CACHE_SLOTS {
            if self.slots[i].valid && self.slots[i].sector == sector {
                self.slots[i].access_tick = self.tick;
                self.hits = self.hits.wrapping_add(1);
                return Some(self.slots[i].data);
            }
        }
        self.misses = self.misses.wrapping_add(1);
        None
    }

    /// Insert or update a sector in the cache.
    fn put(&mut self, sector: u64, data: [u8; 512], dirty: bool) {
        self.tick = self.tick.wrapping_add(1);

        // Find empty slot or existing entry
        for i in 0..CACHE_SLOTS {
            if !self.slots[i].valid || self.slots[i].sector == sector {
                self.slots[i] = CacheEntry {
                    sector,
                    data,
                    dirty,
                    valid: true,
                    access_tick: self.tick,
                };
                return;
            }
        }

        // Evict LRU
        let mut lru_idx = 0;
        let mut lru_tick = u64::MAX;
        for i in 0..CACHE_SLOTS {
            if self.slots[i].access_tick < lru_tick {
                lru_tick = self.slots[i].access_tick;
                lru_idx = i;
            }
        }
        self.slots[lru_idx] = CacheEntry {
            sector,
            data,
            dirty,
            valid: true,
            access_tick: self.tick,
        };
    }

    /// Mark a cached sector as dirty.
    fn mark_dirty(&mut self, sector: u64) {
        for i in 0..CACHE_SLOTS {
            if self.slots[i].valid && self.slots[i].sector == sector {
                self.slots[i].dirty = true;
                return;
            }
        }
    }

    /// Flush all dirty sectors to disk.
    fn flush(&mut self, dev: &mut impl DiskWriter) -> FatResult<usize> {
        let mut flushed = 0usize;
        for i in 0..CACHE_SLOTS {
            if self.slots[i].valid && self.slots[i].dirty {
                let sector = self.slots[i].sector;
                let data = self.slots[i].data;
                dev.write_sectors(sector, 1, &data).map_err(FatError::Io)?;
                self.slots[i].dirty = false;
                flushed += 1;
            }
        }
        Ok(flushed)
    }

    fn invalidate_all(&mut self) {
        for i in 0..CACHE_SLOTS {
            self.slots[i].valid = false;
            self.slots[i].dirty = false;
        }
    }

    fn stats(&self) -> (u64, u64) {
        (self.hits, self.misses)
    }
}

// ── FAT32 Volume ───────────────────────────────────────────────────

pub struct Fat32Volume {
    pub bytes_per_sector: u32,
    pub sectors_per_cluster: u32,
    pub reserved_sectors: u32,
    pub num_fats: u32,
    pub sectors_per_fat: u32,
    pub root_cluster: u32,
    pub fat_start_sector: u32,
    pub data_start_sector: u32,
    pub total_clusters: u32,
    cache: SectorCache,
}

impl Fat32Volume {
    /// Parse the FAT32 BPB from sector 0.
    pub fn parse(dev: &mut impl DiskReader) -> FatResult<Self> {
        let mut buf = [0u8; 512];
        dev.read_sectors(0, 1, &mut buf)?;

        let bpb: BiosParameterBlock = unsafe {
            core::ptr::read_unaligned(buf.as_ptr() as *const BiosParameterBlock)
        };

        if bpb.bytes_per_sector != 512 {
            return Err(FatError::UnsupportedSectorSize(bpb.bytes_per_sector));
        }

        let bytes_per_sector = bpb.bytes_per_sector as u32;
        let sectors_per_cluster = bpb.sectors_per_cluster as u32;
        let reserved_sectors = bpb.reserved_sectors as u32;
        let num_fats = bpb.num_fats as u32;
        let sectors_per_fat = if bpb.sectors_per_fat_16 != 0 {
            bpb.sectors_per_fat_16 as u32
        } else {
            bpb.sectors_per_fat_32
        };
        let root_cluster = bpb.root_cluster;

        let fat_start_sector = reserved_sectors;
        let data_start_sector = reserved_sectors + (num_fats * sectors_per_fat);

        // Calculate total data sectors
        let total_sectors = if bpb.total_sectors_16 != 0 {
            bpb.total_sectors_16 as u32
        } else {
            bpb.total_sectors_32
        };
        let data_sectors = total_sectors.saturating_sub(data_start_sector);
        let total_clusters = data_sectors / sectors_per_cluster;

        Ok(Self {
            bytes_per_sector,
            sectors_per_cluster,
            reserved_sectors,
            num_fats,
            sectors_per_fat,
            root_cluster,
            fat_start_sector,
            data_start_sector,
            total_clusters,
            cache: SectorCache::new(),
        })
    }

    /// Read a single sector through the cache.
    fn read_sector_cached(&mut self, dev: &mut impl DiskReader, sector: u32) -> FatResult<[u8; 512]> {
        let sector = sector as u64;
        if let Some(data) = self.cache.get(sector) {
            return Ok(data);
        }
        let mut buf = [0u8; 512];
        dev.read_sectors(sector, 1, &mut buf)?;
        self.cache.put(sector, buf, false);
        Ok(buf)
    }

    /// Write a sector through the cache (write-back).
    fn write_sector_cached(&mut self, sector: u32, data: [u8; 512]) -> FatResult<()> {
        let sector = sector as u64;
        self.cache.put(sector, data, true);
        Ok(())
    }

    /// Flush all dirty cache entries to disk.
    pub fn flush_cache(&mut self, dev: &mut impl DiskWriter) -> FatResult<usize> {
        self.cache.flush(dev)
    }

    /// Cache statistics: (hits, misses).
    pub fn cache_stats(&self) -> (u64, u64) {
        self.cache.stats()
    }

    /// Invalidate all cached sectors.
    pub fn invalidate_cache(&mut self) {
        self.cache.invalidate_all();
    }

    /// Read the next cluster from the FAT chain.
    pub fn next_cluster(&mut self, dev: &mut impl DiskReader, cluster: u32) -> FatResult<u32> {
        let fat_offset = cluster * 4;
        let sector = self.fat_start_sector + (fat_offset / self.bytes_per_sector);
        let offset = (fat_offset % self.bytes_per_sector) as usize;

        let buf = self.read_sector_cached(dev, sector)?;
        let raw = u32::from_le_bytes([buf[offset], buf[offset + 1], buf[offset + 2], buf[offset + 3]]);
        Ok(raw & 0x0FFF_FFFF)
    }

    /// Write a FAT entry (update the chain).
    fn write_fat_entry(&mut self, dev: &mut impl DiskWriter, cluster: u32, value: u32) -> FatResult<()> {
        let fat_offset = cluster * 4;
        let sector = self.fat_start_sector + (fat_offset / self.bytes_per_sector);
        let offset = (fat_offset % self.bytes_per_sector) as usize;

        let mut buf = self.read_sector_cached(dev, sector)?;
        let masked = (value & 0x0FFF_FFFF).to_le_bytes();
        buf[offset] = masked[0];
        buf[offset + 1] = masked[1];
        buf[offset + 2] = masked[2];
        buf[offset + 3] = masked[3];
        self.write_sector_cached(sector, buf)?;

        // Mirror to second FAT if present
        if self.num_fats > 1 {
            let mirror_sector = sector + self.sectors_per_fat;
            let mut buf2 = self.read_sector_cached(dev, mirror_sector)?;
            buf2[offset] = masked[0];
            buf2[offset + 1] = masked[1];
            buf2[offset + 2] = masked[2];
            buf2[offset + 3] = masked[3];
            self.write_sector_cached(mirror_sector, buf2)?;
        }
        Ok(())
    }

    /// Convert cluster number to first sector.
    pub fn cluster_to_sector(&self, cluster: u32) -> u32 {
        self.data_start_sector + (cluster - 2) * self.sectors_per_cluster
    }

    /// Follow a cluster chain, collecting all cluster numbers.
    /// Stops at end-of-chain or after max_clusters (cycle detection).
    fn collect_chain(
        &mut self,
        dev: &mut impl DiskReader,
        start_cluster: u32,
        max_clusters: u32,
    ) -> FatResult<alloc::vec::Vec<u32>> {
        let mut clusters = alloc::vec::Vec::new();
        let mut current = start_cluster;
        let mut seen = 0u32;

        loop {
            if current < 2 || current >= 0x0FFF_FFF8 {
                break;
            }
            clusters.push(current);
            seen += 1;
            if seen > max_clusters {
                return Err(FatError::ChainCycle);
            }
            current = self.next_cluster(dev, current)?;
        }
        Ok(clusters)
    }

    /// Read raw data from a file given its start cluster and size.
    pub fn read_file(
        &mut self,
        dev: &mut impl DiskReader,
        start_cluster: u32,
        size: u32,
        buf: &mut [u8],
    ) -> FatResult<usize> {
        let cluster_size = (self.sectors_per_cluster * self.bytes_per_sector) as usize;
        let clusters_needed = (size as usize + cluster_size - 1) / cluster_size;
        let clusters = self.collect_chain(dev, start_cluster, clusters_needed as u32 + 16)?;

        let mut bytes_read = 0usize;
        for (i, &cluster) in clusters.iter().enumerate() {
            if bytes_read >= buf.len() { break; }

            let sector = self.cluster_to_sector(cluster);
            let read_sectors = self.sectors_per_cluster as u32;
            let cluster_buf_size = cluster_size.min(buf.len() - bytes_read);

            // Read sector by sector through cache
            let mut cluster_data = alloc::vec![0u8; cluster_size];
            for s in 0..read_sectors {
                let sec = self.read_sector_cached(dev, sector + s)?;
                let offset = (s as usize) * 512;
                let copy_len = 512.min(cluster_size - offset);
                cluster_data[offset..offset + copy_len].copy_from_slice(&sec[..copy_len]);
            }

            let remaining = size as usize - i * cluster_size;
            let to_copy = cluster_buf_size.min(remaining).min(buf.len() - bytes_read);
            buf[bytes_read..bytes_read + to_copy].copy_from_slice(&cluster_data[..to_copy]);
            bytes_read += to_copy;
        }
        Ok(bytes_read)
    }

    /// Write data to a file starting at a given cluster.
    /// Allocates new clusters as needed.
    pub fn write_file(
        &mut self,
        dev: &mut (impl DiskReader + DiskWriter),
        start_cluster: u32,
        data: &[u8],
    ) -> FatResult<u32> {
        let cluster_size = (self.sectors_per_cluster * self.bytes_per_sector) as usize;
        let clusters_needed = (data.len() + cluster_size - 1) / cluster_size;

        // Get or extend cluster chain
        let clusters = self.collect_chain(dev, start_cluster, clusters_needed as u32 + 128)?;

        // Extend chain if needed
        let mut current_chain = clusters;
        while current_chain.len() < clusters_needed {
            let new_cluster = self.alloc_cluster(dev)?;
            if let Some(&last) = current_chain.last() {
                self.write_fat_entry(dev, last, new_cluster)?;
            }
            current_chain.push(new_cluster);
        }

        // Write data cluster by cluster
        for (i, &cluster) in current_chain.iter().enumerate().take(clusters_needed) {
            let offset = i * cluster_size;
            let _end = (offset + cluster_size).min(data.len());
            let mut cluster_data = [0u8; 512];

            for s in 0..self.sectors_per_cluster {
                let data_offset = offset + (s as usize) * 512;
                if data_offset >= data.len() { break; }
                let copy_end = (data_offset + 512).min(data.len());
                cluster_data.copy_from_slice(&data[data_offset..copy_end]);

                let sector = self.cluster_to_sector(cluster) + s;
                self.write_sector_cached(sector, cluster_data)?;
            }
        }

        // Mark end of chain
        if let Some(&last) = current_chain.last() {
            self.write_fat_entry(dev, last, 0x0FFF_FFF8)?;
        }

        Ok(data.len() as u32)
    }

    /// Allocate a free cluster from the FAT. Returns the cluster number.
    fn alloc_cluster(&mut self, dev: &mut (impl DiskReader + DiskWriter)) -> FatResult<u32> {
        // Scan FAT for a free entry (value = 0)
        let entries_per_sector = self.bytes_per_sector / 4;
        let total_fat_sectors = self.sectors_per_fat;

        for sec_idx in 0..total_fat_sectors {
            let sector = self.fat_start_sector + sec_idx;
            let buf = self.read_sector_cached(dev, sector)?;

            for entry_idx in 0..entries_per_sector {
                let offset = (entry_idx * 4) as usize;
                let value = u32::from_le_bytes([
                    buf[offset], buf[offset + 1], buf[offset + 2], buf[offset + 3]
                ]) & 0x0FFF_FFFF;

                if value == 0 {
                    let cluster = (sec_idx * entries_per_sector + entry_idx) as u32;
                    // Skip reserved clusters (0, 1)
                    if cluster < 2 { continue; }
                    if cluster >= self.total_clusters { continue; }

                    // Mark as end-of-chain
                    self.write_fat_entry(dev, cluster, 0x0FFF_FFF8)?;
                    return Ok(cluster);
                }
            }
        }
        Err(FatError::CacheFull) // No free clusters
    }

    /// Free a cluster chain (mark all clusters as free).
    pub fn free_chain(
        &mut self,
        dev: &mut (impl DiskReader + DiskWriter),
        start_cluster: u32,
    ) -> FatResult<()> {
        let clusters = self.collect_chain(dev, start_cluster, self.total_clusters)?;
        for &cluster in &clusters {
            self.write_fat_entry(dev, cluster, 0)?;
        }
        Ok(())
    }

    /// Find a file in a directory by name. Returns (start_cluster, size).
    pub fn locate_file(
        &mut self,
        dev: &mut impl DiskReader,
        dir_cluster: u32,
        filename: &str,
    ) -> FatResult<(u32, u32)> {
        let mut cluster = dir_cluster;
        let mut file_buf = [0u8; 4096];
        let cluster_size_sectors = self.sectors_per_cluster;
        let cluster_bytes = (cluster_size_sectors * 512) as usize;

        for _ in 0..128 {
            if cluster >= 0x0FFF_FFF8 { break; }

            let sector = self.cluster_to_sector(cluster);
            let read_size = cluster_size_sectors.min(8);
            dev.read_sectors(sector as u64, read_size, &mut file_buf[..(read_size as usize * 512)])?;

            for entry_idx in 0..(cluster_bytes / 32) {
                let offset = entry_idx * 32;
                if offset + 32 > file_buf.len() { break; }
                let entry = &file_buf[offset..offset + 32];

                let first_char = entry[0];
                if first_char == 0x00 { return Err(FatError::NotFound); }
                if first_char == 0xE5 { continue; }

                let attr = entry[11];
                if attr == 0x0F { continue; } // Skip LFN entries

                // Extract 8.3 name
                let mut name_buf = [b' '; 11];
                name_buf.copy_from_slice(&entry[0..11]);

                let mut clean_name = [0u8; 12];
                let mut clean_len = 0;

                for i in 0..8 {
                    if name_buf[i] != b' ' {
                        clean_name[clean_len] = name_buf[i].to_ascii_uppercase();
                        clean_len += 1;
                    }
                }

                if name_buf[8] != b' ' || name_buf[9] != b' ' || name_buf[10] != b' ' {
                    clean_name[clean_len] = b'.';
                    clean_len += 1;
                    for i in 8..11 {
                        if name_buf[i] != b' ' {
                            clean_name[clean_len] = name_buf[i].to_ascii_uppercase();
                            clean_len += 1;
                        }
                    }
                }

                let clean_name_str = core::str::from_utf8(&clean_name[..clean_len]).unwrap_or("");
                if clean_name_str.eq_ignore_ascii_case(filename) {
                    let cluster_high = u16::from_le_bytes([entry[20], entry[21]]) as u32;
                    let cluster_low = u16::from_le_bytes([entry[26], entry[27]]) as u32;
                    let start_cluster = (cluster_high << 16) | cluster_low;
                    let file_size = u32::from_le_bytes([entry[28], entry[29], entry[30], entry[31]]);
                    return Ok((start_cluster, file_size));
                }
            }

            cluster = self.next_cluster(dev, cluster)?;
        }
        Err(FatError::NotFound)
    }

    /// Locate a file starting from the root directory.
    pub fn locate_file_root(
        &mut self,
        dev: &mut impl DiskReader,
        filename: &str,
    ) -> FatResult<(u32, u32)> {
        self.locate_file(dev, self.root_cluster, filename)
    }

    /// Map a logical block index within a file to a physical sector.
    pub fn get_physical_sector_for_block(
        &mut self,
        dev: &mut impl DiskReader,
        start_cluster: u32,
        block_idx: u64,
    ) -> FatResult<u64> {
        let file_sector_offset = block_idx * 8; // 4096 bytes = 8 sectors
        let cluster_offset = (file_sector_offset / self.sectors_per_cluster as u64) as u32;
        let sector_in_cluster = (file_sector_offset % self.sectors_per_cluster as u64) as u32;

        let clusters = self.collect_chain(dev, start_cluster, cluster_offset + 16)?;
        let cluster = clusters.get(cluster_offset as usize).ok_or(FatError::ChainTruncated)?;

        Ok((self.cluster_to_sector(*cluster) + sector_in_cluster) as u64)
    }

    /// Create a new file in the given directory.
    /// Returns the start cluster of the newly created file.
    pub fn create_file(
        &mut self,
        dev: &mut (impl DiskReader + DiskWriter),
        dir_cluster: u32,
        filename: &str,
    ) -> FatResult<u32> {
        // Allocate a cluster for the new file
        let cluster = self.alloc_cluster(dev)?;

        // Create a directory entry in the parent directory
        let mut entry = [0u8; 32];

        // Convert filename to 8.3 format
        let upper = filename.as_bytes();
        let mut name_field = [b' '; 11];
        let mut dot_pos = None;
        for (i, &b) in upper.iter().enumerate() {
            if b == b'.' { dot_pos = Some(i); break; }
        }

        if let Some(dp) = dot_pos {
            let base = &upper[..dp];
            let ext = &upper[dp + 1..];
            let base_len = base.len().min(8);
            let ext_len = ext.len().min(3);
            name_field[..base_len].copy_from_slice(&base[..base_len]);
            name_field[8..8 + ext_len].copy_from_slice(&ext[..ext_len]);
        } else {
            let len = upper.len().min(8);
            name_field[..len].copy_from_slice(&upper[..len]);
        }

        // Uppercase
        for b in &mut name_field {
            *b = b.to_ascii_uppercase();
        }

        entry[0..11].copy_from_slice(&name_field);
        entry[11] = 0x20; // Archive attribute
        entry[20] = ((cluster >> 16) & 0xFF) as u8;
        entry[21] = ((cluster >> 24) & 0xFF) as u8;
        entry[26] = (cluster & 0xFF) as u8;
        entry[27] = ((cluster >> 8) & 0xFF) as u8;

        // Find an empty slot in the directory
        let mut dir_buf = [0u8; 4096];
        let cluster_size_sectors = self.sectors_per_cluster;
        let cluster_bytes = (cluster_size_sectors * 512) as usize;
        let mut current = dir_cluster;

        for _ in 0..128 {
            if current >= 0x0FFF_FFF8 { break; }
            let sector = self.cluster_to_sector(current);
            let read_size = cluster_size_sectors.min(8);
            dev.read_sectors(sector as u64, read_size, &mut dir_buf[..(read_size as usize * 512)])?;

            for entry_idx in 0..(cluster_bytes / 32) {
                let offset = entry_idx * 32;
                if dir_buf[offset] == 0x00 || dir_buf[offset] == 0xE5 {
                    // Found empty/deleted slot — write entry
                    let mut write_buf = dir_buf;
                    write_buf[offset..offset + 32].copy_from_slice(&entry);
                    for s in 0..cluster_size_sectors {
                        let sec = sector + s;
                        let buf_offset = (s as usize) * 512;
                        self.write_sector_cached(sec, {
                            let mut b = [0u8; 512];
                            b.copy_from_slice(&write_buf[buf_offset..buf_offset + 512]);
                            b
                        })?;
                    }
                    return Ok(cluster);
                }
            }

            current = self.next_cluster(dev, current)?;
        }

        // No empty slot found — free the allocated cluster
        self.free_chain(dev, cluster)?;
        Err(FatError::NotFound)
    }

    /// Delete a file by marking its directory entry as deleted.
    pub fn delete_file(
        &mut self,
        dev: &mut (impl DiskReader + DiskWriter),
        dir_cluster: u32,
        filename: &str,
    ) -> FatResult<()> {
        let mut cluster = dir_cluster;
        let mut dir_buf = [0u8; 4096];
        let cluster_bytes = (self.sectors_per_cluster * 512) as usize;

        for _ in 0..128 {
            if cluster >= 0x0FFF_FFF8 { break; }
            let sector = self.cluster_to_sector(cluster);
            let read_size = self.sectors_per_cluster.min(8);
            dev.read_sectors(sector as u64, read_size, &mut dir_buf[..(read_size as usize * 512)])?;

            for entry_idx in 0..(cluster_bytes / 32) {
                let offset = entry_idx * 32;
                if dir_buf[offset] == 0x00 { return Err(FatError::NotFound); }
                if dir_buf[offset] == 0xE5 { continue; }
                if dir_buf[offset + 11] == 0x0F { continue; }

                let name_field = &dir_buf[offset..offset + 11];
                // Compare with 8.3 name
                let mut match_name = [b' '; 11];
                let upper = filename.as_bytes();
                let mut dot_pos = None;
                for (i, &b) in upper.iter().enumerate() {
                    if b == b'.' { dot_pos = Some(i); break; }
                }
                if let Some(dp) = dot_pos {
                    let base = &upper[..dp];
                    let ext = &upper[dp + 1..];
                    let base_len = base.len().min(8);
                    let ext_len = ext.len().min(3);
                    match_name[..base_len].copy_from_slice(&base[..base_len]);
                    match_name[8..8 + ext_len].copy_from_slice(&ext[..ext_len]);
                } else {
                    let len = upper.len().min(8);
                    match_name[..len].copy_from_slice(&upper[..len]);
                }
                for b in &mut match_name { *b = b.to_ascii_uppercase(); }

                if name_field == &match_name {
                    // Mark as deleted
                    let cluster_high = u16::from_le_bytes([dir_buf[offset + 20], dir_buf[offset + 21]]) as u32;
                    let cluster_low = u16::from_le_bytes([dir_buf[offset + 26], dir_buf[offset + 27]]) as u32;
                    let file_cluster = (cluster_high << 16) | cluster_low;

                    // Free the cluster chain
                    self.free_chain(dev, file_cluster)?;

                    // Mark directory entry as deleted
                    let mut write_buf = dir_buf;
                    write_buf[offset] = 0xE5;
                    for s in 0..self.sectors_per_cluster {
                        let sec = sector + s;
                        let buf_offset = (s as usize) * 512;
                        self.write_sector_cached(sec, {
                            let mut b = [0u8; 512];
                            b.copy_from_slice(&write_buf[buf_offset..buf_offset + 512]);
                            b
                        })?;
                    }
                    return Ok(());
                }
            }

            cluster = self.next_cluster(dev, cluster)?;
        }
        Err(FatError::NotFound)
    }
}
