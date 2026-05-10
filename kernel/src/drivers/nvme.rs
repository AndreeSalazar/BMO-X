//! Minimal Read-Only NVMe Driver for FastOS.
//! Bare-metal with DMA bounce buffer, phase-tag polling, bus mastering.

use crate::drivers::pci::{self, PciDevice};
use crate::fs::{DiskReader, DiskError};
use crate::arch::page_alloc;
use core::ptr::{read_volatile, write_volatile};

const NVME_CLASS: u8 = 0x01;
const NVME_SUBCLASS: u8 = 0x08;

#[repr(C)]
struct NvmeRegs {
    cap: u64,
    vs: u32,
    intms: u32,
    intmc: u32,
    cc: u32,
    _rsvd: u32,
    csts: u32,
    nssr: u32,
    aqa: u32,
    asq: u64,
    acq: u64,
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct NvmeCmd {
    opcode: u8,
    flags: u8,
    cid: u16,
    nsid: u32,
    _rsvd: u64,
    mptr: u64,
    dptr: [u64; 2],
    cdw10: u32,
    cdw11: u32,
    cdw12: u32,
    cdw13: u32,
    cdw14: u32,
    cdw15: u32,
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct NvmeCqe {
    result: u32,
    _rsvd: u32,
    sq_head: u16,
    sq_id: u16,
    cid: u16,
    status: u16,
}

pub struct NvmeDriver {
    regs: *mut NvmeRegs,
    asq: *mut NvmeCmd,
    acq: *mut NvmeCqe,
    io_sq: *mut NvmeCmd,
    io_cq: *mut NvmeCqe,
    db_stride: usize,
    admin_sq_tail: u16,
    admin_cq_head: u16,
    admin_phase: u16,
    io_sq_tail: u16,
    io_cq_head: u16,
    io_phase: u16,
    dma_buf: u64,
}

impl NvmeDriver {
    pub unsafe fn detect() -> Option<Self> {
        let pci_devs = pci::scan_pci_bus();
        for i in 0..pci_devs.count {
            let dev = pci_devs.devices[i];
            let class_rev = pci::pci_read32(dev.bus, dev.device, dev.function, 0x08);
            let class = (class_rev >> 24) as u8;
            let subclass = (class_rev >> 16) as u8;

            if class == NVME_CLASS && subclass == NVME_SUBCLASS {
                return Some(Self::init(dev));
            }
        }
        None
    }

    unsafe fn init(dev: PciDevice) -> Self {
        // Enable PCI Bus Mastering + Memory Space (required for DMA)
        let cmd_reg = pci::pci_read32(dev.bus, dev.device, dev.function, 0x04);
        pci::pci_write32(dev.bus, dev.device, dev.function, 0x04,
            (cmd_reg & 0xFFFF) | 0x06);

        let bar0 = pci::pci_read32(dev.bus, dev.device, dev.function, 0x10) as u64 & !0xF;
        let regs = bar0 as *mut NvmeRegs;

        // 1. Disable controller
        let mut cc = read_volatile(&(*regs).cc);
        cc &= !1;
        write_volatile(&mut (*regs).cc, cc);
        let mut timeout = 5_000_000u32;
        while (read_volatile(&(*regs).csts) & 1) != 0 && timeout > 0 {
            timeout -= 1;
        }

        // 2. Allocate and zero Admin Queues
        let asq_phys = page_alloc::alloc_pages_contiguous(1).unwrap();
        let acq_phys = page_alloc::alloc_pages_contiguous(1).unwrap();
        core::ptr::write_bytes(asq_phys as *mut u8, 0, 4096);
        core::ptr::write_bytes(acq_phys as *mut u8, 0, 4096);

        write_volatile(&mut (*regs).aqa, (63 << 16) | 63);
        write_volatile(&mut (*regs).asq, asq_phys);
        write_volatile(&mut (*regs).acq, acq_phys);

        // 3. Enable controller
        let cap = read_volatile(&(*regs).cap);
        let dstrd = ((cap >> 32) & 0xF) as usize;
        cc |= 1 | (0 << 7) | (6 << 16) | (4 << 20);
        write_volatile(&mut (*regs).cc, cc);
        timeout = 5_000_000;
        while (read_volatile(&(*regs).csts) & 1) == 0 && timeout > 0 {
            timeout -= 1;
        }

        // 4. Allocate I/O Queues + DMA bounce buffer
        let io_sq_phys = page_alloc::alloc_pages_contiguous(1).unwrap();
        let io_cq_phys = page_alloc::alloc_pages_contiguous(1).unwrap();
        let dma_buf = page_alloc::alloc_pages_contiguous(1).unwrap();
        core::ptr::write_bytes(io_sq_phys as *mut u8, 0, 4096);
        core::ptr::write_bytes(io_cq_phys as *mut u8, 0, 4096);
        core::ptr::write_bytes(dma_buf as *mut u8, 0, 4096);

        let mut driver = Self {
            regs,
            asq: asq_phys as *mut NvmeCmd,
            acq: acq_phys as *mut NvmeCqe,
            io_sq: io_sq_phys as *mut NvmeCmd,
            io_cq: io_cq_phys as *mut NvmeCqe,
            db_stride: 1 << (2 + dstrd),
            admin_sq_tail: 0,
            admin_cq_head: 0,
            admin_phase: 1,
            io_sq_tail: 0,
            io_cq_head: 0,
            io_phase: 1,
            dma_buf,
        };

        driver.create_io_queues(io_sq_phys, io_cq_phys);
        driver
    }

    unsafe fn create_io_queues(&mut self, sq_phys: u64, cq_phys: u64) {
        // Create CQ first
        let mut cmd = core::mem::zeroed::<NvmeCmd>();
        cmd.opcode = 0x05;
        cmd.dptr[0] = cq_phys;
        cmd.cdw10 = (63 << 16) | 1;
        cmd.cdw11 = 1;
        self.submit_admin(&cmd);

        // Create SQ
        let mut cmd = core::mem::zeroed::<NvmeCmd>();
        cmd.opcode = 0x01;
        cmd.dptr[0] = sq_phys;
        cmd.cdw10 = (63 << 16) | 1;
        cmd.cdw11 = (1 << 16) | 1;
        self.submit_admin(&cmd);
    }

    unsafe fn submit_admin(&mut self, cmd: &NvmeCmd) {
        let tail = self.admin_sq_tail as usize;
        self.asq.add(tail).write_volatile(*cmd);
        self.admin_sq_tail = (self.admin_sq_tail + 1) % 64;

        let db = (self.regs as usize + 0x1000) as *mut u32;
        write_volatile(db, self.admin_sq_tail as u32);
        loop {
            let cqe = self.acq.add(self.admin_cq_head as usize).read_volatile();
            if (cqe.status & 1) == self.admin_phase { break; }
            core::hint::spin_loop();
        }
        self.admin_cq_head = (self.admin_cq_head + 1) % 64;
        if self.admin_cq_head == 0 {
            self.admin_phase ^= 1;
        }
        let cq_db = (self.regs as usize + 0x1000 + self.db_stride) as *mut u32;
        write_volatile(cq_db, self.admin_cq_head as u32);
    }

    /// Read sectors via bounce buffer (max 8 sectors = 4096 bytes per command)
    unsafe fn submit_io_read(&mut self, lba: u64, count: u32) -> Result<(), DiskError> {
        let mut cmd = core::mem::zeroed::<NvmeCmd>();
        cmd.opcode = 0x02;
        cmd.nsid = 1;
        cmd.dptr[0] = self.dma_buf;
        cmd.cdw10 = lba as u32;
        cmd.cdw11 = (lba >> 32) as u32;
        cmd.cdw12 = (count - 1) & 0xFFFF;

        let tail = self.io_sq_tail as usize;
        self.io_sq.add(tail).write_volatile(cmd);
        self.io_sq_tail = (self.io_sq_tail + 1) % 64;

        let db = (self.regs as usize + 0x1000 + (2 * self.db_stride)) as *mut u32;
        write_volatile(db, self.io_sq_tail as u32);

        let status;
        loop {
            let cqe = self.io_cq.add(self.io_cq_head as usize).read_volatile();
            if (cqe.status & 1) == self.io_phase {
                status = cqe.status >> 1;
                break;
            }
            core::hint::spin_loop();
        }
        self.io_cq_head = (self.io_cq_head + 1) % 64;
        if self.io_cq_head == 0 {
            self.io_phase ^= 1;
        }

        let cq_db = (self.regs as usize + 0x1000 + (3 * self.db_stride)) as *mut u32;
        write_volatile(cq_db, self.io_cq_head as u32);

        if status == 0 { Ok(()) } else { Err(DiskError::IOError) }
    }

    pub fn read_sectors_raw(&mut self, lba: u64, count: u32, buf_phys: u64) -> Result<(), DiskError> {
        unsafe {
            let mut current_lba = lba;
            let mut remaining = count;
            let mut offset: usize = 0;
            while remaining > 0 {
                let chunk = core::cmp::min(remaining, 8);
                self.submit_io_read(current_lba, chunk)?;
                let bytes = (chunk as usize) * 512;
                core::ptr::copy_nonoverlapping(
                    self.dma_buf as *const u8,
                    (buf_phys as usize + offset) as *mut u8,
                    bytes,
                );
                current_lba += chunk as u64;
                remaining -= chunk;
                offset += bytes;
            }
            Ok(())
        }
    }
}

impl DiskReader for NvmeDriver {
    fn read_sectors(&mut self, lba: u64, count: u32, buf: &mut [u8]) -> Result<(), DiskError> {
        let mut current_lba = lba;
        let mut remaining = count;
        let mut offset: usize = 0;
        while remaining > 0 {
            let chunk = core::cmp::min(remaining, 8);
            unsafe {
                self.submit_io_read(current_lba, chunk)?;
                let bytes = (chunk as usize) * 512;
                let src = core::slice::from_raw_parts(self.dma_buf as *const u8, bytes);
                buf[offset..offset + bytes].copy_from_slice(src);
            }
            current_lba += chunk as u64;
            remaining -= chunk;
            offset += (chunk as usize) * 512;
        }
        Ok(())
    }
}
