//! xHCI USB Controller Driver — controller init, port enumeration, device addressing.
//!
//! Provides `XhciHal` trait for kernel services. The kernel writes MMIO via direct
//! identity mapping (PCI BAR is below 4GB).

#![no_std]

use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

// ═══════════════════════════════════════════════════════════════════
//  HAL trait
// ═══════════════════════════════════════════════════════════════════

pub trait XhciHal {
    fn alloc_dma_pages(&self, count: usize) -> Option<u64>;
    fn phys_to_virt(&self, phys: u64) -> *mut u8;
    fn log(&self, msg: &str);
    fn log_u64(&self, msg: &str, val: u64);
}

static mut XHCI_HAL: Option<&'static dyn XhciHal> = None;
static INIT: AtomicBool = AtomicBool::new(false);

pub fn init_hal(hal: &'static dyn XhciHal) {
    if INIT.swap(true, Ordering::SeqCst) { return; }
    unsafe { XHCI_HAL = Some(hal); }
}
fn hal() -> &'static dyn XhciHal { unsafe { XHCI_HAL.expect("XhciHal not init") } }

static MMIO_BASE: AtomicUsize = AtomicUsize::new(0);

pub fn set_mmio(mmio: u64) {
    MMIO_BASE.store(mmio as usize, Ordering::Relaxed);
}
pub fn get_mmio() -> Option<u64> {
    let v = MMIO_BASE.load(Ordering::Relaxed);
    if v == 0 { None } else { Some(v as u64) }
}
pub fn is_controller_initialized() -> bool {
    unsafe { CTRL.is_some() }
}

// ═══════════════════════════════════════════════════════════════════
//  Registers
// ═══════════════════════════════════════════════════════════════════

const CAPLENGTH:   u32 = 0x00; const HCSPARAMS1: u32 = 0x04;
const HCSPARAMS2:  u32 = 0x08; const HCSPARAMS3: u32 = 0x0C;
const HCCPARAMS1:  u32 = 0x10; const DBOFF:     u32 = 0x14;
const RTSOFF:      u32 = 0x18;

const USBCMD:   u32 = 0x00; const USBSTS: u32 = 0x04;
const PAGESIZE: u32 = 0x08; const CONFIG: u32 = 0x38;

const DBOFF_DB: u32 = 0x00;

const RT_IMAN: u32 = 0x20; const RT_IMOD:  u32 = 0x24;
const RT_ERSTSZ: u32 = 0x28; const RT_ERSTBA: u32 = 0x30;
const RT_ERDP:   u32 = 0x38;

const PORTSC:   u32 = 0x00; const PORTPMSC: u32 = 0x04;
const PORTLI:   u32 = 0x08;

const USBCMD_RS: u32 = 1 << 0; const USBCMD_HCRST: u32 = 1 << 1;
const USBSTS_HCH: u32 = 1 << 0; const USBSTS_CNR: u32 = 1 << 11;
const PORTSC_CCS: u32 = 1 << 0; const PORTSC_PED: u32 = 1 << 1;
const PORTSC_PR:  u32 = 1 << 4; const PORTSC_PP:  u32 = 1 << 9;
const PORTSC_CSC: u32 = 1 << 17; const PORTSC_PRC: u32 = 1 << 21;
const PORTSC_WRC: u32 = 1 << 19; const PORTSC_WPR: u32 = 1 << 31;
const IMAN_IE: u32 = 1 << 1; const IMAN_IP: u32 = 1 << 0;

const TRB_NORMAL:       u32 = 1;   const TRB_SETUP:    u32 = 2;
const TRB_DATA:         u32 = 3;   const TRB_STATUS:   u32 = 4;
const TRB_LINK:         u32 = 6;   const TRB_ENABLE:   u32 = 9;
const TRB_ADDRESS_DEV:  u32 = 11;  const TRB_CONFIGURE: u32 = 12;
const TRB_EVAL_CTX:     u32 = 13;  const TRB_NOOP:     u32 = 23;
const TRB_TRANSFER:     u32 = 32;  const TRB_COMPLETION: u32 = 33;
const TRB_PORT_STATUS:  u32 = 34;

