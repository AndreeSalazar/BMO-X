pub mod traits;

pub use traits::{FileSystem, FileHandle, FileMetadata, FileType};

/// exFAT filesystem — thin wrapper around exfat-slim concepts.
///
/// This module provides a minimal exFAT implementation for kernel logging.
/// Full implementation will use the exfat-slim crate.

/// Simple exFAT filesystem over a block device.
pub struct ExFatFs<D: super::storage::BlockDevice> {
    device: D,
    partition_start: u64,
    bytes_per_cluster: u32,
    first_data_sector: u64,
    fat_start_sector: u64,
    root_cluster: u32,
}

impl<D: super::storage::BlockDevice> ExFatFs<D> {
    /// Open an exFAT filesystem on the given device and partition.
    pub fn open(mut device: D, partition_start: u64) -> Result<Self, FsError> {
        let bs = device.sector_size();

        // Read boot sector
        let mut sector = alloc::vec![0u8; bs];
        if !device.read_sectors(partition_start, 1, &mut sector) {
            return Err(FsError::IoError);
        }

        // Verify exFAT signature
        if &sector[3..11] != b"EXFAT   " {
            return Err(FsError::NotExFat);
        }

        let bytes_per_sector_shift = sector[108];
        let bytes_per_sector = 1u32 << bytes_per_sector_shift;
        let sectors_per_cluster_shift = sector[110];
        let sectors_per_cluster = 1u32 << sectors_per_cluster_shift;
        let bytes_per_cluster = bytes_per_sector * sectors_per_cluster;

        let fat_offset = read_u64(&sector, 80); // bytes
        let fat_start_sector = partition_start + fat_offset / bytes_per_sector as u64;

        let cluster_heap_offset = read_u64(&sector, 88); // bytes
        let first_data_sector = partition_start + cluster_heap_offset / bytes_per_sector as u64;

        let root_cluster = read_u32(&sector, 96);

        Ok(ExFatFs {
            device,
            partition_start,
            bytes_per_cluster,
            first_data_sector,
            fat_start_sector,
            root_cluster,
        })
    }

    /// Read a cluster chain from FAT.
    pub fn read_cluster_chain(&mut self, start_cluster: u32) -> Result<alloc::vec::Vec<u32>, FsError> {
        let mut clusters = alloc::vec::Vec::new();
        let mut current = start_cluster;

        loop {
            clusters.push(current);
            let next = self.fat_read(current)?;
            if next >= 0xFFFFFFF8 {
                break;
            }
            current = next;
        }

        Ok(clusters)
    }

    /// Read a single cluster from disk.
    pub fn read_cluster(&mut self, cluster: u32, buf: &mut [u8]) -> Result<(), FsError> {
        let lba = self.cluster_to_lba(cluster);
        let sectors = self.bytes_per_cluster / self.device.sector_size() as u32;
        if !self.device.read_sectors(lba, sectors as u16, buf) {
            return Err(FsError::IoError);
        }
        Ok(())
    }

    /// Write a single cluster to disk.
    pub fn write_cluster(&mut self, cluster: u32, buf: &[u8]) -> Result<(), FsError> {
        let lba = self.cluster_to_lba(cluster);
        let sectors = self.bytes_per_cluster / self.device.sector_size() as u32;
        if !self.device.write_sectors(lba, sectors as u16, buf) {
            return Err(FsError::IoError);
        }
        Ok(())
    }

    /// Append data to a file (simple: just allocate clusters and write).
    pub fn append_file(&mut self, _path: &str, data: &[u8]) -> Result<(), FsError> {
        // Simplified: write data starting at root cluster offset
        // Full implementation needs directory traversal
        let clusters_needed = (data.len() + self.bytes_per_cluster as usize - 1) / self.bytes_per_cluster as usize;
        let clusters = self.read_cluster_chain(self.root_cluster)?;

        for (i, chunk) in data.chunks(self.bytes_per_cluster as usize).enumerate() {
            if i >= clusters.len() {
                // Need to allocate more clusters
                let new_cluster = self.fat_alloc()?;
                if let Some(&last) = clusters.last() {
                    self.fat_write(last, new_cluster)?;
                }
                self.fat_write(new_cluster, 0xFFFFFFF8)?;
                let mut buf = alloc::vec![0u8; self.bytes_per_cluster as usize];
                buf[..chunk.len()].copy_from_slice(chunk);
                self.write_cluster(new_cluster, &buf)?;
            } else {
                let mut buf = alloc::vec![0u8; self.bytes_per_cluster as usize];
                self.read_cluster(clusters[i], &mut buf)?;
                buf[..chunk.len()].copy_from_slice(chunk);
                self.write_cluster(clusters[i], &buf)?;
            }
        }

        Ok(())
    }

    fn cluster_to_lba(&self, cluster: u32) -> u64 {
        self.first_data_sector + ((cluster as u64 - 2) * (self.bytes_per_cluster / self.device.sector_size() as u32) as u64)
    }

    fn fat_read(&mut self, cluster: u32) -> Result<u32, FsError> {
        let offset = cluster as u64 * 4;
        let sector = self.fat_start_sector + offset / self.device.sector_size() as u64;
        let mut buf = [0u8; 4];
        if !self.device.read_bytes(sector * self.device.sector_size() as u64 + (offset % self.device.sector_size() as u64), &mut buf) {
            return Err(FsError::IoError);
        }
        Ok(u32::from_le_bytes(buf))
    }

    fn fat_write(&mut self, cluster: u32, value: u32) -> Result<(), FsError> {
        let offset = cluster as u64 * 4;
        let byte_offset = self.fat_start_sector * self.device.sector_size() as u64 + offset;
        let buf = value.to_le_bytes();
        if !self.device.write_bytes(byte_offset, &buf) {
            return Err(FsError::IoError);
        }
        Ok(())
    }

    fn fat_alloc(&mut self) -> Result<u32, FsError> {
        // Simple: scan FAT for free entry
        let mut test_cluster = 2u32;
        loop {
            let val = self.fat_read(test_cluster)?;
            if val == 0 {
                return Ok(test_cluster);
            }
            test_cluster += 1;
            if test_cluster > 0x0FFFFFF6 {
                return Err(FsError::DiskFull);
            }
        }
    }

    pub fn device(&self) -> &D {
        &self.device
    }

    pub fn bytes_per_cluster(&self) -> u32 {
        self.bytes_per_cluster
    }
}

/// Filesystem errors.
#[derive(Debug)]
pub enum FsError {
    IoError,
    NotExFat,
    DiskFull,
    FileNotFound,
    InvalidPath,
}

fn read_u64(data: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap())
}

fn read_u32(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap())
}
