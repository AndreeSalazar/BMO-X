//! AHCI controller: probe, DMA init, sector read.
//!
//! Uses `StorageHal` trait for physical memory allocation and MMIO access.
//! Hardware registers accessed via direct pointers (identity-mapped PCI BARs).

use crate::storage_hal;
use core::sync::atomic::{AtomicBool, Ordering};

// ── HBA Registers ────────────────────────────────────────────────

const HBA_CAP: usize = 0x00;
const HBA_GHC: usize = 0x04;
const HBA_PI:  usize = 0x0C;
const PORT_STRIDE: usize = 0x100;
const PORT_CLB:  usize = 0x00;
const PORT_CLBU: usize = 0x04;
const PORT_FB:   usize = 0x08;
const PORT_FBU:  usize = 0x0C;
const PORT_CMD:  usize = 0x18;
const PORT_SSTS: usize = 0x28;
const PORT_SIG:  usize = 0x24;
const PORT_IS:   usize = 0x10;
const PORT_CI:   usize = 0x38;

const GHC_HR: u32 = 1 << 0;
const GHC_IE: u32 = 1 << 1;
const CMD_ST:  u32 = 1 << 0;
const CMD_FRE: u32 = 1 << 4;
const CMD_FR:  u32 = 1 << 14;
const CMD_CR:  u32 = 1 << 15;
const SSTS_DET: u32 = 0x0F;
const FIS_TYPE_REG_H2D: u8 = 0x27;
const ATA_CMD_READ_DMA_EX: u8 = 0x25;
const ATA_CMD_WRITE_DMA_EX: u8 = 0x35;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortState { Empty, Present, Active, Error }

#[derive(Debug, Clone, Copy)]
pub struct AhciPort {
    pub port_number: u8,
    pub state: PortState,
    pub signature: u32,
    pub command_list_phys: u64,
    pub fis_phys: u64,
}

#[derive(Debug)]
pub struct AhciController {
    pub mmio_base: u64,
    pub port_count: u8,
    pub ports_implemented: u32,
    pub ports: [AhciPort; 32],
}

static mut CONTROLLER: Option<AhciController> = None;
static INIT_DONE: AtomicBool = AtomicBool::new(false);

unsafe fn hba_read(mmio: u64, offset: usize) -> u32 {
    core::ptr::read_volatile((mmio + offset as u64) as *const u32)
}
unsafe fn hba_write(mmio: u64, offset: usize, val: u32) {
    core::ptr::write_volatile((mmio + offset as u64) as *mut u32, val);
}
unsafe fn port_read(mmio: u64, port: u8, offset: usize) -> u32 {
    let base = mmio + 0x100 + (port as u64) * PORT_STRIDE as u64;
    core::ptr::read_volatile((base + offset as u64) as *const u32)
}
unsafe fn port_write(mmio: u64, port: u8, offset: usize, val: u32) {
    let base = mmio + 0x100 + (port as u64) * PORT_STRIDE as u64;
    core::ptr::write_volatile((base + offset as u64) as *mut u32, val);
}

/// Probe and initialize the AHCI controller at the given MMIO base.
pub unsafe fn probe(mmio_base: u64) -> bool {
    if INIT_DONE.swap(true, Ordering::SeqCst) { return true; }

    let hal = storage_hal::hal();
    hal.log("[ahci] probing controller\n");

    let cap = hba_read(mmio_base, HBA_CAP);
    let port_count = ((cap >> 20) & 0x1F) as u8 + 1;
    let pi = hba_read(mmio_base, HBA_PI);

    // Reset HBA
    hba_write(mmio_base, HBA_GHC, GHC_HR);
    for _ in 0..100000 {
        if hba_read(mmio_base, HBA_GHC) & GHC_HR == 0 { break; }
    }
    hba_write(mmio_base, HBA_GHC, GHC_IE);

    let mut ctrl = AhciController {
        mmio_base, port_count, ports_implemented: pi,
        ports: [AhciPort { port_number: 0, state: PortState::Empty, signature: 0, command_list_phys: 0, fis_phys: 0 }; 32],
    };

    for i in 0..port_count.min(32) {
        if pi & (1 << i) == 0 { continue; }
        let ssts = port_read(mmio_base, i, PORT_SSTS);
        let det = ssts & SSTS_DET;
        let state = match det { 0x03 => PortState::Active, 0x01 => PortState::Present, _ => PortState::Empty };
        let sig = port_read(mmio_base, i, PORT_SIG);
        ctrl.ports[i as usize] = AhciPort { port_number: i, state, signature: sig, command_list_phys: 0, fis_phys: 0 };
    }

    CONTROLLER = Some(ctrl);
    hal.log("[ahci] probe complete\n");
    true
}