const TRB_SIZE: usize = 16;
const RING_SIZE: usize = 256;

// ── Transfer Ring ─────────────────────────────────────────────────────

/// xHCI Transfer/Command/Event Ring backed by a DMA buffer.
///
/// **CRITICAL**: `enqueue_trb()` writes directly into the DMA buffer
/// at `dma_virt`. The xHC reads from the physical address `dma_phys`,
/// so the TRB *must* land in the same memory the controller is
/// programmed to walk.
pub struct TransferRing {
    pub dma_virt: *mut u32,
    pub dma_phys: u64,
    pub enqueue: usize,
    pub pcs: bool, // Producer Cycle State
}

impl TransferRing {
    /// Create a ring backed by a caller-supplied DMA allocation.
    /// The caller must zero the buffer before calling this.
    pub fn new(dma_virt: *mut u32, dma_phys: u64) -> Self {
        Self { dma_virt, dma_phys, enqueue: 0, pcs: true }
    }

    /// Write a 4-dword TRB at the current enqueue position in the DMA buffer.
    /// Returns the slot index (for later completion matching).
    pub fn enqueue_trb(&mut self, p0: u32, p1: u32, p2: u32, p3: u32) -> usize {
        let idx = self.enqueue;
        let base = idx * 4;
        unsafe {
            self.dma_virt.add(base    ).write_volatile(p0);
            self.dma_virt.add(base + 1).write_volatile(p1);
            self.dma_virt.add(base + 2).write_volatile(p2);
            let cycle = if self.pcs { 1u32 } else { 0u32 };
            self.dma_virt.add(base + 3).write_volatile(p3 | cycle);
        }
        self.enqueue = (idx + 1) % RING_SIZE;
        if self.enqueue == 0 { self.pcs = !self.pcs; }
        idx
    }
}

// ═══════════════════════════════════════════════════════════════════
//  Device context structures
// ═══════════════════════════════════════════════════════════════════

pub struct InputCtrlCtx { pub drop_flags: u32, pub add_flags: u32, _r: [u32; 6] }

pub struct SlotCtx { pub entries: [u32; 8] }

pub struct EpCtx { pub entries: [u32; 8] }

pub struct DeviceCtx { pub slot: SlotCtx, pub ep0: EpCtx, pub eps: [EpCtx; 30] }

pub struct InputCtx { pub ctrl: InputCtrlCtx, pub slot: SlotCtx, pub eps: [EpCtx; 31] }

// ═══════════════════════════════════════════════════════════════════
//  Controller state
// ═══════════════════════════════════════════════════════════════════

pub struct XhciController {
    pub mmio: u64,
    pub op_base: u32,
    pub rt_base: u32,
    pub db_base: u32,
    pub max_slots: u8,
    pub max_ports: u8,
    pub ctx_size: u8,
    pub dcbaa_phys: u64,
    pub cmd_ring: TransferRing,
    pub event_ring: TransferRing,
    pub erst_phys: u64,
    pub initialized: bool,
    pub evt_dequeue: u32,
    pub evt_cycle: u32,
    pub ctrl_trb_phys: u64,
    pub ctrl_data_phys: u64,
}

static mut CTRL: Option<XhciController> = None;

pub fn controller() -> Option<&'static XhciController> { unsafe { CTRL.as_ref() } }
pub fn controller_mut() -> Option<&'static mut XhciController> { unsafe { CTRL.as_mut() } }

// ═══════════════════════════════════════════════════════════════════
//  MMIO helpers
// ═══════════════════════════════════════════════════════════════════

unsafe fn read32(addr: u64) -> u32 { core::ptr::read_volatile(addr as *const u32) }
unsafe fn write32(addr: u64, val: u32) { core::ptr::write_volatile(addr as *mut u32, val); }

