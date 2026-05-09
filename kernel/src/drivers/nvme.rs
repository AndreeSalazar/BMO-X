//! Minimal Read-Only NVMe Driver for FastOS.
//! Implementación bare-metal (~150 líneas) sin crates externos.

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
    _rsvd: [u64; 2],
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
    sq_tail: u16,
    cq_head: u16,
}

impl NvmeDriver {
    pub unsafe fn detect() -> Option<Self> {
        let pci_devs = pci::scan_pci_bus();
        for i in 0..pci_devs.count {
            let dev = pci_devs.devices[i];
            // NVMe class/subclass check (read from config space if not in PciDevice struct)
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
        let bar0 = pci::pci_read32(dev.bus, dev.device, dev.function, 0x10) as u64 & !0xF;
        let regs = bar0 as *mut NvmeRegs;
        
        // 1. Disable controller
        let mut cc = read_volatile(&(*regs).cc);
        cc &= !1;
        write_volatile(&mut (*regs).cc, cc);
        while (read_volatile(&(*regs).csts) & 1) != 0 {}

        // 2. Setup Admin Queues
        let asq_phys = page_alloc::alloc_pages_contiguous(1).unwrap();
        let acq_phys = page_alloc::alloc_pages_contiguous(1).unwrap();
        write_volatile(&mut (*regs).aqa, (63 << 16) | 63); // 64 entries each
        write_volatile(&mut (*regs).asq, asq_phys);
        write_volatile(&mut (*regs).acq, acq_phys);

        // 3. Enable controller
        let cap = read_volatile(&(*regs).cap);
        let dstrd = ((cap >> 32) & 0xF) as usize;
        cc |= 1 | (0 << 7) | (6 << 16) | (4 << 20); // EN, CSS=NVM, AMS=RR, MPS=4KB
        write_volatile(&mut (*regs).cc, cc);
        while (read_volatile(&(*regs).csts) & 1) == 0 {}

        // 4. Create I/O Queues (simplified: reusing logic for minimal driver)
        let io_sq_phys = page_alloc::alloc_pages_contiguous(1).unwrap();
        let io_cq_phys = page_alloc::alloc_pages_contiguous(1).unwrap();

        let mut driver = Self {
            regs,
            asq: asq_phys as *mut NvmeCmd,
            acq: acq_phys as *mut NvmeCqe,
            io_sq: io_sq_phys as *mut NvmeCmd,
            io_cq: io_cq_phys as *mut NvmeCqe,
            db_stride: 1 << (2 + dstrd),
            sq_tail: 0,
            cq_head: 0,
        };

        driver.create_io_queues(io_sq_phys, io_cq_phys);
        driver
    }

    unsafe fn create_io_queues(&mut self, sq_phys: u64, cq_phys: u64) {
        // Create CQ
        let mut cmd = core::mem::zeroed::<NvmeCmd>();
        cmd.opcode = 0x05; // Create IO Completion Queue
        cmd.dptr[0] = cq_phys;
        cmd.cdw10 = (63 << 16) | 1; // Size 64, ID 1
        cmd.cdw11 = 1; // Physically contiguous
        self.submit_admin(&cmd);

        // Create SQ
        let mut cmd = core::mem::zeroed::<NvmeCmd>();
        cmd.opcode = 0x01; // Create IO Submission Queue
        cmd.dptr[0] = sq_phys;
        cmd.cdw10 = (63 << 16) | 1; // Size 64, ID 1
        cmd.cdw11 = (1 << 16) | 1; // CQID 1, Physically contiguous
        self.submit_admin(&cmd);
    }

    unsafe fn submit_admin(&mut self, cmd: &NvmeCmd) {
        let tail = self.sq_tail as usize;
        self.asq.add(tail).write_volatile(*cmd);
        self.sq_tail = (self.sq_tail + 1) % 64;
        let db = (self.regs as usize + 0x1000) as *mut u32;
        write_volatile(db, self.sq_tail as u32);
        loop {
            let cqe = self.acq.add(self.cq_head as usize).read_volatile();
            if cqe.status != 0 { break; }
        }
        self.cq_head = (self.cq_head + 1) % 64;
    }

    pub fn read_sectors_raw(&mut self, lba: u64, count: u32, buf_phys: u64) -> Result<(), DiskError> {
        unsafe {
            let mut cmd = core::mem::zeroed::<NvmeCmd>();
            cmd.opcode = 0x02; // Read
            cmd.nsid = 1;
            cmd.dptr[0] = buf_phys;
            cmd.cdw10 = lba as u32;
            cmd.cdw11 = (lba >> 32) as u32;
            cmd.cdw12 = (count - 1) & 0xFFFF; // NLB (0-based)

            let tail = self.sq_tail as usize;
            self.io_sq.add(tail).write_volatile(cmd);
            self.sq_tail = (self.sq_tail + 1) % 64;
            
            let db = (self.regs as usize + 0x1000 + (2 * self.db_stride)) as *mut u32;
            write_volatile(db, self.sq_tail as u32);

            let mut status;
            loop {
                let cqe = self.io_cq.add(self.cq_head as usize).read_volatile();
                status = cqe.status >> 1;
                if status != 0 { break; }
            }
            self.cq_head = (self.cq_head + 1) % 64;
            
            if status == 0 { Ok(()) } else { Err(DiskError::IOError) }
        }
    }
}

impl DiskReader for NvmeDriver {
    fn read_sectors(&mut self, lba: u64, count: u32, buf: &mut [u8]) -> Result<(), DiskError> {
        // En una implementación real, buf debería ser una dirección física válida.
        // Para FastOS (identidad mapeada), buf.as_ptr() es la dirección física.
        self.read_sectors_raw(lba, count, buf.as_ptr() as u64)
    }
}
