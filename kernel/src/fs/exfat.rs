//! exFAT filesystem (pure Rust, no_std).

#[repr(C, packed)]
pub struct ExfatBootSector {
    pub jump_boot: [u8; 3],
    pub fs_name: [u8; 8],               // "EXFAT   "
    pub must_be_zero: [u8; 53],
    pub partition_offset: u64,
    pub volume_length: u64,
    pub fat_offset: u32,
    pub fat_length: u32,
    pub cluster_heap_offset: u32,
    pub cluster_count: u32,
    pub first_cluster_of_root: u32,
    pub volume_serial: u32,
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

pub mod entry_type {
    pub const END_OF_DIR: u8    = 0x00;
    pub const ALLOC_BITMAP: u8  = 0x81;
    pub const UPCASE_TABLE: u8  = 0x82;
    pub const VOLUME_LABEL: u8  = 0x83;
    pub const FILE_DIR: u8      = 0x85;
    pub const STREAM_EXT: u8    = 0xC0;
    pub const FILE_NAME: u8     = 0xC1;
}

pub fn is_exfat(bs: &ExfatBootSector) -> bool {
    &bs.fs_name == b"EXFAT   " && bs.boot_signature == 0xAA55
}

pub fn bytes_per_sector(bs: &ExfatBootSector) -> u32 {
    1u32 << bs.bytes_per_sector_shift
}

pub fn bytes_per_cluster(bs: &ExfatBootSector) -> u32 {
    bytes_per_sector(bs) << bs.sectors_per_cluster_shift
}

pub fn cluster_to_sector(bs: &ExfatBootSector, cluster: u32) -> u64 {
    let spc = 1u64 << bs.sectors_per_cluster_shift;
    bs.cluster_heap_offset as u64 + ((cluster - 2) as u64) * spc
}