unsafe fn cap_read(ctrl: &XhciController, off: u32) -> u32 {
    read32(ctrl.mmio + off as u64)
}
unsafe fn op_read(ctrl: &XhciController, off: u32) -> u32 {
    read32(ctrl.mmio + ctrl.op_base as u64 + off as u64)
}
unsafe fn op_write(ctrl: &XhciController, off: u32, val: u32) {
    write32(ctrl.mmio + ctrl.op_base as u64 + off as u64, val)
}
unsafe fn rt_write(ctrl: &XhciController, off: u32, val: u32) {
    write32(ctrl.mmio + ctrl.rt_base as u64 + off as u64, val)
}

unsafe fn op_read_reg(mmio: u64, op: u32, off: u32) -> u32 {
    read32(mmio + op as u64 + off as u64)
}
unsafe fn op_write_reg(mmio: u64, op: u32, off: u32, val: u32) {
    write32(mmio + op as u64 + off as u64, val)
}
unsafe fn rt_write_reg(mmio: u64, rt: u32, off: u32, val: u32) {
    write32(mmio + rt as u64 + off as u64, val)
}

// ═══════════════════════════════════════════════════════════════════
//  Init
// ═══════════════════════════════════════════════════════════════════

pub unsafe fn init(mmio: u64) -> bool {
    if CTRL.is_some() { hal().log("[xhci] already init, skip\n"); return true; }
    let h = hal();
    h.log("[xhci] === INIT START ===\n");

    let cap_len = read32(mmio + CAPLENGTH as u64) & 0xFF;
    let hcs1 = read32(mmio + HCSPARAMS1 as u64);
    let hcc1 = read32(mmio + HCCPARAMS1 as u64);
    let op_base = cap_len;
    let rt_off = read32(mmio + RTSOFF as u64) & !0x1F;
    let db_off = read32(mmio + DBOFF as u64) & !0x1F;
    let max_slots = (hcs1 & 0xFF) as u8;
    let max_ports = ((hcs1 >> 24) & 0xFF) as u8;
    let ctx_size = if hcc1 & (1 << 2) != 0 { 1u8 } else { 0u8 };

    h.log_u64("[xhci] cap_len=", cap_len as u64);
    h.log_u64("[xhci] max_slots=", max_slots as u64);
    h.log_u64("[xhci] max_ports=", max_ports as u64);
    h.log_u64("[xhci] rt_off=", rt_off as u64);
    h.log_u64("[xhci] db_off=", db_off as u64);

    // 1. Take ownership from BIOS
    let eecp = ((hcc1 >> 8) & 0xFF) as u32;
    if eecp >= 0x40 {
        let bios = read32(mmio + eecp as u64);
        h.log_u64("[xhci] EECP=", eecp as u64);
        h.log_u64("[xhci] BIOS sem=", bios as u64);
        if bios & 1 != 0 {
            write32(mmio + eecp as u64 + 4, 1);
            for i in 0..50000 {
                if read32(mmio + eecp as u64) & 1 == 0 {
                    h.log_u64("[xhci] ownership taken after ", i as u64);
                    h.log(" loops\n");
                    break;
                }
            }
        } else {
            h.log("[xhci] BIOS already released\n");
        }
    } else {
        h.log("[xhci] No legacy support EECP\n");
    }

    // 2. Stop + reset
    let cmd = op_read_reg(mmio, op_base, USBCMD);
    h.log_u64("[xhci] USBCMD before stop=", cmd as u64);
    op_write_reg(mmio, op_base, USBCMD, cmd & !USBCMD_RS);
    for i in 0..50000 {
        if op_read_reg(mmio, op_base, USBSTS) & USBSTS_HCH != 0 {
            h.log_u64("[xhci] stopped after ", i as u64);
            h.log(" loops\n");
            break;
        }
    }
    h.log_u64("[xhci] USBSTS after stop=", op_read_reg(mmio, op_base, USBSTS) as u64);

    op_write_reg(mmio, op_base, USBCMD, USBCMD_HCRST);
    for i in 0..100000 {
        if op_read_reg(mmio, op_base, USBCMD) & USBCMD_HCRST == 0 {
            h.log_u64("[xhci] reset bit clear after ", i as u64);
            h.log(" loops\n");
            break;
        }
    }
    for i in 0..50000 {
        if op_read_reg(mmio, op_base, USBSTS) & USBSTS_CNR == 0 {
            h.log_u64("[xhci] CNR clear after ", i as u64);
            h.log(" loops\n");
            break;
        }
    }
    h.log("[xhci] reset complete\n");

    // 3. Set max slots
    let cfg = op_read_reg(mmio, op_base, CONFIG);
    op_write_reg(mmio, op_base, CONFIG, (cfg & !0xFF) | max_slots as u32);
    h.log_u64("[xhci] CONFIG=", op_read_reg(mmio, op_base, CONFIG) as u64);

    // 4. Allocate DCBAA
    let dcbaa_pages = ((max_slots as usize + 1) * 8 + 4095) / 4096;
    let dcbaa_phys = match h.alloc_dma_pages(dcbaa_pages) { Some(p) => p, None => { h.log("[xhci] FAIL dcbaa alloc\n"); return false; } };
    let dcbaa_virt = h.phys_to_virt(dcbaa_phys);
    core::ptr::write_bytes(dcbaa_virt, 0, dcbaa_pages * 4096);
    let dcbaa_phys_64 = dcbaa_phys & !0x3F;
    op_write_reg(mmio, op_base, 0x30, (dcbaa_phys_64 & 0xFFFF_FFFF) as u32);
    op_write_reg(mmio, op_base, 0x34, ((dcbaa_phys_64 >> 32) & 0xFFFF_FFFF) as u32);
    h.log_u64("[xhci] DCBAA phys=", dcbaa_phys_64);

    // 5. Set up Command Ring — WRITE TRBs TO DMA BUFFER
    let cmd_ring_pages = (RING_SIZE * TRB_SIZE + 4095) / 4096;
    let cmd_ring_phys = match h.alloc_dma_pages(cmd_ring_pages) { Some(p) => p, None => { h.log("[xhci] FAIL cmd ring alloc\n"); return false; } };
    let cmd_ring_virt = h.phys_to_virt(cmd_ring_phys) as *mut u32;
    core::ptr::write_bytes(cmd_ring_virt as *mut u8, 0, cmd_ring_pages * 4096);
    let cmd_ring_phys_64 = (cmd_ring_phys & !0x3F) | 1; // RCS = 1 (first cycle)
    op_write_reg(mmio, op_base, 0x18, (cmd_ring_phys_64 & 0xFFFF_FFFF) as u32);
    op_write_reg(mmio, op_base, 0x1C, ((cmd_ring_phys_64 >> 32) & 0xFFFF_FFFF) as u32);
    h.log_u64("[xhci] CRCR=", cmd_ring_phys_64);

    // 6. Set up Event Ring
    let evt_pages = (RING_SIZE * TRB_SIZE + 4095) / 4096;
    let evt_phys = match h.alloc_dma_pages(evt_pages) { Some(p) => p, None => { h.log("[xhci] FAIL evt ring alloc\n"); return false; } };
    let evt_virt = h.phys_to_virt(evt_phys) as *mut u32;
    core::ptr::write_bytes(evt_virt as *mut u8, 0, evt_pages * 4096);
    rt_write_reg(mmio, rt_off, RT_ERSTSZ, 1);
    rt_write_reg(mmio, rt_off, RT_ERSTBA, (evt_phys & 0xFFFF_FFFF) as u32);
    rt_write_reg(mmio, rt_off, RT_ERSTBA + 4, ((evt_phys >> 32) & 0xFFFF_FFFF) as u32);
    rt_write_reg(mmio, rt_off, RT_ERDP, (evt_phys & 0xFFFF_FFFF) as u32);
    rt_write_reg(mmio, rt_off, RT_ERDP + 4, ((evt_phys >> 32) & 0xFFFF_FFFF) as u32);
    rt_write_reg(mmio, rt_off, RT_IMAN, IMAN_IE);
    h.log_u64("[xhci] ERST phys=", evt_phys);

    // 7. Page size
    op_write_reg(mmio, op_base, PAGESIZE, 1);

    // 8. Start controller
    let cmd2 = op_read_reg(mmio, op_base, USBCMD);
    op_write_reg(mmio, op_base, USBCMD, cmd2 | USBCMD_RS);
    for i in 0..50000 {
        if op_read_reg(mmio, op_base, USBSTS) & USBSTS_HCH == 0 {
            h.log_u64("[xhci] started after ", i as u64);
            h.log(" loops\n");
            break;
        }
    }
    h.log_u64("[xhci] USBSTS after start=", op_read_reg(mmio, op_base, USBSTS) as u64);

    // 9. Persistent control-transfer buffers
    let ctrl_trb_phys = match h.alloc_dma_pages(1) { Some(p) => p, None => { h.log("[xhci] FAIL ctrl trb alloc\n"); return false; } };
    core::ptr::write_bytes(h.phys_to_virt(ctrl_trb_phys), 0, 4096);
    let ctrl_data_phys = match h.alloc_dma_pages(1) { Some(p) => p, None => { h.log("[xhci] FAIL ctrl data alloc\n"); return false; } };
    core::ptr::write_bytes(h.phys_to_virt(ctrl_data_phys), 0, 4096);

    // 10. Build TransferRing wrappers pointing to DMA memory
    let cmd_ring = TransferRing::new(cmd_ring_virt, cmd_ring_phys & !0x3F);
    let event_ring = TransferRing::new(evt_virt, evt_phys & !0x3F);

    let ctrl = XhciController {
        mmio, op_base, rt_base: rt_off, db_base: db_off,
        max_slots, max_ports, ctx_size,
        dcbaa_phys: dcbaa_phys_64,
        cmd_ring, event_ring,
        erst_phys: evt_phys & !0x3F,
        initialized: true,
        evt_dequeue: 0, evt_cycle: 1,
        ctrl_trb_phys, ctrl_data_phys,
    };
    CTRL = Some(ctrl);

    h.log("[xhci] === INIT DONE ===\n");
    true
}

