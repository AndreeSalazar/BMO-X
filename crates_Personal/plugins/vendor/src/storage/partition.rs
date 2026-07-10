use super::block::BlockDevice;

/// Partition type identifiers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PartitionType {
    Unknown(u8),
    Fat12,
    Fat16,
    Fat32,
    Fat32Lba,
    ExFat,
    Linux,
    EfSystem,
    EfIsp,
    MicrosoftReserved,
}

impl PartitionType {
    pub fn from_guid(guid: &[u8; 16]) -> Self {
        // EFI System Partition GUID: C12A7328-F81F-11D2-BA4B-00A0C93EC93B
        if guid == b"\xC1\x2A\x73\x28\xF8\x1F\x11\xD2\xBA\x4B\x00\xA0\xC9\x3E\xC9\x3B" {
            return PartitionType::EfSystem;
        }
        PartitionType::Unknown(guid[0])
    }

    pub fn from_mbr(id: u8) -> Self {
        match id {
            0x01 => PartitionType::Fat12,
            0x04 | 0x06 => PartitionType::Fat16,
            0x0B => PartitionType::Fat32,
            0x0C | 0x0E => PartitionType::Fat32Lba,
            0x07 => PartitionType::MicrosoftReserved,
            0x83 => PartitionType::Linux,
            _ => PartitionType::Unknown(id),
        }
    }
}

/// A partition entry.
#[derive(Debug, Clone, Copy)]
pub struct Partition {
    pub index: usize,
    pub partition_type: PartitionType,
    pub start_lba: u64,
    pub sectors: u64,
    pub bootable: bool,
}

/// GPT header (at LBA 1).
#[repr(C)]
#[derive(Clone, Copy)]
struct GptHeader {
    signature: [u8; 8],
    revision: u32,
    header_size: u32,
    header_crc32: u32,
    _reserved: u32,
    my_lba: u64,
    alternate_lba: u64,
    first_usable_lba: u64,
    last_usable_lba: u64,
    disk_guid: [u8; 16],
    partition_entries_lba: u64,
    num_partition_entries: u32,
    partition_entry_size: u32,
    partition_entries_crc32: u32,
}

/// GPT partition entry (128 bytes each).
#[repr(C)]
#[derive(Clone, Copy)]
struct GptEntry {
    type_guid: [u8; 16],
    unique_guid: [u8; 16],
    start_lba: u64,
    end_lba: u64,
    attributes: u64,
    name: [u8; 72], // UTF-16LE
}

/// MBR partition entry.
#[repr(C)]
#[derive(Clone, Copy)]
struct MbrEntry {
    status: u8,
    chs_start: [u8; 3],
    partition_type: u8,
    chs_end: [u8; 3],
    lba_start: u32,
    sectors: u32,
}

/// Detected partition table.
pub enum PartitionTable {
    Gpt {
        partitions: [Option<Partition>; 128],
        count: usize,
    },
    Mbr {
        partitions: [Option<Partition>; 4],
        count: usize,
    },
    None,
}

impl PartitionTable {
    /// Detect and parse partition table from a block device.
    pub fn detect(device: &mut dyn BlockDevice) -> Self {
        let mut sector = [0u8; 512];
        if !device.read_sectors(0, 1, &mut sector) {
            return PartitionTable::None;
        }

        // Check for GPT signature at LBA 1
        if &sector[0..8] == b"EFI PART" {
            return Self::parse_gpt(device);
        }

        // Check for MBR signature
        if sector[510] == 0x55 && sector[511] == 0xAA {
            return Self::parse_mbr(&sector);
        }

        PartitionTable::None
    }

    fn parse_gpt(device: &mut dyn BlockDevice) -> Self {
        let mut hdr_sector = [0u8; 512];
        if !device.read_sectors(1, 1, &mut hdr_sector) {
            return PartitionTable::None;
        }

        let hdr = unsafe { core::ptr::read(hdr_sector.as_ptr() as *const GptHeader) };
        let entry_size = hdr.partition_entry_size as usize;
        let num_entries = hdr.num_partition_entries as usize;
        let entries_per_sector = 512 / entry_size;

        let mut partitions = [None::<Partition>; 128];
        let mut count = 0;
        let mut remaining = num_entries;
        let mut lba = hdr.partition_entries_lba;

        while remaining > 0 && count < 128 {
            let mut entry_sector = [0u8; 512];
            if !device.read_sectors(lba, 1, &mut entry_sector) {
                break;
            }

            for i in 0..entries_per_sector.min(remaining) {
                let offset = i * entry_size;
                if offset + entry_size > 512 { break; }

                let entry = unsafe {
                    core::ptr::read(entry_sector[offset..].as_ptr() as *const GptEntry)
                };

                // Empty entry
                if entry.type_guid == [0u8; 16] { continue; }

                let ptype = PartitionType::from_guid(&entry.type_guid);
                let start = entry.start_lba;
                let end = entry.end_lba;
                if end > start {
                    partitions[count] = Some(Partition {
                        index: count,
                        partition_type: ptype,
                        start_lba: start,
                        sectors: end - start + 1,
                        bootable: false,
                    });
                    count += 1;
                }
            }

            remaining -= entries_per_sector.min(remaining);
            lba += 1;
        }

        PartitionTable::Gpt { partitions, count }
    }

    fn parse_mbr(sector: &[u8; 512]) -> Self {
        let mut partitions = [None::<Partition>; 4];
        let mut count = 0;

        for i in 0..4 {
            let offset = 446 + i * 16;
            let entry = unsafe {
                core::ptr::read(sector[offset..].as_ptr() as *const MbrEntry)
            };

            if entry.partition_type == 0 { continue; }

            let ptype = PartitionType::from_mbr(entry.partition_type);
            partitions[count] = Some(Partition {
                index: count,
                partition_type: ptype,
                start_lba: entry.lba_start as u64,
                sectors: entry.sectors as u64,
                bootable: entry.status & 0x80 != 0,
            });
            count += 1;
        }

        PartitionTable::Mbr { partitions, count }
    }

    /// Get all partitions.
    pub fn partitions(&self) -> &[Option<Partition>] {
        match self {
            PartitionTable::Gpt { partitions, .. } => partitions,
            PartitionTable::Mbr { partitions, .. } => partitions,
            PartitionTable::None => &[],
        }
    }

    /// Find partition by type.
    pub fn find_type(&self, target: PartitionType) -> Option<&Partition> {
        self.partitions().iter().filter_map(|p| p.as_ref()).find(|p| p.partition_type == target)
    }

    /// Count of partitions.
    pub fn count(&self) -> usize {
        match self {
            PartitionTable::Gpt { count, .. } | PartitionTable::Mbr { count, .. } => *count,
            PartitionTable::None => 0,
        }
    }
}
