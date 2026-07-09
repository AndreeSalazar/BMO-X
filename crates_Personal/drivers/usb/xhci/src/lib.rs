//! xHCI USB Controller Driver — full device lifecycle + HID interrupt transfers.
//!
//! Modeled after the proven xhci-nostd crate at github.com/suhteevah/xhci-nostd.

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
pub fn hal() -> &'static dyn XhciHal { unsafe { XHCI_HAL.expect("XhciHal not init") } }

static MMIO_BASE: AtomicUsize = AtomicUsize::new(0);

pub fn set_mmio(mmio: u64) { MMIO_BASE.store(mmio as usize, Ordering::Relaxed); }
pub fn get_mmio() -> Option<u64> {
    let v = MMIO_BASE.load(Ordering::Relaxed);
    if v == 0 { None } else { Some(v as u64) }
}
pub fn is_controller_initialized() -> bool { unsafe { CTRL.is_some() } }

// ═══════════════════════════════════════════════════════════════════
//  Registers
// ═══════════════════════════════════════════════════════════════════

const CAPLENGTH: u32 = 0x00; const HCSPARAMS1: u32 = 0x04;
const HCSPARAMS2: u32 = 0x08; const HCSPARAMS3: u32 = 0x0C;
const HCCPARAMS1: u32 = 0x10; const DBOFF: u32 = 0x14; const RTSOFF: u32 = 0x18;
const USBCMD: u32 = 0x00; const USBSTS: u32 = 0x04;
const PAGESIZE: u32 = 0x08; const CONFIG: u32 = 0x38;
const DBOFF_DB: u32 = 0x00;
const RT_IMAN: u32 = 0x20; const RT_IMOD: u32 = 0x24;
const RT_ERSTSZ: u32 = 0x28; const RT_ERSTBA: u32 = 0x30;
const RT_ERDP: u32 = 0x38;
const PORTSC: u32 = 0x00;
const USBCMD_RS: u32 = 1 << 0; const USBCMD_HCRST: u32 = 1 << 1;
const USBSTS_HCH: u32 = 1 << 0; const USBSTS_CNR: u32 = 1 << 11;
const PORTSC_CCS: u32 = 1 << 0; const PORTSC_PED: u32 = 1 << 1;
const PORTSC_PR: u32 = 1 << 4; const PORTSC_PP: u32 = 1 << 9;
const PORTSC_CSC: u32 = 1 << 17; const PORTSC_PRC: u32 = 1 << 21;
const IMAN_IE: u32 = 1 << 1; const IMAN_IP: u32 = 1 << 0;

const TRB_NORMAL: u32 = 1;  const TRB_SETUP: u32 = 2;
const TRB_DATA: u32 = 3;    const TRB_STATUS: u32 = 4;
const TRB_LINK: u32 = 6;    const TRB_ENABLE: u32 = 9;
const TRB_ADDRESS_DEV: u32 = 11; const TRB_CONFIGURE: u32 = 12;
const TRB_EVAL_CTX: u32 = 13;
const TRB_TRANSFER: u32 = 32; const TRB_COMPLETION: u32 = 33;
const TRB_PORT_STATUS: u32 = 34;

const TRB_SIZE: usize = 16;
const RING_SIZE: usize = 256;
const LAST_TRB_IDX: usize = RING_SIZE - 1; // Link TRB lives here

const CC_SUCCESS: u32 = 1;
const CC_SHORT: u32 = 13;

// ═══════════════════════════════════════════════════════════════════
//  TRB  (16 bytes, 4 DWORDs)
// ═══════════════════════════════════════════════════════════════════

#[derive(Clone, Copy)]
struct Trb { dw0: u32, dw1: u32, dw2: u32, dw3: u32 }

impl Trb {
    fn zeroed() -> Self { Trb { dw0: 0, dw1: 0, dw2: 0, dw3: 0 } }
    fn with_ptr(ptr: u64) -> Self {
        Trb { dw0: ptr as u32, dw1: (ptr >> 32) as u32, dw2: 0, dw3: 0 }
    }
}

// ═══════════════════════════════════════════════════════════════════
//  Transfer Ring
// ═══════════════════════════════════════════════════════════════════

pub struct TransferRing {
    dma_virt: *mut u32,
    dma_phys: u64,
    enqueue: usize,
    pcs: bool,
}

impl TransferRing {
    /// Create a new ring. The DMA buffer must be 4K, zeroed.
    /// Places a Link TRB at LAST_TRB_IDX pointing back to `dma_phys`.
    pub unsafe fn new(dma_virt: *mut u32, dma_phys: u64) -> Self {
        let mut r = Self { dma_virt, dma_phys: dma_phys & !0xF, enqueue: 0, pcs: true };
        // Link TRB at the last slot
        let base = LAST_TRB_IDX * 4;
        r.dma_virt.add(base    ).write_volatile((r.dma_phys & 0xFFFF_FFFF) as u32);
        r.dma_virt.add(base + 1).write_volatile(((r.dma_phys >> 32) & 0xFFFF_FFFF) as u32);
        r.dma_virt.add(base + 2).write_volatile(0);
        r.dma_virt.add(base + 3).write_volatile((TRB_LINK << 10) | 1);
        r
    }