/// Initialize DMA for a port.
pub unsafe fn init_port_dma(port_idx: u8) -> bool {
    let ctrl = match CONTROLLER.as_mut() { Some(c) => c, None => return false };
    let port = &mut ctrl.ports[port_idx as usize];
    if port.state != PortState::Active { return false; }
    let mmio = ctrl.mmio_base;
    let hal = storage_hal::hal();

    let cl_phys = match hal.alloc_dma_pages(1) { Some(p) => p, None => return false };
    let cl_virt = hal.phys_to_virt(cl_phys);
    core::ptr::write_bytes(cl_virt, 0, 4096);
    port.command_list_phys = cl_phys;

    let fis_phys = match hal.alloc_dma_pages(1) { Some(p) => p, None => return false };
    let fis_virt = hal.phys_to_virt(fis_phys);
    core::ptr::write_bytes(fis_virt, 0, 4096);
    port.fis_phys = fis_phys;

    let ct_phys = match hal.alloc_dma_pages(1) { Some(p) => p, None => return false };
    let ct_virt = hal.phys_to_virt(ct_phys) as *mut u32;
    core::ptr::write_bytes(ct_virt as *mut u8, 0, 4096);
    ct_virt.write_volatile(ct_phys as u32); // CTBA low

    port_write(mmio, port_idx, PORT_CLB, cl_phys as u32);
    port_write(mmio, port_idx, PORT_CLBU, (cl_phys >> 32) as u32);
    port_write(mmio, port_idx, PORT_FB, fis_phys as u32);
    port_write(mmio, port_idx, PORT_FBU, (fis_phys >> 32) as u32);

    // Stop + restart port
    let mut cmd = port_read(mmio, port_idx, PORT_CMD);
    while cmd & (CMD_CR | CMD_FR) != 0 {
        port_write(mmio, port_idx, PORT_CMD, cmd & !(CMD_ST | CMD_FRE));
        core::hint::spin_loop();
        cmd = port_read(mmio, port_idx, PORT_CMD);
    }
    port_write(mmio, port_idx, PORT_CMD, CMD_FRE | CMD_ST);
    for _ in 0..1000 {
        cmd = port_read(mmio, port_idx, PORT_CMD);
        if cmd & CMD_FR != 0 && cmd & CMD_CR != 0 { break; }
        core::hint::spin_loop();
    }
    true
}

