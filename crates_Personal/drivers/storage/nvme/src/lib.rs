//! BMO NVMe Driver — PCIe NVMe SSD controller.
//!
//! Minimal driver: admin queue init → identify namespace → create IO queue
//! → read/write. Uses PRP for data transfers. Single IO queue pair.
//!
//! ## Usage
//! ```ignore
//! let mut ctrl = Nvme::probe(mmiobar, &backend);
//! let size = ctrl.namespace_size();  // in 512-byte sectors
//! ctrl.read(lba, sectors, buf_ptr);
//! ctrl.write(lba, sectors, buf_ptr);
//! ```

#![no_std]

use core::ptr;

/// NVMe controller registers (BAR0 offsets).
struct Regs(*mut u32);

impl Regs {
    #[inline] unsafe fn read(&self, off: usize) -> u32 {
        (self.0.add(off / 4)).read_volatile()
    }
    #[inline] unsafe fn write(&self, off: usize, val: u32) {
        (self.0.add(off / 4)).write_volatile(val);
    }
    #[inline] unsafe fn read64(&self, off: usize) -> u64 {
        let lo = self.read(off) as u64;
        let hi = self.read(off + 4) as u64;
        lo | (hi << 32)
    }
    #[inline] unsafe fn write64(&self, off: usize, val: u64) {
        self.write(off, val as u32);
        self.write(off + 4, (val >> 32) as u32);
    }
}

// Register offsets
const CAP:     usize = 0x00;
#[allow(dead_code)] const VS:      usize = 0x08;
const CC:      usize = 0x14;
const CSTS:    usize = 0x1C;
const AQA:     usize = 0x24;
const ASQ:     usize = 0x28;
const ACQ:     usize = 0x30;

fn sq0tdbl(_cap: u64) -> usize { 0x1000 }
fn cq0hdbl(_cap: u64) -> usize { 0x1000 + 4 }

// CAP register fields
fn cap_mqes(cap: u64) -> u16 { (cap & 0xFFFF) as u16 }
fn cap_dstrd(cap: u64) -> u8 { (((cap >> 32) & 0xF) as u8).max(0) }
fn cap_css_nvm(cap: u64) -> bool { (cap >> 37) & 1 == 1 }

// CSTS flags
const CSTS_RDY: u32 = 1;

// CC flags
const CC_EN:  u32 = 1;
const CC_IOCQES_SHIFT: u32 = 20;
const CC_IOSQES_SHIFT: u32 = 16;

// Admin commands
#[allow(dead_code)] const CMD_DELETE_IO_SQ: u8 = 0x00;
const CMD_CREATE_IO_SQ: u8 = 0x01;
#[allow(dead_code)] const CMD_DELETE_IO_CQ: u8 = 0x04;
const CMD_CREATE_IO_CQ: u8 = 0x05;
const CMD_IDENTIFY:     u8 = 0x06;

// NVM commands
const CMD_WRITE: u8 = 0x01;
const CMD_READ:  u8 = 0x02;

// Queue sizes
const ADMIN_QUEUE_SIZE: u16 = 64;
const IO_QUEUE_SIZE:    u16 = 256;
const PAGE_SIZE: usize = 4096;

/// Backend for physical memory allocation (provided by caller).
pub trait NvmeBackend {
    fn alloc_dma(&self, pages: usize) -> Option<u64>;
    fn free_dma(&self, phys: u64, pages: usize);
    fn phys_to_virt(&self, phys: u64) -> *mut u8;
    fn wait_ms(&self, ms: u64);
}

/// Submission Queue Entry (64 bytes at 16-byte aligned offset).
#[repr(C, align(64))]
struct SqEntry {
    cdw0: u32,      // 0x00: opcode (bits 7:0), fuse (9:8), psdt (11:10), cid (31:16)
    nsid: u32,      // 0x04
    _rsvd1: [u8; 8],   // 0x08-0x0F
    mptr: u64,      // 0x10
    prp1: u64,      // 0x18
    prp2: u64,      // 0x20
    cdw10: u32,     // 0x28
    cdw11: u32,     // 0x2C
    cdw12: u32,     // 0x30
    cdw13: u32,     // 0x34
    cdw14: u32,     // 0x38
    cdw15: u32,     // 0x3C
}

