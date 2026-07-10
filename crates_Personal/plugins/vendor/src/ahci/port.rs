use super::hba::*;
use alloc::alloc::{alloc_zeroed, Layout};
use core::ptr::{NonNull, read_volatile, write_volatile};

/// AHCI port — manages a single SATA port.
pub struct AhciPort {
    pub mmio: usize,
    pub port_num: u8,
    pub port_offset: usize,
    pub cmd_list: NonNull<CmdHeader>,
    pub cmd_table: NonNull<CmdTable>,
    pub lba48: bool,
    pub max_lba: u64,
}

impl AhciPort {
    /// Initialize a port. Returns None if no device present.
    pub unsafe fn new(mmio: usize, port_num: u8) -> Option<Self> {
        let port_off = 0x100 + (port_num as usize) * 0x80;
        let port_base = mmio + port_off;

        // Read SATA Status
        let ssts = read_volatile((port_base + 0x28) as *const u32);
        let det = ssts & 0x0F;
        if det != SSTS_DET_PRESENT {
            return None;
        }

        // Stop port (clear ST and FRE)
        let cmd = read_volatile((port_base + 0x18) as *const u32);
        write_volatile((port_base + 0x18) as *mut u32, cmd & !(PORT_CMD_ST | PORT_CMD_FRE));

        // Wait for CR and FR to clear
        wait_until(1000, || {
            let c = read_volatile((port_base + 0x18) as *const u32);
            (c & (1 << 15) == 0) && (c & (1 << 14) == 0)
        });

        // Check if device is busy
        let tfd = read_volatile((port_base + 0x20) as *const u32);
        if tfd & (TFD_BSY | TFD_DRQ) != 0 {
            let cmd = read_volatile((port_base + 0x18) as *const u32);
            write_volatile((port_base + 0x18) as *mut u32, cmd | PORT_CMD_CLO);
            wait_until(1000, || {
                read_volatile((port_base + 0x18) as *const u32) & PORT_CMD_CLO == 0
            });
        }

        // Spin up
        let cmd = read_volatile((port_base + 0x18) as *const u32);
        write_volatile((port_base + 0x18) as *mut u32, cmd | PORT_CMD_SUD);

        // Wait for link
        wait_until(1000, || {
            let det = read_volatile((port_base + 0x28) as *const u32) & 0x0F;
            det == 0x01 || det == 0x03
        });

        // Clear errors
        write_volatile((port_base + 0x30) as *mut u32, 0xFFFF);
        write_volatile((port_base + 0x10) as *mut u32, 0xFFFF);

        // Allocate command list (1KB aligned)
        let cmd_list_layout = Layout::from_size_align(32 * size_of::<CmdHeader>(), 1024).unwrap();
        let cmd_list_ptr = alloc_zeroed(cmd_list_layout);
        if cmd_list_ptr.is_null() { return None; }
        let cmd_list = NonNull::new_unchecked(cmd_list_ptr as *mut CmdHeader);

        // Allocate command table (128 byte aligned)
        let cmd_tbl_layout = Layout::from_size_align(size_of::<CmdTable>(), 128).unwrap();
        let cmd_tbl_ptr = alloc_zeroed(cmd_tbl_layout);
        if cmd_tbl_ptr.is_null() { return None; }
        let cmd_tbl = NonNull::new_unchecked(cmd_tbl_ptr as *mut CmdTable);

        // Set command list base
        let clb_addr = cmd_list.as_ptr() as u64;
        write_volatile((port_base + 0x00) as *mut u32, clb_addr as u32);
        write_volatile((port_base + 0x04) as *mut u32, (clb_addr >> 32) as u32);

        // Start port
        let cmd = read_volatile((port_base + 0x18) as *const u32);
        write_volatile((port_base + 0x18) as *mut u32, cmd | PORT_CMD_FRE | PORT_CMD_ST);

        // Enable interrupts
        write_volatile((port_base + 0x14) as *mut u32, 0x01);

        // Identify device
        let mut port = AhciPort {
            mmio, port_num, port_offset: port_off,
            cmd_list, cmd_table: cmd_tbl,
            lba48: false, max_lba: 0,
        };

        if !port.identify_device() {
            return None;
        }

        Some(port)
    }

    fn port_reg(&self, offset: u32) -> usize {
        self.mmio + self.port_offset + offset as usize
    }

    /// Send ATA IDENTIFY DEVICE command to detect disk parameters.
    unsafe fn identify_device(&mut self) -> bool {
        let mut buf = [0u8; 512];
        let fis = FisH2D::new_command(ATA_CMD_IDENTIFY, 0, 0, false);
        if !self.exec_command(fis, &mut buf, false) {
            return false;
        }

        let id = core::slice::from_raw_parts(buf.as_ptr() as *const u16, 256);

        // Words 100-103: max 48-bit LBA
        let lo = id[100] as u64;
        let hi = id[101] as u64;
        self.max_lba = lo | (hi << 32);

        // Word 83 bit 10: LBA48 support
        self.lba48 = (id[83] & (1 << 10)) != 0;

        true
    }

    /// Execute a command on this port.
    pub unsafe fn exec_command(&mut self, fis: FisH2D, buf: &mut [u8], is_write: bool) -> bool {
        // Wait for slot 0 to be free
        if !wait_until(1000, || read_volatile(self.port_reg(0x38) as *const u32) & 1 == 0) {
            return false;
        }

        // Write FIS to command table
        let tbl = self.cmd_table.as_ptr();
        core::ptr::copy_nonoverlapping(
            &fis as *const FisH2D as *const u8,
            (*tbl).cfis.as_mut_ptr(),
            size_of::<FisH2D>(),
        );

        // Set up scatter-gather if buffer provided
        if !buf.is_empty() {
            let buf_addr = buf.as_ptr() as u64;
            (*tbl).sg[0] = SgEntry {
                addr_lo: buf_addr as u32,
                addr_hi: (buf_addr >> 32) as u32,
                reserved: 0,
                flags_size: (buf.len() - 1) as u32,
            };
        }

        // Build command header
        let cfl = (size_of::<FisH2D>() / 4) as u32;
        let opts = cfl | (1 << 16) | ((is_write as u32) << 6);
        let tbl_addr = self.cmd_table.as_ptr() as u64;

        let hdr = &mut *self.cmd_list.as_ptr();
        *hdr = CmdHeader {
            opts,
            status: 0,
            tbl_addr_lo: tbl_addr as u32,
            tbl_addr_hi: (tbl_addr >> 32) as u32,
            reserved: [0; 4],
        };

        // Issue command
        write_volatile(self.port_reg(0x38) as *mut u32, 1);

        // Wait for completion
        let ok = wait_until(5000, || read_volatile(self.port_reg(0x38) as *const u32) & 1 == 0);

        ok
    }

    /// Read sectors from disk.
    pub unsafe fn read_sectors(&mut self, lba: u64, count: u16, buf: &mut [u8]) -> bool {
        let cmd = if self.lba48 { ATA_CMD_READ_DMA_EXT } else { ATA_CMD_READ_EXT };
        let fis = FisH2D::new_command(cmd, lba, count, self.lba48);
        self.exec_command(fis, buf, false)
    }

    /// Write sectors to disk.
    pub unsafe fn write_sectors(&mut self, lba: u64, count: u16, buf: &[u8]) -> bool {
        let cmd = if self.lba48 { ATA_CMD_WRITE_DMA_EXT } else { ATA_CMD_WRITE_EXT };
        let mut buf_copy = buf.to_vec();
        let fis = FisH2D::new_command(cmd, lba, count, self.lba48);
        self.exec_command(fis, &mut buf_copy, true)
    }
}