/// Read sectors from a port into a buffer.
pub unsafe fn read_sectors(port_idx: u8, lba: u64, sector_count: u16, buf: *mut u8) -> u16 {
    let ctrl = match CONTROLLER.as_ref() { Some(c) => c, None => return 0 };
    let port = &ctrl.ports[port_idx as usize];
    let mmio = ctrl.mmio_base;
    if port.command_list_phys == 0 { return 0; }
    let hal = storage_hal::hal();

    let cl_virt = hal.phys_to_virt(port.command_list_phys) as *mut u32;
    let ct_phys = cl_virt.read_volatile() as u64;
    let ct_virt = hal.phys_to_virt(ct_phys) as *mut u8;

    // Build command FIS
    let fis = ct_virt;
    fis.write_volatile(FIS_TYPE_REG_H2D);
    fis.add(1).write_volatile(0x80); // C=1
    fis.add(2).write_volatile(ATA_CMD_READ_DMA_EX);
    fis.add(3).write_volatile(0);
    let lb = lba.to_le_bytes();
    fis.add(4).write_volatile(lb[0]); fis.add(5).write_volatile(lb[1]); fis.add(6).write_volatile(lb[2]);
    fis.add(7).write_volatile(0x40);
    fis.add(8).write_volatile(lb[3]); fis.add(9).write_volatile(lb[4]); fis.add(10).write_volatile(lb[5]);
    fis.add(11).write_volatile(0);
    fis.add(12).write_volatile((sector_count & 0xFF) as u8);
    fis.add(13).write_volatile((sector_count >> 8) as u8);

    // PRDT
    let prdt = ct_virt.add(0x80) as *mut u32;
    prdt.write_volatile(buf as u32);
    prdt.add(1).write_volatile(0);
    prdt.add(2).write_volatile(0);
    let bc = (sector_count as u32).min(65535 / 512) * 512;
    prdt.add(3).write_volatile(bc | (1 << 31));

    // CFL + PRDBC
    let cl_hdr = cl_virt;
    cl_hdr.add(2).write_volatile(0);
    cl_hdr.add(3).write_volatile(bc | (5 << 16));

    port_write(mmio, port_idx, PORT_CI, 1);

    for _ in 0..1000000 {
        let ci = port_read(mmio, port_idx, PORT_CI);
        let is_val = port_read(mmio, port_idx, PORT_IS);
        if (ci & 1) == 0 { port_write(mmio, port_idx, PORT_IS, is_val); return sector_count; }
        if is_val & (1 << 30) != 0 { port_write(mmio, port_idx, PORT_IS, is_val); return 0; }
        core::hint::spin_loop();
    }
    0
}

/// Write sectors to a port from a buffer.
pub unsafe fn write_sectors(port_idx: u8, lba: u64, sector_count: u16, buf: *const u8) -> u16 {
    let ctrl = match CONTROLLER.as_ref() { Some(c) => c, None => return 0 };
    let port = &ctrl.ports[port_idx as usize];
    let mmio = ctrl.mmio_base;
    if port.command_list_phys == 0 { return 0; }
    let hal = storage_hal::hal();

    let cl_virt = hal.phys_to_virt(port.command_list_phys) as *mut u32;
    let ct_phys = cl_virt.read_volatile() as u64;
    let ct_virt = hal.phys_to_virt(ct_phys) as *mut u8;

    // Build command FIS
    let fis = ct_virt;
    fis.write_volatile(FIS_TYPE_REG_H2D);
    fis.add(1).write_volatile(0x80); // C=1
    fis.add(2).write_volatile(ATA_CMD_WRITE_DMA_EX);
    fis.add(3).write_volatile(0);
    let lb = lba.to_le_bytes();
    fis.add(4).write_volatile(lb[0]); fis.add(5).write_volatile(lb[1]); fis.add(6).write_volatile(lb[2]);
    fis.add(7).write_volatile(0x40);
    fis.add(8).write_volatile(lb[3]); fis.add(9).write_volatile(lb[4]); fis.add(10).write_volatile(lb[5]);
    fis.add(11).write_volatile(0);
    fis.add(12).write_volatile((sector_count & 0xFF) as u8);
    fis.add(13).write_volatile((sector_count >> 8) as u8);

    // PRDT
    let prdt = ct_virt.add(0x80) as *mut u32;
    prdt.write_volatile(buf as u32);
    prdt.add(1).write_volatile(0);
    prdt.add(2).write_volatile(0);
    let bc = (sector_count as u32).min(65535 / 512) * 512;
    prdt.add(3).write_volatile(bc | (1 << 31));

    // CFL + PRDBC
    let cl_hdr = cl_virt;
    cl_hdr.add(2).write_volatile(0);
    cl_hdr.add(3).write_volatile(bc | (5 << 16));

    port_write(mmio, port_idx, PORT_CI, 1);

    for _ in 0..1000000 {
        let ci = port_read(mmio, port_idx, PORT_CI);
        let is_val = port_read(mmio, port_idx, PORT_IS);
        if (ci & 1) == 0 { port_write(mmio, port_idx, PORT_IS, is_val); return sector_count; }
        if is_val & (1 << 30) != 0 { port_write(mmio, port_idx, PORT_IS, is_val); return 0; }
        core::hint::spin_loop();
    }
    0
}

pub fn controller() -> Option<&'static AhciController> {
    unsafe { CONTROLLER.as_ref() }
}