/// Completion Queue Entry (16 bytes).
#[repr(C)]
struct CqEntry {
    cdw0: u32,
    _rsvd1: u32,
    sq_head: u16,
    sq_id: u16,
    cid: u16,
    sf: u16,         // status field: phase bit (15) + status code (14:0)
}

/// Admin completion queue (dequeued from CQ after command completes).
pub struct AdminCompletion {
    pub status: u16,
}

/// Namespace info from Identify command (simplified).
pub struct NamespaceInfo {
    pub block_count: u64,
    pub block_size: u32,
}

/// NVMe controller state.
pub struct Nvme {
    mmio: Regs,
    cap: u64,
    /// Virtual addr of admin SQ
    admin_sq: *mut SqEntry,
    /// Virtual addr of admin CQ
    admin_cq: *mut CqEntry,
    /// Physical addr of admin SQ
    #[allow(dead_code)] admin_sq_phys: u64,
    /// Physical addr of admin CQ
    #[allow(dead_code)] admin_cq_phys: u64,
    /// Admin SQ tail index
    admin_sq_tail: u16,
    /// Admin CQ head index
    admin_cq_head: u16,
    /// Admin CQ phase
    admin_cq_phase: u8,
    /// Command ID counter
    cid_counter: u16,
    /// IO SQ virt
    io_sq: *mut SqEntry,
    /// IO CQ virt
    io_cq: *mut CqEntry,
    io_sq_phys: u64,
    io_cq_phys: u64,
    io_sq_tail: u16,
    io_cq_head: u16,
    io_cq_phase: u8,
    /// Namespace info
    ns_block_count: u64,
    ns_block_size: u32,
    nsid: u32,
}