// ═══════════════════════════════════════════════════════════════════
//  Port enumeration
// ═══════════════════════════════════════════════════════════════════

pub unsafe fn port_reset(port: u8) -> bool {
    let ctrl = match CTRL.as_mut() { Some(c) => c, None => { hal().log("[xhci] port_reset: no ctrl\n"); return false; } };
    let port_base = ctrl.op_base as u64 + 0x400 + port as u64 * 0x10;
    let sc = read32(ctrl.mmio + port_base + PORTSC as u64);

    hal().log_u64("[xhci] port_reset(", port as u64);
    hal().log_u64(") PORTSC=", sc as u64);

    if sc & PORTSC_CCS == 0 {
        hal().log_u64("[xhci] port ", port as u64);
        hal().log(" not connected\n");
        return false;
    }

    let speed = (sc >> 10) & 0x0F;
    hal().log_u64("[xhci] port speed=", speed as u64);

    write32(ctrl.mmio + port_base + PORTSC as u64, sc | PORTSC_PR);
    hal().log("[xhci] reset asserted, waiting...\n");
    for i in 0..100000 {
        let s = read32(ctrl.mmio + port_base + PORTSC as u64);
        if s & PORTSC_PR == 0 && s & PORTSC_PRC != 0 {
            write32(ctrl.mmio + port_base + PORTSC as u64, s | PORTSC_PRC);
            hal().log_u64("[xhci] PRC cleared at loop ", i as u64);
            for j in 0..50000 {
                let s2 = read32(ctrl.mmio + port_base + PORTSC as u64);
                if s2 & PORTSC_PED != 0 {
                    hal().log_u64("[xhci] port enabled at loop ", j as u64);
                    hal().log("\n");
                    return true;
                }
                core::hint::spin_loop();
            }
            hal().log("[xhci] port NOT enabled after reset\n");
            break;
        }
        core::hint::spin_loop();
    }
    hal().log("[xhci] port_reset FAIL\n");
    false
}

