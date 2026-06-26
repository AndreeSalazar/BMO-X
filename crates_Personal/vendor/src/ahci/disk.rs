use super::port::AhciPort;
use super::hba::*;
use core::ptr::{read_volatile, write_volatile};

/// AHCI disk — high-level block device interface.
pub struct AhciDisk {
    port: AhciPort,
    block_size: usize,
}

impl AhciDisk {
    /// Create a new AHCI disk from an MMIO base address.
    /// Returns the disk if a valid SATA device was found.
    pub unsafe fn new(mmio: usize) -> Option<Self> {
        // Reset HBA
        let ghc = mmio as *mut u32;
        write_volatile(ghc, GHC_HR);
        wait_until(1000, || read_volatile(ghc) & GHC_HR == 0);

        // Enable AHCI mode
        let ghc_val = read_volatile(ghc);
        write_volatile(ghc, ghc_val | GHC_AE);

        // Enable all ports
        let pi = (mmio + 0x0C) as *mut u32;
        write_volatile(pi, 0xFFFFFFFF);

        // Enable interrupts globally
        let ghc_val = read_volatile(ghc);
        write_volatile(ghc, ghc_val | GHC_IE);

        // Find first active port
        let ports_implemented = read_volatile((mmio + 0x0C) as *const u32);
        for i in 0..32 {
            if ports_implemented & (1 << i) != 0 {
                if let Some(port) = AhciPort::new(mmio, i as u8) {
                    return Some(AhciDisk {
                        port,
                        block_size: 512,
                    });
                }
            }
        }

        None
    }

    /// Get disk capacity in sectors.
    pub fn max_lba(&self) -> u64 {
        self.port.max_lba
    }

    /// Get block size in bytes.
    pub fn block_size(&self) -> usize {
        self.block_size
    }

    /// Check if disk supports 48-bit LBA.
    pub fn lba48(&self) -> bool {
        self.port.lba48
    }

    /// Read sectors from disk.
    /// `lba`: starting sector
    /// `count`: number of sectors
    /// `buf`: buffer to read into (must be >= count * 512 bytes)
    pub unsafe fn read(&mut self, lba: u64, count: u16, buf: &mut [u8]) -> bool {
        self.port.read_sectors(lba, count, buf)
    }

    /// Write sectors to disk.
    /// `lba`: starting sector
    /// `count`: number of sectors
    /// `buf`: data to write (must be >= count * 512 bytes)
    pub unsafe fn write(&mut self, lba: u64, count: u16, buf: &[u8]) -> bool {
        self.port.write_sectors(lba, count, buf)
    }

    /// Read bytes from disk (handles partial sectors).
    pub unsafe fn read_bytes(&mut self, offset: u64, buf: &mut [u8]) -> bool {
        let bs = self.block_size as u64;
        let start_sector = offset / bs;
        let start_offset = (offset % bs) as usize;
        let total = start_offset + buf.len();
        let sectors_needed = ((total + self.block_size - 1) / self.block_size) as u16;

        if sectors_needed == 0 { return true; }

        let mut temp = alloc::vec![0u8; sectors_needed as usize * self.block_size];
        if !self.port.read_sectors(start_sector, sectors_needed, &mut temp) {
            return false;
        }

        let end = (start_offset + buf.len()).min(temp.len());
        buf.copy_from_slice(&temp[start_offset..end]);
        true
    }

    /// Write bytes to disk (handles partial sectors with read-modify-write).
    pub unsafe fn write_bytes(&mut self, offset: u64, data: &[u8]) -> bool {
        let bs = self.block_size as u64;
        let start_sector = offset / bs;
        let start_offset = (offset % bs) as usize;
        let total = start_offset + data.len();
        let sectors_needed = ((total + self.block_size - 1) / self.block_size) as u16;

        if sectors_needed == 0 { return true; }

        // Read existing data first (for partial sector writes)
        let mut temp = alloc::vec![0u8; sectors_needed as usize * self.block_size];
        if start_offset > 0 || total < temp.len() {
            if !self.port.read_sectors(start_sector, sectors_needed, &mut temp) {
                return false;
            }
        }

        // Copy new data
        let end = (start_offset + data.len()).min(temp.len());
        temp[start_offset..end].copy_from_slice(&data[..end - start_offset]);

        // Write back
        self.port.write_sectors(start_sector, sectors_needed, &temp)
    }
}
