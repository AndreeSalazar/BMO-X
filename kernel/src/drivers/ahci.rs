#![allow(dead_code)]

//! Bare-metal AHCI/SATA Driver for FastOS.
//! Implements basic port initialization and Write DMA Ext for data export.

use crate::drivers::pci::{self, PciDevice};
use crate::fs::{DiskReader, DiskWriter, DiskError};
use crate::arch::page_alloc;
use core::ptr::{read_volatile, write_volatile};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AhciRuntimeMode {
    DryRun,
    Active,
}

const SATA_SIG_ATA: u32 = 0x00000101;
const AHCI_CLASS: u8 = 0x01;
const AHCI_SUBCLASS: u8 = 0x06;

#[repr(C)]
struct AhciPortRegs {
    clb: u32,       // 0x00, command list base address, 1K-byte aligned
    clbu: u32,      // 0x04, command list base address upper 32 bits
    fb: u32,        // 0x08, FIS base address, 256-byte aligned
    fbu: u32,       // 0x0C, FIS base address upper 32 bits
    is: u32,        // 0x10, interrupt status
    ie: u32,        // 0x14, interrupt enable
    cmd: u32,       // 0x18, command and status
    rsv0: u32,      // 0x1C, Reserved
    tfd: u32,       // 0x20, task file data
    sig: u32,       // 0x24, signature
    ssts: u32,      // 0x28, SATA status (SCR0:SStatus)
    sctl: u32,      // 0x2C, SATA control (SCR2:SControl)
    serr: u32,      // 0x30, SATA error (SCR1:SError)
    sact: u32,      // 0x34, SATA active (SCR3:SActive)
    ci: u32,        // 0x38, command issue
    sntf: u32,      // 0x3C, SATA notification (SCR4:SNotification)
    fbs: u32,       // 0x40, FIS-based switch control
    rsv1: [u32; 11], // 0x44 ~ 0x6F, Reserved
    vendor: [u32; 4], // 0x70 ~ 0x7F, vendor specific
}