impl Nvme {
    /// Probe and initialize an NVMe controller at the given MMIO BAR address.
    pub fn probe(mmiobar: u64, backend: &impl NvmeBackend) -> Option<Self> {
        let mmio = Regs(mmiobar as *mut u32);
        let cap = unsafe { mmio.read64(CAP) };

        // Check NVM command set
        if !cap_css_nvm(cap) {
            return None;
        }

        let mqes = cap_mqes(cap);
        let sq_size = if mqes > ADMIN_QUEUE_SIZE { ADMIN_QUEUE_SIZE } else { mqes };

        // Disable controller
        unsafe { mmio.write(CC, 0); }
        backend.wait_ms(10);

        // Wait for CSTS.RDY = 0
        for _ in 0..100 {
            if unsafe { mmio.read(CSTS) } & CSTS_RDY == 0 { break; }
            backend.wait_ms(10);
        }

        // Allocate admin queues (1 page each)
        let sq_phys = backend.alloc_dma(1)?;
        let cq_phys = backend.alloc_dma(1)?;
        let sq_virt = backend.phys_to_virt(sq_phys) as *mut SqEntry;
        let cq_virt = backend.phys_to_virt(cq_phys) as *mut CqEntry;
        unsafe { ptr::write_bytes(sq_virt as *mut u8, 0, PAGE_SIZE); }
        unsafe { ptr::write_bytes(cq_virt as *mut u8, 0, PAGE_SIZE); }

        // Configure admin queues
        unsafe {
            mmio.write(AQA, ((sq_size - 1) as u32) | (((sq_size - 1) as u32) << 16));
            mmio.write64(ASQ, sq_phys);
            mmio.write64(ACQ, cq_phys);
        }

        // Enable controller
        let cc_val = CC_EN
            | (4u32 << CC_IOCQES_SHIFT)   // CQ entry size = 2^4 = 16
            | (6u32 << CC_IOSQES_SHIFT);   // SQ entry size = 2^6 = 64
        unsafe { mmio.write(CC, cc_val); }

        // Wait for CSTS.RDY = 1
        for _ in 0..1000 {
            if unsafe { mmio.read(CSTS) } & CSTS_RDY != 0 { break; }
            backend.wait_ms(1);
        }

        let mut ctrl = Self {
            mmio,
            cap,
            admin_sq: sq_virt,
            admin_cq: cq_virt,
            admin_sq_phys: sq_phys,
            admin_cq_phys: cq_phys,
            admin_sq_tail: 0,
            admin_cq_head: 0,
            admin_cq_phase: 1,
            cid_counter: 0,
            io_sq: ptr::null_mut(),
            io_cq: ptr::null_mut(),
            io_sq_phys: 0,
            io_cq_phys: 0,
            io_sq_tail: 0,
            io_cq_head: 0,
            io_cq_phase: 1,
            ns_block_count: 0,
            ns_block_size: 512,
            nsid: 1,
        };

        // Identify namespace
        let id_buf_phys = backend.alloc_dma(1)?;
        let id_buf = backend.phys_to_virt(id_buf_phys);
        ctrl.admin_cmd(CMD_IDENTIFY, 1,
            id_buf_phys, 0, // CNS=0, NSID=1
            0x0000_0001, 0, 0, 0, 0, 0);

        unsafe {
            let block_count = ptr::read_volatile(id_buf.add(0) as *const u64);
            // Get block size from LBAF in Identify response
            let flbas: u8 = ptr::read_volatile(id_buf.add(26));
            let lbaf_idx = (flbas & 0xF) as usize;
            if lbaf_idx < 16 {
                let lbaf_off = 128 + lbaf_idx * 4;
                let lbaf_data = ptr::read_volatile(id_buf.add(lbaf_off) as *const u32);
                ctrl.ns_block_size = if lbaf_data & 0xFFFF == 0 { 512 } else { lbaf_data & 0xFFFF };
            }
            ctrl.ns_block_count = block_count;
        }
        backend.free_dma(id_buf_phys, 1);

        // Create IO submission queue
        let io_sq_phys = backend.alloc_dma(1)?;
        let io_sq = backend.phys_to_virt(io_sq_phys) as *mut SqEntry;
        unsafe { ptr::write_bytes(io_sq as *mut u8, 0, PAGE_SIZE); }
        ctrl.admin_cmd(CMD_CREATE_IO_SQ, 0,
            io_sq_phys, 0,
            1 | (((IO_QUEUE_SIZE - 1) as u32) << 16) | (1 << 24), 0, 0, 0, 0, 0);
        ctrl.io_sq = io_sq;
        ctrl.io_sq_phys = io_sq_phys;

        // Create IO completion queue
        let io_cq_phys = backend.alloc_dma(1)?;
        let io_cq = backend.phys_to_virt(io_cq_phys) as *mut CqEntry;
        unsafe { ptr::write_bytes(io_cq as *mut u8, 0, PAGE_SIZE); }
        ctrl.admin_cmd(CMD_CREATE_IO_CQ, 0,
            io_cq_phys, 0,
            1 | (((IO_QUEUE_SIZE - 1) as u32) << 16) | (1 << 24), 0, 0, 0, 0, 0);
        ctrl.io_cq = io_cq;
        ctrl.io_cq_phys = io_cq_phys;

        Some(ctrl)
    }

    /// Total blocks in the namespace.
    pub fn block_count(&self) -> u64 { self.ns_block_count }

    /// Block size in bytes (usually 512).
    pub fn block_size(&self) -> u32 { self.ns_block_size }

    /// Read sectors from LBA.
    pub fn read(&mut self, lba: u64, sectors: u16, buf: *mut u8, backend: &impl NvmeBackend) -> bool {
        let buf_phys = match backend.alloc_dma((sectors as usize * 512 + 4095) / 4096) {
            Some(p) => p,
            None => return false,
        };
        self.io_cmd(CMD_READ, lba, sectors, buf_phys);
        // Copy from DMA buffer to caller buf
        let src = backend.phys_to_virt(buf_phys);
        let len = sectors as usize * 512;
        unsafe { ptr::copy_nonoverlapping(src, buf, len); }
        backend.free_dma(buf_phys, (sectors as usize * 512 + 4095) / 4096);
        true
    }

    /// Write sectors to LBA.
    pub fn write(&mut self, lba: u64, sectors: u16, buf: *const u8, backend: &impl NvmeBackend) -> bool {
        let buf_phys = match backend.alloc_dma((sectors as usize * 512 + 4095) / 4096) {
            Some(p) => p,
            None => return false,
        };
        let dst = backend.phys_to_virt(buf_phys);
        let len = sectors as usize * 512;
        unsafe { ptr::copy_nonoverlapping(buf, dst, len); }
        self.io_cmd(CMD_WRITE, lba, sectors, buf_phys);
        backend.free_dma(buf_phys, (sectors as usize * 512 + 4095) / 4096);
        true
    }

