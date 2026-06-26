/// Block device abstraction — common interface for all storage backends.
pub trait BlockDevice {
    /// Read sectors from the device.
    fn read_sectors(&mut self, lba: u64, count: u16, buf: &mut [u8]) -> bool;

    /// Write sectors to the device.
    fn write_sectors(&mut self, lba: u64, count: u16, buf: &[u8]) -> bool;

    /// Get the sector size in bytes (usually 512).
    fn sector_size(&self) -> usize;

    /// Get total number of sectors.
    fn total_sectors(&self) -> u64;

    /// Read bytes at a byte offset.
    fn read_bytes(&mut self, offset: u64, buf: &mut [u8]) -> bool {
        let bs = self.sector_size() as u64;
        let start_sector = offset / bs;
        let start_offset = (offset % bs) as usize;
        let total = start_offset + buf.len();
        let sectors = ((total + self.sector_size() - 1) / self.sector_size()) as u16;

        if sectors == 0 { return true; }

        let mut temp = alloc::vec![0u8; sectors as usize * self.sector_size()];
        if !self.read_sectors(start_sector, sectors, &mut temp) {
            return false;
        }

        let end = (start_offset + buf.len()).min(temp.len());
        buf.copy_from_slice(&temp[start_offset..end]);
        true
    }

    /// Write bytes at a byte offset (read-modify-write for partial sectors).
    fn write_bytes(&mut self, offset: u64, data: &[u8]) -> bool {
        let bs = self.sector_size() as u64;
        let start_sector = offset / bs;
        let start_offset = (offset % bs) as usize;
        let total = start_offset + data.len();
        let sectors = ((total + self.sector_size() - 1) / self.sector_size()) as u16;

        if sectors == 0 { return true; }

        let mut temp = alloc::vec![0u8; sectors as usize * self.sector_size()];
        if start_offset > 0 || total < temp.len() {
            if !self.read_sectors(start_sector, sectors, &mut temp) {
                return false;
            }
        }

        let end = (start_offset + data.len()).min(temp.len());
        temp[start_offset..end].copy_from_slice(&data[..end - start_offset]);

        self.write_sectors(start_sector, sectors, &temp)
    }
}

/// Wrapper to make any block device usable as a byte-oriented device.
pub struct ByteDevice<D: BlockDevice> {
    inner: D,
}

impl<D: BlockDevice> ByteDevice<D> {
    pub fn new(device: D) -> Self {
        Self { inner: device }
    }

    pub fn inner(&self) -> &D {
        &self.inner
    }

    pub fn inner_mut(&mut self) -> &mut D {
        &mut self.inner
    }
}

impl<D: BlockDevice> BlockDevice for ByteDevice<D> {
    fn read_sectors(&mut self, lba: u64, count: u16, buf: &mut [u8]) -> bool {
        self.inner.read_sectors(lba, count, buf)
    }

    fn write_sectors(&mut self, lba: u64, count: u16, buf: &[u8]) -> bool {
        self.inner.write_sectors(lba, count, buf)
    }

    fn sector_size(&self) -> usize {
        self.inner.sector_size()
    }

    fn total_sectors(&self) -> u64 {
        self.inner.total_sectors()
    }
}
