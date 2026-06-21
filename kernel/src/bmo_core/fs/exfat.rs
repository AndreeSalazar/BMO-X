//! exFAT filesystem driver for FastOS.
//!
//! exFAT (Extended File Allocation Table) is used for the user data
//! partition. It supports large files (>4GB), long filenames, and is
//! cross-platform compatible.
//!
//! Status: Core read support implemented. Write support pending disk driver.

#![allow(dead_code)]

use crate::dev::console;

const SECTOR_SIZE: usize = 512;
const MAX_OPEN_FILES: usize = 32;
const MAX_CACHE_SECTORS: usize = 8;

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
    NotMounted,
    BadHandle,
}

impl ExfatError {
    pub fn as_str(&self) -> &'static str {
        match self {
            ExfatError::IoError => "I/O error",
            ExfatError::BadSignature => "bad exFAT signature",
            ExfatError::InvalidCluster => "invalid cluster chain",
            ExfatError::NoSpace => "no free space",
            ExfatError::FileNotFound => "file not found",
            ExfatError::NotADirectory => "not a directory",
            ExfatError::TooManyOpen => "too many open files",
            ExfatError::NotMounted => "volume not mounted",
            ExfatError::BadHandle => "bad file handle",
        }
    }
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

    pub fn total_sectors(&self) -> u64 {
        self.volume_length
    }

    pub fn fat_start_sector(&self) -> u64 {
        self.partition_offset + self.fat_offset as u64
    }

    pub fn cluster_heap_start_sector(&self) -> u64 {
        self.partition_offset + self.cluster_heap_offset as u64
    }
}

/// Directory entry for exFAT files/dirs.
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

    pub fn is_deleted(&self) -> bool {
        self.entry_type == 0xE5
    }
}

/// LRU sector cache for exFAT.
#[derive(Debug, Clone, Copy)]
struct CacheEntry {
    lba: u64,
    data: [u8; SECTOR_SIZE],
    dirty: bool,
    valid: bool,
}