pub unsafe fn address_device(port: u8, slot_type: u8) -> Option<u8> {
    let ctrl = match CTRL.as_mut() { Some(c) => c, None => { hal().log("[xhci] addr_dev: no ctrl\n"); return None; } };
    let h = hal();

    h.log_u64("[xhci] address_device port=", port as u64);
    h.log_u64(" type=", slot_type as u64);
    h.log("\n");

    let slot = enable_slot(ctrl)?;
    h.log_u64("[xhci] slot enabled → ", slot as u64);

    let ctx_pages = (core::mem::size_of::<InputCtx>() + 4095) / 4096;
    let ctx_phys = h.alloc_dma_pages(ctx_pages)?;
    let ctx_virt = h.phys_to_virt(ctx_phys) as *mut InputCtx;
    core::ptr::write_bytes(ctx_virt as *mut u8, 0, ctx_pages * 4096);

    (*ctx_virt).ctrl.add_flags = 3;
    let sc = &mut (*ctx_virt).slot;
    sc.entries[0] = (1 << 27);
    sc.entries[1] = (slot_type as u32) << 16;
    sc.entries[2] = 0;
    sc.entries[3] = (1 << 26) | (port as u32 + 1);

    let ep0 = &mut (*ctx_virt).eps[0];
    ep0.entries[0] = 0;
    ep0.entries[1] = 8;
    ep0.entries[2] = 0;

    let dcbaa_virt = h.phys_to_virt(ctrl.dcbaa_phys) as *mut u64;
    dcbaa_virt.add(slot as usize).write_volatile(ctx_phys);

    h.log_u64("[xhci] ctx_phys=", ctx_phys);

    let trb_p0 = ctx_phys as u32;
    let trb_p1 = ((ctx_phys >> 32) & 0xFFFF_FFFF) as u32;
    let trb_p2 = (slot as u32) << 24;

    // Write Address Device TRB into the DMA-backed command ring
    ctrl.cmd_ring.enqueue_trb(trb_p0, trb_p1, trb_p2, TRB_ADDRESS_DEV << 10);
    ring_doorbell(ctrl, 0);
    h.log("[xhci] address_device doorbell rung, waiting event...\n");

    let evt = wait_for_event(ctrl);
    if evt.is_none() {
        h.log("[xhci] address_device: no event\n");
        return None;
    }

    h.log_u64("[xhci] address_device event dw3=", evt.unwrap().3 as u64);
    Some(slot)
}