    /// Enable Toggle Cycle on Link TRB (for event rings where xHC manages cycle).
    pub unsafe fn enable_toggle_cycle(&mut self) {
        let base = LAST_TRB_IDX * 4;
        let dw3 = self.dma_virt.add(base + 3).read_volatile();
        self.dma_virt.add(base + 3).write_volatile(dw3 | (1 << 1));
    }

    pub fn phys_with_dcs(&self) -> u64 { self.dma_phys | if self.pcs { 1 } else { 0 } }

    /// Write a TRB at the current enqueue position, advance.
    pub fn enqueue(&mut self, trb: &Trb) {
        let idx = self.enqueue;
        let b = idx * 4;
        unsafe {
            self.dma_virt.add(b    ).write_volatile(trb.dw0);
            self.dma_virt.add(b + 1).write_volatile(trb.dw1);
            self.dma_virt.add(b + 2).write_volatile(trb.dw2);
            let cycle = if self.pcs { 1u32 } else { 0u32 };
            self.dma_virt.add(b + 3).write_volatile(trb.dw3 | cycle);
        }
        self.advance();
    }

    /// Advance enqueue, wrapping at Link TRB.
    fn advance(&mut self) {
        self.enqueue += 1;
        if self.enqueue >= LAST_TRB_IDX {
            // Update Link TRB cycle bit to match current PCS
            let lb = LAST_TRB_IDX * 4;
            unsafe {
                let link_dw3 = self.dma_virt.add(lb + 3).read_volatile();
                if self.pcs { self.dma_virt.add(lb + 3).write_volatile(link_dw3 | 1); }
                else { self.dma_virt.add(lb + 3).write_volatile(link_dw3 & !1u32); }
            }
            self.enqueue = 0;
            self.pcs = !self.pcs;
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
//  USB Descriptor types  (for external consumers)
// ═══════════════════════════════════════════════════════════════════

pub const USB_REQ_GET_DESCRIPTOR: u8 = 0x06;
pub const USB_REQ_SET_CONFIG: u8 = 0x09;
pub const USB_DESC_DEVICE: u16 = 1;
pub const USB_DESC_CONFIG: u16 = 2;

// ═══════════════════════════════════════════════════════════════════
//  Controller
// ═══════════════════════════════════════════════════════════════════

pub struct XhciController {
    pub mmio: u64, pub op_base: u32, pub rt_base: u32, pub db_base: u32,
    pub max_slots: u8, pub max_ports: u8, pub ctx_size: u8,
    pub dcbaa_phys: u64,
    pub cmd_ring: TransferRing,
    pub event_ring: TransferRing,
    pub erst_phys: u64,
    pub initialized: bool,
    pub evt_dequeue: u32, pub evt_cycle: u32,
}

static mut CTRL: Option<XhciController> = None;
pub fn controller() -> Option<&'static XhciController> { unsafe { CTRL.as_ref() } }
pub fn controller_mut() -> Option<&'static mut XhciController> { unsafe { CTRL.as_mut() } }

// ═══════════════════════════════════════════════════════════════════
//  MMIO helpers
// ═══════════════════════════════════════════════════════════════════

unsafe fn r32(addr: u64) -> u32 { core::ptr::read_volatile(addr as *const u32) }
unsafe fn w32(addr: u64, val: u32) { core::ptr::write_volatile(addr as *mut u32, val); }
unsafe fn op_r(m: u64, o: u32, off: u32) -> u32 { r32(m + o as u64 + off as u64) }
unsafe fn op_w(m: u64, o: u32, off: u32, v: u32) { w32(m + o as u64 + off as u64, v) }
unsafe fn rt_w(m: u64, r: u32, off: u32, v: u32) { w32(m + r as u64 + off as u64, v) }

fn ctx_sz(c: &XhciController) -> usize { if c.ctx_size != 0 { 64 } else { 32 } }

// ═══════════════════════════════════════════════════════════════════
//  Event ring consumer
// ═══════════════════════════════════════════════════════════════════

unsafe fn evt_poll_block(ctrl: &mut XhciController) -> Option<(u32, u32, u32, u32)> {
    let base = hal().phys_to_virt(ctrl.erst_phys) as *const u32;
    let mut dq = ctrl.evt_dequeue;
    let mut cy = ctrl.evt_cycle;
    for _ in 0..500000 {
        let dw3 = base.add((dq as usize) * 4 + 3).read_volatile();
        if (dw3 & 1) == cy {
            let dw0 = base.add((dq as usize) * 4).read_volatile();
            let dw1 = base.add((dq as usize) * 4 + 1).read_volatile();
            let dw2 = base.add((dq as usize) * 4 + 2).read_volatile();
            dq += 1; if dq >= RING_SIZE as u32 { dq = 0; cy ^= 1; }
            let erdp = ctrl.erst_phys + (dq as u64) * (TRB_SIZE as u64);
            w32(ctrl.mmio + ctrl.rt_base as u64 + RT_ERDP as u64, (erdp & 0xFFFF_FFFF) as u32);
            w32(ctrl.mmio + ctrl.rt_base as u64 + RT_ERDP as u64 + 4, ((erdp >> 32) & 0xFFFF_FFFF) as u32);
            ctrl.evt_dequeue = dq; ctrl.evt_cycle = cy;
            return Some((dw0, dw1, dw2, dw3));
        }
        core::hint::spin_loop();
    }
    None
}

unsafe fn evt_poll_nb(ctrl: &mut XhciController) -> Option<(u32, u32, u32, u32)> {
    let base = hal().phys_to_virt(ctrl.erst_phys) as *const u32;
    let dq = ctrl.evt_dequeue;
    let dw3 = base.add((dq as usize) * 4 + 3).read_volatile();
    if (dw3 & 1) == ctrl.evt_cycle {
        let dw0 = base.add((dq as usize) * 4).read_volatile();
        let dw1 = base.add((dq as usize) * 4 + 1).read_volatile();
        let dw2 = base.add((dq as usize) * 4 + 2).read_volatile();
        let ndq = if dq + 1 >= RING_SIZE as u32 { 0 } else { dq + 1 };
        let ncy = if ndq == 0 { ctrl.evt_cycle ^ 1 } else { ctrl.evt_cycle };
        let erdp = ctrl.erst_phys + (ndq as u64) * (TRB_SIZE as u64);
        w32(ctrl.mmio + ctrl.rt_base as u64 + RT_ERDP as u64, (erdp & 0xFFFF_FFFF) as u32);
        w32(ctrl.mmio + ctrl.rt_base as u64 + RT_ERDP as u64 + 4, ((erdp >> 32) & 0xFFFF_FFFF) as u32);
        ctrl.evt_dequeue = ndq; ctrl.evt_cycle = ncy;
        Some((dw0, dw1, dw2, dw3))
    } else { None }
}

// ═══════════════════════════════════════════════════════════════════
//  Helpers
// ═══════════════════════════════════════════════════════════════════

unsafe fn dcbaa_get(slot: u8) -> Option<u64> {
    let ctrl = CTRL.as_ref()?;
    let a = hal().phys_to_virt(ctrl.dcbaa_phys) as *const u64;
    let v = a.add(slot as usize).read_volatile();
    if v == 0 { None } else { Some(v) }
}

unsafe fn send_cmd(trb: Trb) -> Option<(u32, u32, u32, u32)> {
    let ctrl = match CTRL.as_mut() { Some(c) => c, None => return None };
    ctrl.cmd_ring.enqueue(&trb);
    ring_doorbell(0, 0);
    loop {
        let ev = evt_poll_block(ctrl)?;
        let typ = (ev.3 >> 10) & 0x3F;
        if typ == TRB_COMPLETION {
            let cc = (ev.2 >> 24) & 0xFF;
            if cc == CC_SUCCESS || cc == CC_SHORT { return Some(ev); }
            return None;
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
//  Init  (unchanged logic, proven)
// ═══════════════════════════════════════════════════════════════════

pub unsafe fn init(mmio: u64) -> bool {
    if CTRL.is_some() { return true; }
    let h = hal();
    h.log("[xhci] === INIT ===\n");
    let cap_len = r32(mmio + CAPLENGTH as u64) & 0xFF;
    let hcs1 = r32(mmio + HCSPARAMS1 as u64);
    let hcc1 = r32(mmio + HCCPARAMS1 as u64);
    let op_base = cap_len;
    let rt_off = r32(mmio + RTSOFF as u64) & !0x1F;
    let db_off = r32(mmio + DBOFF as u64) & !0x1F;
    let max_slots = (hcs1 & 0xFF) as u8;
    let max_ports = ((hcs1 >> 24) & 0xFF) as u8;
    let csz = if hcc1 & (1 << 2) != 0 { 1u8 } else { 0u8 };
    h.log_u64(" max_slots=", max_slots as u64);
    h.log_u64(" max_ports=", max_ports as u64);

    let eecp = ((hcc1 >> 8) & 0xFF) as u32;
    if eecp >= 0x40 && (r32(mmio + eecp as u64) & 1) != 0 {
        w32(mmio + eecp as u64 + 4, 1);
        for _ in 0..50000 { if r32(mmio + eecp as u64) & 1 == 0 { break; } }
    }
    let cmd = op_r(mmio, op_base, USBCMD);
    op_w(mmio, op_base, USBCMD, cmd & !USBCMD_RS);
    for _ in 0..50000 { if op_r(mmio, op_base, USBSTS) & USBSTS_HCH != 0 { break; } }
    op_w(mmio, op_base, USBCMD, USBCMD_HCRST);
    for _ in 0..100000 { if op_r(mmio, op_base, USBCMD) & USBCMD_HCRST == 0 { break; } }
    for _ in 0..50000 { if op_r(mmio, op_base, USBSTS) & USBSTS_CNR == 0 { break; } }
    op_w(mmio, op_base, CONFIG, (op_r(mmio, op_base, CONFIG) & !0xFF) | max_slots as u32);

    // DCBAA
    let dp = ((max_slots as usize + 1) * 8 + 4095) / 4096;
    let da = match h.alloc_dma_pages(dp) { Some(p) => p, None => { h.log("[xhci] FAIL dcbaa\n"); return false; } };
    core::ptr::write_bytes(h.phys_to_virt(da), 0, dp * 4096);
    let da2 = da & !0x3F;
    op_w(mmio, op_base, 0x30, (da2 & 0xFFFF_FFFF) as u32);
    op_w(mmio, op_base, 0x34, ((da2 >> 32) & 0xFFFF_FFFF) as u32);

    // Cmd ring
    let cp = (RING_SIZE * TRB_SIZE + 4095) / 4096;
    let ca = match h.alloc_dma_pages(cp) { Some(p) => p, None => { h.log("[xhci] FAIL cmd\n"); return false; } };
    let cv = h.phys_to_virt(ca) as *mut u32;
    core::ptr::write_bytes(cv as *mut u8, 0, cp * 4096);
    let cr_val = (ca & !0x3F) | 1;
    op_w(mmio, op_base, 0x18, (cr_val & 0xFFFF_FFFF) as u32);
    op_w(mmio, op_base, 0x1C, ((cr_val >> 32) & 0xFFFF_FFFF) as u32);

    // Event ring
    let ep = (RING_SIZE * TRB_SIZE + 4095) / 4096;
    let ea = match h.alloc_dma_pages(ep) { Some(p) => p, None => { h.log("[xhci] FAIL evt\n"); return false; } };
    let ev = h.phys_to_virt(ea) as *mut u32;
    core::ptr::write_bytes(ev as *mut u8, 0, ep * 4096);
    let eea = match h.alloc_dma_pages(1) { Some(p) => p, None => { h.log("[xhci] FAIL ERST\n"); return false; } };
    let eev = h.phys_to_virt(eea) as *mut u32;
    core::ptr::write_bytes(eev as *mut u8, 0, 4096);
    eev.add(0).write_volatile((ea & 0xFFFF_FFFF) as u32);
    eev.add(1).write_volatile(((ea >> 32) & 0xFFFF_FFFF) as u32);
    eev.add(2).write_volatile(RING_SIZE as u32);
    rt_w(mmio, rt_off, RT_ERSTSZ, 1);
    rt_w(mmio, rt_off, RT_ERSTBA, (eea & 0xFFFF_FFFF) as u32);
    rt_w(mmio, rt_off, RT_ERSTBA + 4, ((eea >> 32) & 0xFFFF_FFFF) as u32);
    let eo = (ea & !0x3F) as u64;
    rt_w(mmio, rt_off, RT_ERDP, (eo & 0xFFFF_FFFF) as u32);
    rt_w(mmio, rt_off, RT_ERDP + 4, ((eo >> 32) & 0xFFFF_FFFF) as u32);
    rt_w(mmio, rt_off, RT_IMAN, IMAN_IE);

    // Start
    op_w(mmio, op_base, USBCMD, op_r(mmio, op_base, USBCMD) | USBCMD_RS);
    for _ in 0..50000 { if op_r(mmio, op_base, USBSTS) & USBSTS_HCH == 0 { break; } }

    // Build ring wrappers
    let cmd_ring = TransferRing::new(cv, ca & !0x3F);
    let mut event_ring = TransferRing::new(ev, eo);
    event_ring.enable_toggle_cycle(); // xHC manages event ring cycle

    CTRL = Some(XhciController {
        mmio, op_base, rt_base: rt_off, db_base: db_off,
        max_slots, max_ports, ctx_size: csz,
        dcbaa_phys: da2,
        cmd_ring, event_ring,
        erst_phys: eo,
        initialized: true,
        evt_dequeue: 0, evt_cycle: 1,
    });
    h.log("[xhci] INIT DONE\n");
    true
}

// ═══════════════════════════════════════════════════════════════════
//  Port ops
// ═══════════════════════════════════════════════════════════════════

pub unsafe fn port_speed(port: u8) -> u8 {
    let c = match CTRL.as_ref() { Some(c) => c, None => return 0 };
    let pb = c.op_base as u64 + 0x400 + port as u64 * 0x10;
    ((r32(c.mmio + pb + PORTSC as u64) >> 10) & 0x0F) as u8
}

pub unsafe fn port_reset(port: u8) -> bool {
    let c = match CTRL.as_mut() { Some(c) => c, None => return false };
    let pb = c.op_base as u64 + 0x400 + port as u64 * 0x10;
    let sc = r32(c.mmio + pb + PORTSC as u64);
    if sc & PORTSC_CCS == 0 { return false; }
    w32(c.mmio + pb + PORTSC as u64, sc | PORTSC_PR);
    for _ in 0..100000 {
        let s = r32(c.mmio + pb + PORTSC as u64);
        if s & PORTSC_PR == 0 && s & PORTSC_PRC != 0 {
            w32(c.mmio + pb + PORTSC as u64, s | PORTSC_PRC);
            for _ in 0..50000 {
                if r32(c.mmio + pb + PORTSC as u64) & PORTSC_PED != 0 { return true; }
                core::hint::spin_loop();
            }
            break;
        }
        core::hint::spin_loop();
    }
    false
}

// ═══════════════════════════════════════════════════════════════════
//  Enable Slot
// ═══════════════════════════════════════════════════════════════════

pub unsafe fn enable_slot() -> Option<u8> {
    hal().log("[xhci] enable_slot\n");
    let ev = send_cmd(Trb { dw0: 0, dw1: 0, dw2: 0, dw3: TRB_ENABLE << 10 })?;
    let cc = (ev.2 >> 24) & 0xFF;
    let slot = ((ev.3 >> 24) & 0xFF) as u8;
    hal().log_u64(" cc=", cc as u64);
    hal().log_u64(" slot=", slot as u64);
    if cc != CC_SUCCESS || slot == 0 { None } else { Some(slot) }
}

// ═══════════════════════════════════════════════════════════════════
//  Per-slot EP0 ring storage
// ═══════════════════════════════════════════════════════════════════

const MAX_SLOTS: usize = 255;
#[derive(Clone, Copy)]
struct Ep0Info { valid: bool, ring_phys: u64, ring_virt: *mut u32, pcs: bool, enqueue: usize }
static mut EP0_RINGS: [Ep0Info; MAX_SLOTS] = [Ep0Info {
    valid: false, ring_phys: 0, ring_virt: core::ptr::null_mut(), pcs: true, enqueue: 0
}; MAX_SLOTS];
unsafe fn ep0_reg(slot: u8, phys: u64, virt: *mut u32) {
    EP0_RINGS[slot as usize] = Ep0Info { valid: true, ring_phys: phys, ring_virt: virt, pcs: true, enqueue: 0 };
}
fn ep0_mut(slot: u8) -> Option<&'static mut Ep0Info> {
    if (slot as usize) < MAX_SLOTS { unsafe { let p = &mut EP0_RINGS[slot as usize]; if p.valid { Some(p) } else { None } } }
    else { None }
}

// ═══════════════════════════════════════════════════════════════════
//  Address Device
// ═══════════════════════════════════════════════════════════════════

pub unsafe fn address_device(port: u8, speed: u8) -> Option<u8> {
    let ctrl = match CTRL.as_mut() { Some(c) => c, None => return None };
    let h = hal();
    let slot = enable_slot()?;
    let cs = ctx_sz(ctrl);

    let ep0_phys = h.alloc_dma_pages(1)?;
    let ep0_virt = h.phys_to_virt(ep0_phys) as *mut u32;
    core::ptr::write_bytes(ep0_virt as *mut u8, 0, 4096);
    let _ring = TransferRing::new(ep0_virt, ep0_phys);
    ep0_reg(slot, ep0_phys & !0xF, ep0_virt);

    let in_phys = h.alloc_dma_pages(1)?;
    let in_virt = h.phys_to_virt(in_phys) as *mut u8;
    core::ptr::write_bytes(in_virt, 0, 4096);
    let dev_phys = h.alloc_dma_pages(1)?;
    let dev_virt = h.phys_to_virt(dev_phys) as *mut u8;
    core::ptr::write_bytes(dev_virt, 0, 4096);

    // Input Control Context
    let in32 = in_virt as *mut u32;
    in32.add(0).write_volatile(0); // Drop
    in32.add(1).write_volatile(3); // Add Slot+EP0

    // Slot Context
    let sc = in_virt.add(cs) as *mut u32;
    sc.add(0).write_volatile(((speed as u32) & 0xF) << 20 | (1 << 27));
    sc.add(1).write_volatile((port as u32 + 1) << 16);

    let mps: u32 = match speed { 1|2 => 8, 3 => 64, 4|5 => 512, _ => 8 };
    let dq = (ep0_phys & !0xF) | 1;

    // EP0 Context
    let ep0 = in_virt.add(2 * cs) as *mut u32;
    ep0.add(0).write_volatile(0);
    ep0.add(1).write_volatile((mps << 16) | (4 << 3) | (3 << 1));
    ep0.add(2).write_volatile((dq & 0xFFFF_FFFF) as u32);
    ep0.add(3).write_volatile(((dq >> 32) & 0xFFFF_FFFF) as u32);
    ep0.add(4).write_volatile(8);

    // DCBAA[slot]
    let dcbaa = h.phys_to_virt(ctrl.dcbaa_phys) as *mut u64;
    dcbaa.add(slot as usize).write_volatile(dev_phys & !0x3F);

    // Address Device TRB
    let trb = Trb {
        dw0: (in_phys & 0xFFFF_FFFF) as u32,
        dw1: ((in_phys >> 32) & 0xFFFF_FFFF) as u32,
        dw2: 0,
        dw3: ((slot as u32) << 24) | (TRB_ADDRESS_DEV << 10),
    };
    ctrl.cmd_ring.enqueue(&trb);
    ring_doorbell(0, 0);
    let ev = evt_poll_block(ctrl)?;
    let cc = (ev.2 >> 24) & 0xFF;
    h.log_u64(" addr_dev cc=", cc as u64);
    if cc != CC_SUCCESS { return None; }

    // Write EP0 dequeue into Device Context EP0 for future doorbell reloads
    let d_ep0 = dev_virt.add(cs) as *mut u32;
    d_ep0.add(2).write_volatile((ep0_phys & !0xF) as u32 | 1);
    d_ep0.add(3).write_volatile(((ep0_phys >> 32) & 0xFFFF_FFFF) as u32);

    Some(slot)
}

// ═══════════════════════════════════════════════════════════════════
//  Control Transfer — uses per-slot EP0 ring
// ═══════════════════════════════════════════════════════════════════

pub unsafe fn control_transfer(slot: u8, bm_req_type: u8, b_request: u8,
    w_value: u16, w_index: u16, buf: &mut [u8], data_in: bool) -> usize
{
    let ctrl = match CTRL.as_mut() { Some(c) => c, None => return 0 };
    let h = hal();
    let ep0 = match ep0_mut(slot) { Some(e) => e, None => { h.log("no ep0 ring\n"); return 0; } };

    let has_data = !buf.is_empty();
    let data_page = if has_data {
        let dp = h.alloc_dma_pages(1).unwrap_or(0);
        if dp == 0 { return 0; }
        if !data_in {
            let dv = h.phys_to_virt(dp);
            for i in 0..buf.len() { dv.add(i).write_volatile(buf[i]); }
        }
        dp
    } else { 0 };

    let trt = if !has_data { 0u32 } else if data_in { 3u32 } else { 2u32 };

    // Setup Stage
    let setup = Trb {
        dw0: (bm_req_type as u32) | ((b_request as u32) << 8) | ((w_value as u32) << 16),
        dw1: (w_index as u32) | ((buf.len() as u32) << 16),
        dw2: 8,
        dw3: (TRB_SETUP << 10) | (1 << 6) | (trt << 16) | (1 << 4), // IDT, CH
    };
    let s_idx = ep0.enqueue;
    let sb = s_idx * 4;
    ep0.ring_virt.add(sb).write_volatile(setup.dw0);
    ep0.ring_virt.add(sb + 1).write_volatile(setup.dw1);
    ep0.ring_virt.add(sb + 2).write_volatile(setup.dw2);
    ep0.ring_virt.add(sb + 3).write_volatile(setup.dw3 | if ep0.pcs { 1 } else { 0 });
    ep0.enqueue = s_idx + 1;
    if ep0.enqueue >= LAST_TRB_IDX { ep0.enqueue = 0; ep0.pcs = !ep0.pcs; }

    // Data Stage
    if has_data {
        let d_idx = ep0.enqueue;
        let db = d_idx * 4;
        ep0.ring_virt.add(db).write_volatile((data_page & 0xFFFF_FFFF) as u32);
        ep0.ring_virt.add(db + 1).write_volatile(((data_page >> 32) & 0xFFFF_FFFF) as u32);
        ep0.ring_virt.add(db + 2).write_volatile(buf.len() as u32 & 0x1FFFF);
        let dir = if data_in { 1u32 << 16 } else { 0 };
        ep0.ring_virt.add(db + 3).write_volatile(
            (TRB_DATA << 10) | dir | (1 << 4) | if ep0.pcs { 1 } else { 0 });
        ep0.enqueue = d_idx + 1;
        if ep0.enqueue >= LAST_TRB_IDX { ep0.enqueue = 0; ep0.pcs = !ep0.pcs; }
    }

    // Status Stage
    let st_idx = ep0.enqueue;
    let stb = st_idx * 4;
    ep0.ring_virt.add(stb).write_volatile(0);
    ep0.ring_virt.add(stb + 1).write_volatile(0);
    ep0.ring_virt.add(stb + 2).write_volatile(0);
    let dir_in = if has_data { !data_in } else { true };
    ep0.ring_virt.add(stb + 3).write_volatile(
        (TRB_STATUS << 10) | (if dir_in { 1u32 << 16 } else { 0 }) | (1 << 5)
        | if ep0.pcs { 1 } else { 0 }
    );
    ep0.enqueue = st_idx + 1;
    if ep0.enqueue >= LAST_TRB_IDX { ep0.enqueue = 0; ep0.pcs = !ep0.pcs; }

    // Ring EP0 doorbell
    ring_doorbell(slot, 1);

    // Wait for Transfer Event
    loop {
        let ev = evt_poll_block(ctrl);
        if ev.is_none() { return 0; }
        let (_, _, dw2, dw3) = ev.unwrap();
        let typ = (dw3 >> 10) & 0x3F;
        if typ == TRB_TRANSFER {
            let eslot = ((dw3 >> 24) & 0xFF) as u8;
            let eepid = ((dw3 >> 16) & 0x1F) as u8;
            if eslot == slot && eepid == 1 {
                let cc = (dw2 >> 24) & 0xFF;
                if cc == CC_SUCCESS || cc == CC_SHORT {
                    let rem = dw2 & 0xFFFFFF;
                    let xfer = buf.len().saturating_sub(rem as usize);
                    if data_in && has_data && data_page != 0 {
                        let dv = h.phys_to_virt(data_page);
                        for i in 0..xfer.min(buf.len()) { buf[i] = dv.add(i).read_volatile(); }
                    }
                    return xfer;
                }
                h.log_u64(" ctrl_xfer cc=", cc as u64);
                return 0;
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
//  Descriptor helpers
// ═══════════════════════════════════════════════════════════════════

pub unsafe fn get_device_descriptor(slot: u8, buf: &mut [u8]) -> usize {
    let len = if buf.len() > 18 { 18 } else { buf.len() };
    control_transfer(slot, 0x80, USB_REQ_GET_DESCRIPTOR, USB_DESC_DEVICE << 8, 0, &mut buf[..len], true)
}

pub unsafe fn get_config_descriptor(slot: u8, index: u8, buf: &mut [u8]) -> usize {
    control_transfer(slot, 0x80, USB_REQ_GET_DESCRIPTOR, (USB_DESC_CONFIG << 8) | index as u16, 0, buf, true)
}

// ═══════════════════════════════════════════════════════════════════
//  Per-endpoint transfer ring storage (for non-EP0 endpoints)
// ═══════════════════════════════════════════════════════════════════

const MAX_DCI: usize = 32;
#[derive(Clone, Copy)]
struct EpRing { valid: bool, ring_phys: u64, ring_virt: *mut u32, pcs: bool, enqueue: usize }
static mut EP_RINGS: [[EpRing; MAX_DCI]; MAX_SLOTS] = [[EpRing {
    valid: false, ring_phys: 0, ring_virt: core::ptr::null_mut(), pcs: true, enqueue: 0
}; MAX_DCI]; MAX_SLOTS];
fn ep_ring_mut(slot: u8, dci: u8) -> Option<&'static mut EpRing> {
    if (slot as usize) < MAX_SLOTS && (dci as usize) < MAX_DCI {
        unsafe { let p = &mut EP_RINGS[slot as usize][dci as usize]; if p.valid { Some(p) } else { None } }
    } else { None }
}

// ═══════════════════════════════════════════════════════════════════
//  Configure Endpoint
// ═══════════════════════════════════════════════════════════════════

pub unsafe fn configure_endpoint(slot: u8, dci: u8, ep_type: u8, max_pkt: u16, interval: u8) -> bool {
    let ctrl = match CTRL.as_mut() { Some(c) => c, None => return false };
    let h = hal();
    let cs = ctx_sz(ctrl);

    let in_phys = match h.alloc_dma_pages(1) { Some(p) => p, None => { h.log("[xhci] cfg_ep: no mem\n"); return false; } };
    let in_virt = h.phys_to_virt(in_phys) as *mut u8;
    core::ptr::write_bytes(in_virt, 0, 4096);

    let in32 = in_virt as *mut u32;
    in32.add(0).write_volatile(0); // Drop
    in32.add(1).write_volatile((1u32 << dci) | 1); // Add Slot(0) + EP(dci)

    // Slot Context: preserve the controller-populated route/speed/root-port
    // fields from the output Device Context; only raise Context Entries.
    let sc = in_virt.add(cs) as *mut u32;
    let dev_phys = match dcbaa_get(slot) { Some(p) => p, None => { h.log("[xhci] cfg_ep: no dev ctx\n"); return false; } };
    let dev_virt = h.phys_to_virt(dev_phys) as *const u32;
    let slot_dwords = cs / 4;
    for i in 0..slot_dwords {
        sc.add(i).write_volatile(dev_virt.add(i).read_volatile());
    }
    let old_dw0 = sc.add(0).read_volatile();
    let old_entries = (old_dw0 >> 27) & 0x1F;
    let new_entries = old_entries.max(dci as u32);
    sc.add(0).write_volatile((old_dw0 & !(0x1F << 27)) | (new_entries << 27));

    // Allocate transfer ring for this endpoint
    let tr_phys = match h.alloc_dma_pages(1) { Some(p) => p, None => { h.log("[xhci] cfg_ep: no ring\n"); return false; } };
    let tr_virt = h.phys_to_virt(tr_phys) as *mut u32;
    core::ptr::write_bytes(tr_virt as *mut u8, 0, 4096);
    TransferRing::new(tr_virt, tr_phys); // writes Link TRB
    if (slot as usize) < MAX_SLOTS && (dci as usize) < MAX_DCI {
        EP_RINGS[slot as usize][dci as usize] = EpRing {
            valid: true, ring_phys: tr_phys & !0xF, ring_virt: tr_virt, pcs: true, enqueue: 0,
        };
    }

    let dq = (tr_phys & !0xF) | 1;
    let ep = in_virt.add((dci as usize + 1) * cs) as *mut u32;
    ep.add(0).write_volatile((interval as u32) << 16); // DW0: Interval in bits 23:16
    ep.add(1).write_volatile(
        ((max_pkt as u32) << 16) | ((ep_type as u32) << 3) | (3 << 1)
    );
    ep.add(2).write_volatile((dq & 0xFFFF_FFFF) as u32);
    ep.add(3).write_volatile(((dq >> 32) & 0xFFFF_FFFF) as u32);
    ep.add(4).write_volatile(8);

    let trb = Trb {
        dw0: (in_phys & 0xFFFF_FFFF) as u32,
        dw1: ((in_phys >> 32) & 0xFFFF_FFFF) as u32,
        dw2: 0,
        dw3: ((slot as u32) << 24) | (TRB_CONFIGURE << 10),
    };
    ctrl.cmd_ring.enqueue(&trb);
    ring_doorbell(0, 0);
    let ev = evt_poll_block(ctrl);
    match ev {
        Some((_, _, dw2, _)) => (dw2 >> 24) & 0xFF == CC_SUCCESS,
        None => false
    }
}

// ═══════════════════════════════════════════════════════════════════
//  Queue interrupt IN transfer
// ═══════════════════════════════════════════════════════════════════

pub unsafe fn queue_interrupt_in(slot: u8, dci: u8, buf_phys: u64, len: u16) -> bool {
    let ring = match ep_ring_mut(slot, dci) { Some(r) => r, None => return false };
    let idx = ring.enqueue;
    let b = idx * 4;
    ring.ring_virt.add(b).write_volatile((buf_phys & 0xFFFF_FFFF) as u32);
    ring.ring_virt.add(b + 1).write_volatile(((buf_phys >> 32) & 0xFFFF_FFFF) as u32);
    ring.ring_virt.add(b + 2).write_volatile(len as u32);
    let ctl = (TRB_NORMAL << 10) | (1 << 5); // IOC
    ring.ring_virt.add(b + 3).write_volatile(ctl | if ring.pcs { 1 } else { 0 });
    ring.enqueue = idx + 1;
    if ring.enqueue >= LAST_TRB_IDX { ring.enqueue = 0; ring.pcs = !ring.pcs; }
    true
}

// ═══════════════════════════════════════════════════════════════════
//  Public non-blocking event poll
// ═══════════════════════════════════════════════════════════════════

/// Returns (slot, endpoint_id, cc) for the next transfer event, or None.
/// Ring doorbell for a slot+endpoint. EP0=1, EP1 OUT=2, EP1 IN=3, etc.
pub unsafe fn ring_doorbell(slot: u8, endpoint_id: u8) {
    if let Some(c) = CTRL.as_ref() {
        let db_addr = c.mmio + c.db_base as u64 + (slot as u64) * 4;
        w32(db_addr, endpoint_id as u32);
    }
}

pub unsafe fn poll_transfer_event() -> Option<(u8, u8, u8)> {
    let ctrl = match CTRL.as_mut() { Some(c) => c, None => return None };
    loop {
        let ev = evt_poll_nb(ctrl)?;
        let typ = (ev.3 >> 10) & 0x3F;
        if typ == TRB_TRANSFER {
            return Some((((ev.3 >> 24) & 0xFF) as u8, ((ev.3 >> 16) & 0x1F) as u8, (ev.2 >> 24) as u8));
        }
        if typ == TRB_COMPLETION || typ == TRB_PORT_STATUS { continue; }
    }
}