#[repr(C)]
struct AhciHbaMem {
    cap: u32,       // 0x00, Host capability
    ghc: u32,       // 0x04, Global host control
    is: u32,        // 0x08, Interrupt status
    pi: u32,        // 0x0C, Ports implemented
    vs: u32,        // 0x10, Version
    ccc_ctl: u32,   // 0x14, Command completion coalescing control
    ccc_pts: u32,   // 0x18, Command completion coalescing ports
    em_loc: u32,    // 0x1C, Enclosure management location
    em_ctl: u32,    // 0x20, Enclosure management control
    cap2: u32,      // 0x24, Host capabilities extended
    bohc: u32,      // 0x28, BIOS/OS handoff control and status
    rsv: [u8; 0xA0 - 0x2C],
    vendor: [u8; 0x100 - 0xA0],
    ports: [AhciPortRegs; 32], // 0x100 ~ 0x10FF
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct AhciCmdHeader {
    opts: u16,      // Command FIS length, ATAPI, Write, Prefetchable
    prdtl: u16,     // Physical region descriptor table length
    prdbc: u32,     // Physical region descriptor byte count transferred
    ctba: u32,      // Command table descriptor base address
    ctbau: u32,     // Command table descriptor base address upper 32 bits
    rsv1: [u32; 4], // Reserved
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct AhciPrdtEntry {
    dba: u32,       // Data base address
    dbau: u32,      // Data base address upper 32 bits
    rsv0: u32,      // Reserved
    dbc: u32,       // Byte count, 4M max, interrupt = 1
}

#[repr(C, packed)]
struct AhciCmdTable {
    cfis: [u8; 64], // Command FIS
    acmd: [u8; 16], // ATAPI command
    rsv: [u8; 48],  // Reserved
    prdt: [AhciPrdtEntry; 1], // PRDT entries (we only need 1 for bounce buffer)
}

pub struct AhciDriver {
    hba: *mut AhciHbaMem,
    port: *mut AhciPortRegs,
    port_idx: usize,
    cmd_list: *mut AhciCmdHeader,
    cmd_table: *mut AhciCmdTable,
    dma_buf: u64,
    pub mode: AhciRuntimeMode,
    pub export_bounds: Option<(u64, u64)>, // (start_lba, end_lba)
}

impl AhciDriver {
    pub unsafe fn detect() -> Option<Self> {
        let pci_devs = pci::scan_pci_bus();
        for i in 0..pci_devs.count {
            let dev = pci_devs.devices[i];
            let class_rev = pci::pci_read32(dev.bus, dev.device, dev.function, 0x08);
            let class = (class_rev >> 24) as u8;
            let subclass = (class_rev >> 16) as u8;

            if class == AHCI_CLASS && subclass == AHCI_SUBCLASS {
                return Some(Self::init(dev));
            }
        }
        None
    }

    /// Dump diagnostic registers to Console for debugging DMA failures.
    pub unsafe fn diagnose(&self, con: &mut crate::ui::console::Console) {
        con.println("[AHCI-DIAG] === HBA Register Dump ===");

        let cap  = read_volatile(&(*self.hba).cap);
        let ghc  = read_volatile(&(*self.hba).ghc);
        let pi   = read_volatile(&(*self.hba).pi);
        let vs   = read_volatile(&(*self.hba).vs);
        let cap2 = read_volatile(&(*self.hba).cap2);
        let bohc = read_volatile(&(*self.hba).bohc);

        con.print("  CAP:  0x"); con.print_hex32(cap); con.println("");
        con.print("  GHC:  0x"); con.print_hex32(ghc); con.println("");
        con.print("  PI:   0x"); con.print_hex32(pi); con.println("");
        con.print("  VS:   0x"); con.print_hex32(vs);
        con.print(" (AHCI "); con.print_u64(((vs >> 16) & 0xFFFF) as u64);
        con.print("."); con.print_u64((vs & 0xFFFF) as u64); con.println(")");
        con.print("  CAP2: 0x"); con.print_hex32(cap2); con.println("");
        con.print("  BOHC: 0x"); con.print_hex32(bohc); con.println("");

        // Decode CAP fields
        let s64a = (cap >> 31) & 1; // Supports 64-bit Addressing
        let np   = (cap & 0x1F) + 1; // Number of Ports
        let ncs  = ((cap >> 8) & 0x1F) + 1; // Number of Command Slots
        con.print("  CAP.S64A (64-bit DMA): "); con.print_u64(s64a as u64); con.println("");
        con.print("  CAP.NP (ports):        "); con.print_u64(np as u64); con.println("");
        con.print("  CAP.NCS (cmd slots):   "); con.print_u64(ncs as u64); con.println("");

        con.print("[AHCI-DIAG] Port "); con.print_u64(self.port_idx as u64); con.println(" Registers:");

        let cmd  = read_volatile(&(*self.port).cmd);
        let tfd  = read_volatile(&(*self.port).tfd);
        let ssts = read_volatile(&(*self.port).ssts);
        let _sctl = read_volatile(&(*self.port).sctl);
        let serr = read_volatile(&(*self.port).serr);
        let ci   = read_volatile(&(*self.port).ci);
        let is   = read_volatile(&(*self.port).is);
        let sig  = read_volatile(&(*self.port).sig);
        let clb  = read_volatile(&(*self.port).clb);
        let clbu = read_volatile(&(*self.port).clbu);
        let fb   = read_volatile(&(*self.port).fb);
        let fbu  = read_volatile(&(*self.port).fbu);

        con.print("  CMD:  0x"); con.print_hex32(cmd);
        // Decode CMD bits
        con.print(" [ST="); con.print_u64((cmd & 1) as u64);
        con.print(" FRE="); con.print_u64(((cmd >> 4) & 1) as u64);
        con.print(" FR="); con.print_u64(((cmd >> 14) & 1) as u64);
        con.print(" CR="); con.print_u64(((cmd >> 15) & 1) as u64);
        con.println("]");

        con.print("  TFD:  0x"); con.print_hex32(tfd);
        let ata_status = (tfd & 0xFF) as u8;
        let ata_error  = ((tfd >> 8) & 0xFF) as u8;
        con.print(" [STS=0x"); con.print_hex32(ata_status as u32);
        con.print(" ERR=0x"); con.print_hex32(ata_error as u32);
        con.print("]");
        if ata_status & 0x01 != 0 { con.print(" ERR!"); }
        if ata_status & 0x08 != 0 { con.print(" DRQ"); }
        if ata_status & 0x40 != 0 { con.print(" DRDY"); }
        if ata_status & 0x80 != 0 { con.print(" BSY"); }
        con.println("");

        con.print("  SSTS: 0x"); con.print_hex32(ssts);
        let det = ssts & 0x0F;
        let spd = (ssts >> 4) & 0x0F;
        con.print(" [DET="); con.print_u64(det as u64);
        con.print(" SPD="); con.print_u64(spd as u64); con.println("]");

        con.print("  SERR: 0x"); con.print_hex32(serr); con.println("");
        con.print("  IS:   0x"); con.print_hex32(is); con.println("");
        con.print("  CI:   0x"); con.print_hex32(ci); con.println("");
        con.print("  SIG:  0x"); con.print_hex32(sig); con.println("");
        con.print("  CLB:  0x"); con.print_hex32(clbu); con.print_hex32(clb); con.println("");
        con.print("  FB:   0x"); con.print_hex32(fbu); con.print_hex32(fb); con.println("");
        con.print("  DMA buf: 0x"); con.print_hex32((self.dma_buf >> 32) as u32); con.print_hex32(self.dma_buf as u32); con.println("");

        // Check common failure conditions
        if s64a == 0 && (clbu != 0 || fbu != 0 || (self.dma_buf >> 32) != 0) {
            con.println("  *** PROBLEM: HBA does NOT support 64-bit DMA, but addresses are above 4GB! ***");
        }
        if det != 3 {
            con.println("  *** PROBLEM: SSTS.DET != 3 -- no device communication established ***");
        }
        if ata_status & 0x01 != 0 {
            con.println("  *** PROBLEM: ATA ERR bit set -- check TFD.ERR for error code ***");
        }
        if ata_status & 0x80 != 0 {
            con.println("  *** PROBLEM: ATA BSY bit set -- device is busy/hung ***");
        }
        if serr != 0 {
            con.println("  *** PROBLEM: SERR non-zero -- SATA link errors detected ***");
        }

        con.println("[AHCI-DIAG] === End ===");
    }

    unsafe fn init(dev: PciDevice) -> Self {
        // Enable PCI Bus Mastering + Memory Space (required for DMA)
        let cmd_reg = pci::pci_read32(dev.bus, dev.device, dev.function, 0x04);
        pci::pci_write32(dev.bus, dev.device, dev.function, 0x04, (cmd_reg & 0xFFFF) | 0x06);

        // AHCI Base Address Register (ABAR) is BAR5 (offset 0x24)
        let abar = pci::pci_read32(dev.bus, dev.device, dev.function, 0x24) as u64 & !0xF;
        let hba = abar as *mut AhciHbaMem;

        // Enable AHCI (GHC.AE = 1)
        let mut ghc = read_volatile(&(*hba).ghc);
        ghc |= 1 << 31; 
        write_volatile(&mut (*hba).ghc, ghc);

        // Clear any pending SERR before we start
        let pi = read_volatile(&(*hba).pi);

        // Find active SATA port
        let mut target_port_idx = 0;
        for i in 0..32 {
            if (pi & (1 << i)) != 0 {
                let ssts = read_volatile(&(*hba).ports[i].ssts);
                let sig = read_volatile(&(*hba).ports[i].sig);
                if (ssts & 0x0F) == 3 && sig == SATA_SIG_ATA {
                    target_port_idx = i;
                    break;
                }
            }
        }

        let port = &mut (*hba).ports[target_port_idx] as *mut AhciPortRegs;

        // Clear SERR (write 1s to clear)
        write_volatile(&mut (*port).serr, 0xFFFFFFFF);
        // Clear IS (write 1s to clear)
        write_volatile(&mut (*port).is, 0xFFFFFFFF);

        // Stop port command engine
        let mut cmd = read_volatile(&(*port).cmd);
        cmd &= !1; // ST = 0
        cmd &= !(1 << 4); // FRE = 0
        write_volatile(&mut (*port).cmd, cmd);

        // Allocate memory (Command List, FIS, Command Table, DMA Bounce Buffer)
        let clb_phys = page_alloc::alloc_pages_contiguous(1).unwrap();
        let fb_phys = page_alloc::alloc_pages_contiguous(1).unwrap();
        let ct_phys = page_alloc::alloc_pages_contiguous(1).unwrap();
        let dma_buf = page_alloc::alloc_pages_contiguous(1).unwrap();

        core::ptr::write_bytes(clb_phys as *mut u8, 0, 4096);
        core::ptr::write_bytes(fb_phys as *mut u8, 0, 4096);
        core::ptr::write_bytes(ct_phys as *mut u8, 0, 4096);
        core::ptr::write_bytes(dma_buf as *mut u8, 0, 4096);

        write_volatile(&mut (*port).clb, clb_phys as u32);
        write_volatile(&mut (*port).clbu, (clb_phys >> 32) as u32);
        write_volatile(&mut (*port).fb, fb_phys as u32);
        write_volatile(&mut (*port).fbu, (fb_phys >> 32) as u32);

        // Setup Command Header 0 to point to Command Table
        let cmd_list = clb_phys as *mut AhciCmdHeader;
        let mut hdr = read_volatile(cmd_list);
        hdr.prdtl = 1; // 1 PRDT entry
        hdr.ctba = ct_phys as u32;
        hdr.ctbau = (ct_phys >> 32) as u32;
        write_volatile(cmd_list, hdr);

        let cmd_table = ct_phys as *mut AhciCmdTable;

        // Start port command engine
        while (read_volatile(&(*port).cmd) & (1 << 15)) != 0 { core::hint::spin_loop(); } // wait CR = 0
        while (read_volatile(&(*port).cmd) & (1 << 14)) != 0 { core::hint::spin_loop(); } // wait FR = 0

        cmd = read_volatile(&(*port).cmd);
        cmd |= 1 << 4; // FRE = 1
        cmd |= 1;      // ST = 1
        write_volatile(&mut (*port).cmd, cmd);

        Self {
            hba,
            port,
            port_idx: target_port_idx,
            cmd_list,
            cmd_table,
            dma_buf,
            mode: AhciRuntimeMode::DryRun, // Default to DryRun for safety
            export_bounds: None,
        }
    }

    unsafe fn submit_io_cmd(&mut self, lba: u64, count: u32, is_write: bool) -> Result<(), DiskError> {
        if is_write && self.mode == AhciRuntimeMode::DryRun {
            return Ok(());
        }

        // Clear any stale error state
        write_volatile(&mut (*self.port).serr, 0xFFFFFFFF);
        let is = read_volatile(&(*self.port).is);
        write_volatile(&mut (*self.port).is, is); // clear interrupts

        // Wait for port to be not-busy before issuing
        let mut wait = 1_000_000u32;
        loop {
            let tfd = read_volatile(&(*self.port).tfd);
            if (tfd & 0x88) == 0 { break; } // BSY=0 and DRQ=0
            wait -= 1;
            if wait == 0 {
                return Err(DiskError::Timeout);
            }
            core::hint::spin_loop();
        }

        // Prepare Command Header (FIS length 5 dwords)
        let mut hdr = read_volatile(self.cmd_list);
        hdr.opts = 5; // FIS length in dwords
        if is_write { hdr.opts |= 1 << 6; } // Write flag
        hdr.prdtl = 1; // Ensure PRDT length is always set
        hdr.prdbc = 0;
        write_volatile(self.cmd_list, hdr);

        // Prepare Command Table (FIS) — all writes via write_volatile
        let cfis = core::ptr::addr_of_mut!((*self.cmd_table).cfis) as *mut u8;
        core::ptr::write_bytes(cfis, 0, 64);
        core::ptr::write_volatile(cfis.add(0), 0x27u8);  // Register H2D FIS
        core::ptr::write_volatile(cfis.add(1), 0x80u8);  // Command bit
        core::ptr::write_volatile(cfis.add(2), if is_write { 0x35u8 } else { 0x25u8 });
        
        core::ptr::write_volatile(cfis.add(4), (lba & 0xFF) as u8);
        core::ptr::write_volatile(cfis.add(5), ((lba >> 8) & 0xFF) as u8);
        core::ptr::write_volatile(cfis.add(6), ((lba >> 16) & 0xFF) as u8);
        core::ptr::write_volatile(cfis.add(7), 0x40u8);  // LBA mode
        
        core::ptr::write_volatile(cfis.add(8), ((lba >> 24) & 0xFF) as u8);
        core::ptr::write_volatile(cfis.add(9), ((lba >> 32) & 0xFF) as u8);
        core::ptr::write_volatile(cfis.add(10), ((lba >> 40) & 0xFF) as u8);

        core::ptr::write_volatile(cfis.add(12), (count & 0xFF) as u8);
        core::ptr::write_volatile(cfis.add(13), ((count >> 8) & 0xFF) as u8);

        // Prepare PRDT — all writes via write_volatile
        let prdt = core::ptr::addr_of_mut!((*self.cmd_table).prdt[0]) as *mut u32;
        core::ptr::write_volatile(prdt.add(0), self.dma_buf as u32);       // dba
        core::ptr::write_volatile(prdt.add(1), (self.dma_buf >> 32) as u32); // dbau
        core::ptr::write_volatile(prdt.add(2), 0u32);                       // rsv0
        core::ptr::write_volatile(prdt.add(3), (count * 512) - 1);          // dbc

        // Issue command (Slot 0)
        write_volatile(&mut (*self.port).ci, 1);

        // Poll for completion
        let mut timeout: u32 = 10_000_000;
        loop {
            let ci = read_volatile(&(*self.port).ci);
            if (ci & 1) == 0 {
                // Memory fence — ensure DMA writes to RAM are visible to CPU
                core::arch::asm!("mfence", options(nostack, preserves_flags));

                // Check TFD for errors even on "completion"
                let tfd = read_volatile(&(*self.port).tfd);
                if tfd & 0x01 != 0 {
                    // ATA ERR bit set despite CI clearing
                    return Err(DiskError::IOError);
                }
                return Ok(()); // Command completed successfully
            }
            let port_is = read_volatile(&(*self.port).is);
            if port_is & (1 << 30) != 0 {
                return Err(DiskError::IOError); // Task File Error interrupt
            }
            timeout -= 1;
            if timeout == 0 {
                return Err(DiskError::Timeout);
            }
            core::hint::spin_loop();
        }
    }
}

impl DiskReader for AhciDriver {
    fn read_sectors(&mut self, lba: u64, count: u32, buf: &mut [u8]) -> Result<(), DiskError> {
        let mut current_lba = lba;
        let mut remaining = count;
        let mut offset: usize = 0;
        while remaining > 0 {
            let chunk = core::cmp::min(remaining, 8);
            unsafe {
                self.submit_io_cmd(current_lba, chunk, false)?;
                let bytes = (chunk as usize) * 512;
                // Read DMA buffer via read_volatile — prevent stale cache reads
                let src_ptr = self.dma_buf as *const u8;
                for i in 0..bytes {
                    buf[offset + i] = core::ptr::read_volatile(src_ptr.add(i));
                }
            }
            current_lba += chunk as u64;
            remaining -= chunk;
            offset += (chunk as usize) * 512;
        }
        Ok(())
    }
}

impl DiskWriter for AhciDriver {
    fn write_sectors(&mut self, lba: u64, count: u32, buf: &[u8]) -> Result<(), DiskError> {
        if let Some((start, end)) = self.export_bounds {
            if lba < start || lba + (count as u64) > end {
                return Err(DiskError::InvalidLba);
            }
        }

        let mut current_lba = lba;
        let mut remaining = count;
        let mut offset: usize = 0;
        while remaining > 0 {
            // AHCI dma_buf is 4KB (8 sectors)
            let chunk = core::cmp::min(remaining, 8);
            unsafe {
                let bytes = (chunk as usize) * 512;
                let dst = core::slice::from_raw_parts_mut(self.dma_buf as *mut u8, bytes);
                dst.copy_from_slice(&buf[offset..offset + bytes]);
                self.submit_io_cmd(current_lba, chunk, true)?;
            }
            current_lba += chunk as u64;
            remaining -= chunk;
            offset += (chunk as usize) * 512;
        }
        Ok(())
    }
}