unsafe fn enable_slot(ctrl: &mut XhciController) -> Option<u8> {
    hal().log("[xhci] enable_slot...\n");
    ctrl.cmd_ring.enqueue_trb(0, 0, 0, TRB_ENABLE << 10);
    ring_doorbell(ctrl, 0);
    let evt = wait_for_event(ctrl);
    if evt.is_none() {
        hal().log("[xhci] enable_slot: no event\n");
        return None;
    }
    let (_, _, _, dw3) = evt.unwrap();
    let slot = ((dw3 >> 16) & 0xFF) as u8;
    hal().log_u64("[xhci] enable_slot got slot=", slot as u64);
    if slot == 0 { None } else { Some(slot) }
}

unsafe fn ring_doorbell(ctrl: &XhciController, target: u32) {
    write32(ctrl.mmio + ctrl.db_base as u64 + DBOFF_DB as u64, target);
}

unsafe fn wait_for_event(ctrl: &mut XhciController) -> Option<(u32, u32, u32, u32)> {
    let evt_virt = hal().phys_to_virt(ctrl.erst_phys) as *const u32;
    let mut dequeue = ctrl.evt_dequeue;
    let mut cycle   = ctrl.evt_cycle;

    for i in 0..500000 {
        let base = (dequeue as usize) * 4;
        let dw3 = evt_virt.add(base + 3).read_volatile();

        if (dw3 & 1) == cycle {
            let dw0 = evt_virt.add(base).read_volatile();
            let dw1 = evt_virt.add(base + 1).read_volatile();
            let dw2 = evt_virt.add(base + 2).read_volatile();

            dequeue += 1;
            if dequeue >= RING_SIZE as u32 {
                dequeue = 0;
                cycle ^= 1;
            }

            let new_erdp = ctrl.erst_phys + (dequeue as u64) * (TRB_SIZE as u64);
            write32(ctrl.mmio + ctrl.rt_base as u64 + RT_ERDP as u64,
                    (new_erdp & 0xFFFF_FFFF) as u32);
            write32(ctrl.mmio + ctrl.rt_base as u64 + RT_ERDP as u64 + 4,
                    ((new_erdp >> 32) & 0xFFFF_FFFF) as u32);

            ctrl.evt_dequeue = dequeue;
            ctrl.evt_cycle   = cycle;

            let cc = (dw3 >> 24) & 0xFF;
            if cc == 1 || cc == 13 {
                return Some((dw0, dw1, dw2, dw3));
            }
            hal().log_u64("[xhci] event fail cc=", cc as u64);
            hal().log_u64(" type=", ((dw3 >> 10) & 0x3F) as u64);
            hal().log("\n");
            return None;
        }

        if i == 5000 || i == 50000 || i == 250000 {
            hal().log_u64("[xhci] waiting for event... ", i as u64);
            hal().log_u64(" dequeue=", dequeue as u64);
            hal().log_u64(" cycle=", cycle as u64);
            hal().log_u64(" dw3=", dw3 as u64);
            hal().log("\n");
        }

        core::hint::spin_loop();
    }

    hal().log("[xhci] EVENT TIMEOUT\n");
    None
}

