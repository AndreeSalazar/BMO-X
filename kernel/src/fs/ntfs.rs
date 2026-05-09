//! NTFS filesystem support via the `ntfs` crate.

use crate::fs::{DiskReader, DiskError};
use ntfs::Ntfs;
use alloc::vec::Vec;
use binrw::io::{Read, Seek, SeekFrom, Error, ErrorKind};

pub struct NtfsWrapper<D: DiskReader> {
    disk: D,
    position: u64,
}

impl<D: DiskReader> NtfsWrapper<D> {
    pub fn new(disk: D) -> Self {
        Self {
            disk,
            position: 0,
        }
    }

    pub fn mount(&mut self) -> core::result::Result<Ntfs, DiskError> {
        Ntfs::new(self).map_err(|_| DiskError::ControllerError)
    }
}

impl<D: DiskReader> Read for NtfsWrapper<D> {
    fn read(&mut self, buf: &mut [u8]) -> binrw::io::Result<usize> {
        let lba = self.position / 512;
        let offset_in_lba = (self.position % 512) as usize;
        let bytes_to_read = buf.len();
        
        let count = (bytes_to_read + offset_in_lba + 511) / 512;
        let mut temp = Vec::with_capacity(count * 512);
        temp.resize(count * 512, 0);
        
        self.disk.read_sectors(lba, count as u32, &mut temp)
            .map_err(|_| Error::new(ErrorKind::Other, "Disk read error"))?;
        
        let start = offset_in_lba;
        let end = start + bytes_to_read;
        buf.copy_from_slice(&temp[start..end]);
        
        self.position += bytes_to_read as u64;
        Ok(bytes_to_read)
    }
}

impl<D: DiskReader> Seek for NtfsWrapper<D> {
    fn seek(&mut self, pos: SeekFrom) -> binrw::io::Result<u64> {
        match pos {
            SeekFrom::Start(p) => self.position = p,
            SeekFrom::Current(p) => self.position = (self.position as i64 + p) as u64,
            SeekFrom::End(_) => return Err(Error::new(ErrorKind::Other, "Seek from end not supported")),
        }
        Ok(self.position)
    }
}