impl CacheEntry {
    const fn empty() -> Self {
        Self {
            lba: 0,
            data: [0; SECTOR_SIZE],
            dirty: false,
            valid: false,
        }
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

/// exFAT filesystem state.
struct ExfatState {
    header: ExfatVolumeHeader,
    mounted: bool,
    open_files: [ExfatOpenFile; MAX_OPEN_FILES],
    sector_cache: [CacheEntry; MAX_CACHE_SECTORS],
    cache_lru: [u32; MAX_CACHE_SECTORS],
    lru_counter: u32,
}

impl ExfatState {
    const fn new() -> Self {
        Self {
            header: ExfatVolumeHeader {
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
            },
            mounted: false,
            open_files: [ExfatOpenFile { in_use: false, first_cluster: 0, file_size: 0, position: 0 }; MAX_OPEN_FILES],
            sector_cache: [CacheEntry::empty(); MAX_CACHE_SECTORS],
            cache_lru: [0u32; MAX_CACHE_SECTORS],
            lru_counter: 0,
        }
    }
}

static mut STATE: ExfatState = ExfatState::new();

/// Read a sector from the cache or from disk.
fn read_sector_cached(lba: u64, disk: &mut dyn crate::bmo_core::fs::DiskReader) -> Result<[u8; SECTOR_SIZE], ExfatError> {
    unsafe {
        // Check cache first
        for i in 0..MAX_CACHE_SECTORS {
            if STATE.sector_cache[i].valid && STATE.sector_cache[i].lba == lba {
                STATE.lru_counter += 1;
                STATE.cache_lru[i] = STATE.lru_counter;
                return Ok(STATE.sector_cache[i].data);
            }
        }

        // Cache miss — read from disk
        let mut buf = [0u8; SECTOR_SIZE];
        disk.read_sectors(lba, 1, &mut buf).map_err(|_| ExfatError::IoError)?;

        // Find LRU slot to evict
        let mut lru_slot = 0;
        let mut lru_min = u32::MAX;
        for i in 0..MAX_CACHE_SECTORS {
            if !STATE.sector_cache[i].valid {
                lru_slot = i;
                break;
            }
            if STATE.cache_lru[i] < lru_min {
                lru_min = STATE.cache_lru[i];
                lru_slot = i;
            }
        }

        STATE.sector_cache[lru_slot] = CacheEntry {
            lba,
            data: buf,
            dirty: false,
            valid: true,
        };
        STATE.lru_counter += 1;
        STATE.cache_lru[lru_slot] = STATE.lru_counter;

        Ok(buf)
    }
}

/// Read a cluster's data. Returns the raw sector data.
fn read_cluster(cluster: u32, disk: &mut dyn crate::bmo_core::fs::DiskReader) -> Result<[u8; SECTOR_SIZE], ExfatError> {
    if cluster < 2 {
        return Err(ExfatError::InvalidCluster);
    }
    unsafe {
        let start_lba = STATE.header.cluster_heap_start_sector()
            + ((cluster - 2) as u64) * (1u64 << STATE.header.sectors_per_cluster_shift);
        read_sector_cached(start_lba, disk)
    }
}

/// Walk the FAT chain to find the Nth cluster.
fn get_cluster_in_chain(chain_start: u32, index: u32, disk: &mut dyn crate::bmo_core::fs::DiskReader) -> Result<u32, ExfatError> {
    if index == 0 {
        return Ok(chain_start);
    }

    let mut current = chain_start;
    for _ in 0..index {
        let lba = unsafe { STATE.header.fat_start_sector() } + (current as u64 / (SECTOR_SIZE as u64 / 4));
        let offset = (current as usize % (SECTOR_SIZE / 4)) * 4;
        let sector = read_sector_cached(lba, disk)?;
        let entry = u32::from_ne_bytes([sector[offset], sector[offset + 1], sector[offset + 2], sector[offset + 3]]);

        // exFAT FAT entries: bits 0-27 = next cluster, bit 31 = end-of-chain
        if entry >= 0xFFFFFFF8 {
            return Err(ExfatError::InvalidCluster);
        }
        current = entry & 0x0FFF_FFFF;
    }
    Ok(current)
}

/// Initialize the exFAT driver with a header read from disk.
pub fn init_from_disk(
    lba: u64,
    disk: &mut dyn crate::bmo_core::fs::DiskReader,
) -> Result<(), ExfatError> {
    let header_sector = read_sector_cached(lba, disk)?;

    unsafe {
        // Copy the raw sector into the header struct
        core::ptr::copy_nonoverlapping(
            header_sector.as_ptr(),
            &mut STATE.header as *mut ExfatVolumeHeader as *mut u8,
            core::mem::size_of::<ExfatVolumeHeader>(),
        );

        if !STATE.header.is_valid() {
            console::serial_write("[exfat] Bad signature\n");
            return Err(ExfatError::BadSignature);
        }

        STATE.header.partition_offset = lba;

        console::serial_write("[exfat] Valid header: sectors=");
        serial_write_u64(STATE.header.volume_length);
        console::serial_write(" clusters=");
        serial_write_u32(STATE.header.cluster_count);
        console::serial_write("\n");
    }

    Ok(())
}

/// Mount the exFAT volume.
pub fn mount(disk: &mut dyn crate::bmo_core::fs::DiskReader) -> Result<(), ExfatError> {
    if unsafe { STATE.mounted } {
        return Err(ExfatError::IoError);
    }

    // Read volume header from sector 0
    init_from_disk(0, disk)?;

    unsafe { STATE.mounted = true; }
    console::serial_write("[exfat] Mounted\n");
    Ok(())
}

/// Unmount.
pub fn unmount() -> Result<(), ExfatError> {
    if !unsafe { STATE.mounted } {
        return Err(ExfatError::NotMounted);
    }

    // Flush dirty cache entries
    flush_cache();
    unsafe { STATE.mounted = false; }
    console::serial_write("[exfat] Unmounted\n");
    Ok(())
}

/// Flush all dirty cache entries.
fn flush_cache() {
    // Cache is read-only for now; no dirty entries
}

/// Check if mounted.
pub fn is_mounted() -> bool {
    unsafe { STATE.mounted }
}

/// Open a file by path. Scans root directory for matching filename (8.3 uppercase).
pub fn open_file(name: &str, disk: &mut dyn crate::bmo_core::fs::DiskReader) -> Result<u32, ExfatError> {
    if !unsafe { STATE.mounted } {
        return Err(ExfatError::NotMounted);
    }

    let root_cluster = unsafe { STATE.header.first_cluster };
    let target = to_exfat_83(name);
    let mut cluster = root_cluster;

    let found = loop {
        let sector_data = match read_cluster(cluster, disk) {
            Ok(d) => d,
            Err(_) => break false,
        };
        let entries_per_sector = SECTOR_SIZE / 32;
        let mut found_match = false;
        for e in 0..entries_per_sector {
            let offset = e * 32;
            if offset + 32 > sector_data.len() { break; }
            let entry_type = sector_data[offset];
            if entry_type == 0x00 { break; }
            if entry_type == 0xE5 { continue; }
            if (entry_type & 0x80) != 0 { continue; }
            if entry_type & 0x10 != 0 {
                let mut match_name = [0u8; 11];
                let name_len = (sector_data.len() - offset - 1).min(11);
                for i in 0..name_len {
                    match_name[i] = sector_data[offset + 1 + i];
                }
                if match_name == target {
                    found_match = true;
                    break;
                }
            }
        }
        if found_match { break true; }
        match get_cluster_in_chain(cluster, 1, disk) {
            Ok(c) if c >= 2 && c < 0xFFFFF8 => cluster = c,
            _ => break false,
        }
    };

    if !found {
        return Err(ExfatError::FileNotFound);
    }

    unsafe {
        for i in 0..MAX_OPEN_FILES {
            if !STATE.open_files[i].in_use {
                STATE.open_files[i] = ExfatOpenFile {
                    in_use: true,
                    first_cluster: root_cluster,
                    file_size: 0,
                    position: 0,
                };
                return Ok(i as u32);
            }
        }
    }
    Err(ExfatError::TooManyOpen)
}

/// Read from an open file handle. Walks the cluster chain.
pub fn read_file(fd: u32, buf: &mut [u8], disk: &mut dyn crate::bmo_core::fs::DiskReader) -> Result<usize, ExfatError> {
    if !unsafe { STATE.mounted } {
        return Err(ExfatError::NotMounted);
    }

    let fd = fd as usize;
    if fd >= MAX_OPEN_FILES {
        return Err(ExfatError::BadHandle);
    }

    let (cluster, pos) = unsafe {
        if !STATE.open_files[fd].in_use {
            return Err(ExfatError::BadHandle);
        }
        (STATE.open_files[fd].first_cluster, STATE.open_files[fd].position)
    };

    let cluster_size = unsafe { STATE.header.cluster_size() };
    let mut bytes_read = 0usize;

    // Skip to the cluster containing our current position
    let clusters_to_skip = (pos / cluster_size as u64) as u32;
    let mut current_cluster = cluster;
    for _ in 0..clusters_to_skip {
        match get_cluster_in_chain(current_cluster, 1, disk) {
            Ok(c) if c >= 2 && c < 0xFFFFF8 => current_cluster = c,
            _ => return Ok(bytes_read),
        }
    }

    let offset_in_cluster = (pos % cluster_size as u64) as usize;

    // Read data from clusters
    let mut remaining = buf.len();
    let mut dst_offset = 0usize;
    let mut local_offset = offset_in_cluster;

    loop {
        if remaining == 0 { break; }
        let sector_data = read_cluster(current_cluster, disk)?;
        let to_copy = (cluster_size - local_offset).min(remaining).min(sector_data.len() - local_offset);
        buf[dst_offset..dst_offset + to_copy].copy_from_slice(&sector_data[local_offset..local_offset + to_copy]);
        bytes_read += to_copy;
        dst_offset += to_copy;
        remaining -= to_copy;
        local_offset = 0;

        // Move to next cluster
        match get_cluster_in_chain(current_cluster, 1, disk) {
            Ok(c) if c >= 2 && c < 0xFFFFF8 => current_cluster = c,
            _ => break,
        }
    }

    // Update position
    unsafe { STATE.open_files[fd].position += bytes_read as u64; }
    Ok(bytes_read)
}

/// Close an open file handle.
pub fn close_file(fd: u32) -> Result<(), ExfatError> {
    let fd = fd as usize;
    if fd >= MAX_OPEN_FILES {
        return Err(ExfatError::BadHandle);
    }

    unsafe {
        if !STATE.open_files[fd].in_use {
            return Err(ExfatError::BadHandle);
        }
        STATE.open_files[fd].in_use = false;
    }

    Ok(())
}

/// Get volume info.
pub fn volume_info() -> Option<(u64, u32, u32)> {
    unsafe {
        if !STATE.mounted { return None; }
        Some((
            STATE.header.volume_length,
            STATE.header.cluster_count,
            STATE.header.first_cluster,
        ))
    }
}

/// Get cache stats.
pub fn cache_stats() -> (usize, usize) {
    unsafe {
        let total = MAX_CACHE_SECTORS;
        let valid = STATE.sector_cache.iter().filter(|c| c.valid).count();
        (valid, total)
    }
}

fn to_exfat_83(name: &str) -> [u8; 11] {
    let mut result = [0x20u8; 11];
    let bytes = name.as_bytes();
    let mut dot_pos = bytes.len();
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'.' { dot_pos = i; break; }
    }
    let name_part = &bytes[..dot_pos.min(8)];
    let ext_part = if dot_pos < bytes.len() { &bytes[dot_pos + 1..] } else { &[] };
    for (i, &b) in name_part.iter().enumerate().take(8) {
        result[i] = if b >= b'a' && b <= b'z' { b - 32 } else { b };
    }
    for (i, &b) in ext_part.iter().enumerate().take(3) {
        result[8 + i] = if b >= b'a' && b <= b'z' { b - 32 } else { b };
    }
    result
}

fn serial_write_u64(val: u64) {
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    let mut v = val;
    if v == 0 { i -= 1; buf[i] = b'0'; }
    else { while v > 0 { i -= 1; buf[i] = b'0' + (v % 10) as u8; v /= 10; } }
    console::serial_write(core::str::from_utf8(&buf[i..]).unwrap_or("0"));
}

fn serial_write_u32(val: u32) {
    serial_write_u64(val as u64);
}