// ═══════════════════════════════════════════════════════════════════
//  Control transfers
// ═══════════════════════════════════════════════════════════════════

pub unsafe fn control_transfer(slot: u8, bm_req_type: u8, b_request: u8,
    w_value: u16, w_index: u16, buf: &mut [u8], data_in: bool) -> usize
{
    let ctrl = match CTRL.as_mut() { Some(c) => c, None => { hal().log("[xhci] ctrl_xfer: no ctrl\n"); return 0; } };
    let h = hal();

    h.log_u64("[xhci] ctrl_xfer slot=", slot as u64);
    h.log_u64(" type=", bm_req_type as u64);
    h.log_u64(" req=", b_request as u64);
    h.log("\n");

    let dev_ctx_phys = match read_dcbaa(ctrl, slot) { Some(p) => p, None => { h.log("[xhci] ctrl_xfer: no dev ctx\n"); return 0; } };
    let dev_ctx_virt = h.phys_to_virt(dev_ctx_phys) as *mut u8;
    let ep0_off = 32 + (ctrl.ctx_size as u32) * 32;

    // Use persistent DMA buffers for the control TRB chain and data
    let trb_virt = h.phys_to_virt(ctrl.ctrl_trb_phys) as *mut u32;
    core::ptr::write_bytes(trb_virt as *mut u8, 0, 4096);
    let data_virt = h.phys_to_virt(ctrl.ctrl_data_phys) as *mut u8;

    // Setup Stage
    let setup_dwords: [u32; 2] = [
        (bm_req_type as u32) | ((b_request as u32) << 8) | ((w_value as u32) << 16),
        (w_index as u32) | ((buf.len() as u32) << 16),
    ];
    trb_virt.add(0).write_volatile(setup_dwords[0]);
    trb_virt.add(1).write_volatile(setup_dwords[1]);
    trb_virt.add(2).write_volatile(8);
    let trt = if buf.is_empty() { 3u32 }
              else if data_in   { 2u32 }
              else              { 0u32 };
    trb_virt.add(3).write_volatile(
        (TRB_SETUP << 10)
      | (1       <<  6)
      | (trt     << 16)
      | (1       <<  4)
      | 1
    );

    // Data Stage
    let mut status_idx = 1u32;
    let has_data = !buf.is_empty();
    if has_data {
        if !data_in {
            for i in 0..buf.len() {
                data_virt.add(i).write_volatile(buf[i]);
            }
        }
        let dphys = ctrl.ctrl_data_phys;
        trb_virt.add(4).write_volatile((dphys & 0xFFFF_FFFF) as u32);
        trb_virt.add(5).write_volatile(((dphys >> 32) & 0xFFFF_FFFF) as u32);
        trb_virt.add(6).write_volatile(buf.len() as u32);
        let dir = if data_in { 1u32 << 16 } else { 0u32 };
        trb_virt.add(7).write_volatile(
            (TRB_DATA << 10)
          | dir
          | (1 << 4)
          | 1
        );
        status_idx = 2;
    }

    // Status Stage
    let sb = (status_idx as usize) * 4;
    trb_virt.add(sb).write_volatile(0);
    trb_virt.add(sb + 1).write_volatile(0);
    trb_virt.add(sb + 2).write_volatile(0);
    let dir_in  = if has_data { !data_in } else { true };
    let dir_bit = if dir_in { 1u32 << 16 } else { 0u32 };
    trb_virt.add(sb + 3).write_volatile(
        (TRB_STATUS << 10)
      | dir_bit
      | (1 << 5)
      | 1
    );

    // Update EP0 dequeue pointer to point to our TRB chain in DMA memory
    let dequeue = ctrl.ctrl_trb_phys | 1; // DCS = 1
    let dq_lo = dev_ctx_virt.add((ep0_off + 8) as usize) as *mut u32;
    let dq_hi = dev_ctx_virt.add((ep0_off + 12) as usize) as *mut u32;
    dq_lo.write_volatile((dequeue & 0xFFFF_FFFF) as u32);
    dq_hi.write_volatile(((dequeue >> 32) & 0xFFFF_FFFF) as u32);

    // Ring doorbell
    write32(ctrl.mmio + ctrl.db_base as u64, slot as u32);

    // Wait for Transfer Event completion
    if let Some((_p0, _p1, dw2, dw3)) = wait_for_event(ctrl) {
        let cc = (dw3 >> 24) & 0xFF;
        h.log_u64("[xhci] ctrl_xfer event cc=", cc as u64);
        h.log_u64(" dw3=", dw3 as u64);
        h.log("\n");
        if cc == 1 || cc == 13 {
            let remaining = dw2 as usize;
            let transferred = buf.len().saturating_sub(remaining);
            if data_in && has_data {
                let copy_len = transferred.min(buf.len());
                for i in 0..copy_len {
                    buf[i] = data_virt.add(i).read_volatile();
                }
            }
            h.log_u64("[xhci] ctrl_xfer OK transferred=", transferred as u64);
            h.log("\n");
            return transferred;
        }
    } else {
        h.log("[xhci] ctrl_xfer: no event\n");
    }
    0
}

unsafe fn read_dcbaa(ctrl: &XhciController, slot: u8) -> Option<u64> {
    let dcbaa = hal().phys_to_virt(ctrl.dcbaa_phys) as *const u64;
    let ctx = dcbaa.add(slot as usize).read_volatile();
    if ctx == 0 {
        hal().log_u64("[xhci] dcbaa slot ", slot as u64);
        hal().log(" = 0\n");
        None
    } else {
        Some(ctx)
    }
}