    fn next_cid(&mut self) -> u16 {
        let cid = self.cid_counter;
        self.cid_counter = self.cid_counter.wrapping_add(1);
        cid
    }

    fn admin_cmd(&mut self, opc: u8, nsid: u32, prp1: u64, prp2: u64,
                  cdw10: u32, cdw11: u32, cdw12: u32, cdw13: u32, cdw14: u32, cdw15: u32,
    ) -> AdminCompletion {
        let cid = self.next_cid();
        let tail = self.admin_sq_tail as usize;
        unsafe {
            let entry = &mut *self.admin_sq.add(tail);
            ptr::write_bytes(entry as *mut _ as *mut u8, 0, 64);
            entry.cdw0 = (opc as u32) | ((cid as u32) << 16);
            entry.nsid = nsid;
            entry.prp1 = prp1;
            entry.prp2 = prp2;
            entry.cdw10 = cdw10;
            entry.cdw11 = cdw11;
            entry.cdw12 = cdw12;
            entry.cdw13 = cdw13;
            entry.cdw14 = cdw14;
            entry.cdw15 = cdw15;
        }
        self.admin_sq_tail = (tail as u16 + 1) % ADMIN_QUEUE_SIZE;
        unsafe { self.mmio.write(sq0tdbl(self.cap), self.admin_sq_tail as u32); }

        // Wait for completion
        loop {
            unsafe {
                let cqe = &*self.admin_cq.add(self.admin_cq_head as usize);
                let phase = (cqe.sf >> 15) as u8;
                if phase == self.admin_cq_phase {
                    let status = cqe.sf & 0x7FFF;
                    self.admin_cq_head = (self.admin_cq_head + 1) % ADMIN_QUEUE_SIZE;
                    if self.admin_cq_head == 0 {
                        self.admin_cq_phase ^= 1;
                    }
                    self.mmio.write(cq0hdbl(self.cap), self.admin_cq_head as u32);
                    return AdminCompletion { status };
                }
            }
            core::hint::spin_loop();
        }
    }

    fn io_cmd(&mut self, opc: u8, lba: u64, sectors: u16, prp1: u64) {
        let cid = self.next_cid();
        let tail = self.io_sq_tail as usize;
        unsafe {
            let entry = &mut *self.io_sq.add(tail);
            ptr::write_bytes(entry as *mut _ as *mut u8, 0, 64);
            entry.cdw0 = (opc as u32) | ((cid as u32) << 16);
            entry.nsid = self.nsid;
            entry.prp1 = prp1;
            entry.cdw10 = lba as u32;
            entry.cdw11 = (lba >> 32) as u32;
            entry.cdw12 = sectors as u32;
        }
        self.io_sq_tail = (tail as u16 + 1) % IO_QUEUE_SIZE;
        // IO SQ1 doorbell at 0x1000 + 2*1*(4<<DSTRD)
        let dstrd = cap_dstrd(self.cap);
        let stride = (4u32 << dstrd) as usize;
        let sq1_doorbell = 0x1000 + 2 * 1 * stride;
        unsafe { self.mmio.write(sq1_doorbell, self.io_sq_tail as u32); }

        // Wait for IO completion
        let mut waited = 0;
        loop {
            unsafe {
                let cqe = &*self.io_cq.add(self.io_cq_head as usize);
                let phase = (cqe.sf >> 15) as u8;
                if phase == self.io_cq_phase {
                    self.io_cq_head = (self.io_cq_head + 1) % IO_QUEUE_SIZE;
                    if self.io_cq_head == 0 { self.io_cq_phase ^= 1; }
                    // IO CQ1 doorbell
                    let cq1_doorbell = 0x1000 + (2 * 1 + 1) * stride;
                    self.mmio.write(cq1_doorbell, self.io_cq_head as u32);
                    return;
                }
            }
            waited += 1;
            if waited > 1_000_000 { return; }
            core::hint::spin_loop();
        }
    }
}
