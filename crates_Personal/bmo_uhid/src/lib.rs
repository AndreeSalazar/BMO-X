//! USB HID driver — full lifecycle: descriptor reading, Configure Endpoint,
//! interrupt transfers, HID boot protocol parsing for keyboard + mouse.

#![no_std]
extern crate alloc;

use bmo_input::hal::{InputHal, PointerMode};
use bmo_input::event::InputEvent;
use alloc::vec::Vec;

// ── HID boot protocol report structures ──────────────────────

#[repr(C)]
#[derive(Clone, Copy)]
struct KbdReport { modifiers: u8, _reserved: u8, keys: [u8; 6] }

#[repr(C)]
#[derive(Clone, Copy)]
struct MouseReport { buttons: u8, dx: i8, dy: i8, wheel: i8 }

// ── USB HID usage → PS/2 Set 1 scancode ─────────────────────

static HID_TO_PS2: [u8; 104] = [
    0,0,0,0, 0x1E,0x30,0x2E,0x20,0x12,0x21,0x22,0x23,
    0x17,0x24,0x25,0x26,0x32,0x31,0x18,0x19,0x10,0x13,
    0x1F,0x14,0x16,0x2F,0x11,0x2D,0x15,0x2C,
    0x02,0x03,0x04,0x05,0x06,0x07,0x08,0x09,0x0A,0x0B,
    0x1C,0x01,0x0E,0x0F,0x39,0x0C,0x0D,0x1A,0x1B,0x2B,0,
    0x27,0x28,0x29,0x33,0x34,0x35,
    0x3A,0x3B,0x3C,0x3D,0x3E,0x3F,0x40,0x41,0x42,0x43,0x44,0x57,0x58,
    0x37,0x46,0x45,0x52,0x47,0x49,0x53,0x4F,0x51,0x4D,0x4B,0x50,0x48,0x45,
    0x35,0x37,0x4A,0x4E,0x1C,0x4F,0x50,0x51,0x4B,0x4C,0x4D,0x47,0x48,0x49,0x52,0x53,
    0,0,0,0,
];

fn hid_to_ps2(usage: u8) -> Option<u8> {
    let idx = usage as usize;
    if idx < HID_TO_PS2.len() { let v = HID_TO_PS2[idx]; if v != 0 { Some(v) } else { None } }
    else { None }
}

// ── Modifier bits ───────────────────────────────────────────

const MOD_LCTRL: u8 = 1<<0; const MOD_LSHIFT: u8 = 1<<1;
const MOD_LALT: u8 = 1<<2; const MOD_LGUI: u8 = 1<<3;
const MOD_RCTRL: u8 = 1<<4; const MOD_RSHIFT: u8 = 1<<5;
const MOD_RALT: u8 = 1<<6; const MOD_RGUI: u8 = 1<<7;

// ── Peripheral tracking ─────────────────────────────────────

struct KbdDev {
    slot: u8,
    dci: u8,
    buf_phys: u64,
    buf_virt: *mut u8,
    prev_mod: u8,
    prev_keys: [u8; 6],
    queued: bool,
}

struct MouseDev {
    slot: u8,
    dci: u8,
    buf_phys: u64,
    buf_virt: *mut u8,
    prev_buttons: u8,
    queued: bool,
}

// ── UsbHidHal ───────────────────────────────────────────────

pub struct UsbHidHal {
    kbd: Option<KbdDev>,
    mouse: Option<MouseDev>,
    initialized: bool,
}

impl UsbHidHal {
    pub const fn new() -> Self {
        Self { kbd: None, mouse: None, initialized: false }
    }

    // ── Parsing helpers ─────────────────────────────────────

    /// Read 2 bytes from a slice as little-endian u16.
    fn le_u16(buf: &[u8], off: usize) -> u16 {
        (buf[off] as u16) | ((buf[off + 1] as u16) << 8)
    }

    /// Parse interface descriptors from a full config descriptor set.
    /// Returns (interface_number, class, subclass, protocol) for each.
    fn parse_interfaces(cfg: &[u8]) -> Vec<(u8, u8, u8, u8)> {
        let mut ifs = Vec::new();
        let total = if cfg.len() >= 2 { Self::le_u16(cfg, 2) as usize } else { 0 };
        let limit = if total > 0 && total <= cfg.len() { total } else { cfg.len() };
        let mut off = if cfg.len() >= 1 { cfg[0] as usize } else { 9 };
        while off + 3 <= limit {
            let len = cfg[off] as usize;
            let dtype = cfg[off + 1];
            if len < 2 || off + len > limit { break; }
            if dtype == 4 && len >= 9 {
                ifs.push((cfg[off + 2], cfg[off + 5], cfg[off + 6], cfg[off + 7]));
            }
            off += len;
        }
        ifs
    }

    /// Find interrupt IN endpoint for a given interface.
    /// Returns (endpoint_address, max_packet_size, interval, dci).
    fn find_intr_in(cfg: &[u8], iface_num: u8) -> Option<(u8, u16, u8, u8)> {
        let total = if cfg.len() >= 2 { Self::le_u16(cfg, 2) as usize } else { 0 };
        let limit = if total > 0 && total <= cfg.len() { total } else { cfg.len() };
        let mut off = if cfg.len() >= 1 { cfg[0] as usize } else { 9 };
        let mut current_iface = 0u8;
        while off + 3 <= limit {
            let len = cfg[off] as usize;
            let dtype = cfg[off + 1];
            if len < 2 || off + len > limit { break; }
            if dtype == 4 && len >= 9 { current_iface = cfg[off + 2]; }
            if dtype == 5 && len >= 7 && current_iface == iface_num {
                let ep_addr = cfg[off + 2];
                let attr = cfg[off + 3];
                let mps = Self::le_u16(cfg, off + 4);
                let interval = cfg[off + 6];
                // IN direction + Interrupt transfer type (bits 1:0 = 3)
                if (ep_addr & 0x80) != 0 && (attr & 3) == 3 {
                    let ep_num = ep_addr & 0x0F;
                    let dci = if ep_num == 0 { 1 } else { ep_num * 2 + 1 };
                    return Some((ep_addr, mps, interval, dci));
                }
            }
            off += len;
        }
        None
    }
}

// ═══════════════════════════════════════════════════════════════
//  InputHal impl
// ═══════════════════════════════════════════════════════════════

impl InputHal for UsbHidHal {
    fn init(&mut self) -> bool {
        if self.initialized { return true; }

        // Initialize xHCI controller if needed
        if !bmo_xhci::is_controller_initialized() {
            let mmio = match bmo_xhci::get_mmio() { Some(m) => m, None => return false };
            if !unsafe { bmo_xhci::init(mmio) } { return false; }
        }

        let ctrl = match bmo_xhci::controller() { Some(c) => c, None => return false };
        let h = bmo_xhci::hal();

        // ── Enumerate ports ──
        for port in 0..ctrl.max_ports {
            unsafe {
                if !bmo_xhci::port_reset(port) { continue; }
                let speed = bmo_xhci::port_speed(port);
                if speed == 0 { continue; }

                let slot = match bmo_xhci::address_device(port, speed) {
                    Some(s) => s, None => continue,
                };
                h.log_u64("[uhid] slot=", slot as u64);

                // Read device descriptor (18 bytes)
                let mut dev_desc = [0u8; 18];
                let n = bmo_xhci::get_device_descriptor(slot, &mut dev_desc);
                if n < 8 { h.log("[uhid] no dev desc\n"); continue; }
                let dev_class = dev_desc[4];
                h.log_u64(" class=", dev_class as u64);

                // Read config descriptor header (9 bytes first for total length)
                let mut cfg_hdr = [0u8; 9];
                let n2 = bmo_xhci::get_config_descriptor(slot, 0, &mut cfg_hdr);
                if n2 < 9 { h.log("[uhid] no cfg hdr\n"); continue; }
                let total_len = Self::le_u16(&cfg_hdr, 2) as usize;
                let cfg_val = cfg_hdr[5];
                h.log_u64(" cfg_val=", cfg_val as u64);
                h.log_u64(" total_len=", total_len as u64);

                // Read full config descriptor
                if total_len > 512 { h.log("[uhid] cfg too big\n"); continue; }
                let mut cfg_full = Vec::new();
                cfg_full.resize(total_len, 0u8);
                let n3 = bmo_xhci::get_config_descriptor(slot, 0, &mut cfg_full);
                if n3 < total_len { h.log("[uhid] cfg short\n"); continue; }

                // Parse interfaces
                let ifs = Self::parse_interfaces(&cfg_full);
                let mut found_kbd = false;
                let mut found_mouse = false;

                for (iface_num, class, subclass, protocol) in &ifs {
                    // HID class = 3
                    if *class != 3 { continue; }

                    // Keyboard: subclass=1, protocol=1
                    // Mouse: subclass=1, protocol=2
                    let is_kbd = *subclass == 1 && *protocol == 1 && self.kbd.is_none();
                    let is_mouse = *subclass == 1 && *protocol == 2 && self.mouse.is_none();

                    if !is_kbd && !is_mouse { continue; }

                    // Find interrupt IN endpoint
                    if let Some((ep_addr, mps, interval, dci)) =
                        Self::find_intr_in(&cfg_full, *iface_num)
                    {
                        h.log_u64(" found ep addr=", ep_addr as u64);
                        h.log_u64(" mps=", mps as u64);
                        h.log_u64(" dci=", dci as u64);

                        // SET_CONFIGURATION
                        bmo_xhci::control_transfer(slot, 0x00, 0x09, cfg_val as u16, 0, &mut [], false);

                        // Configure Endpoint in xHCI
                        if !bmo_xhci::configure_endpoint(slot, dci, 7, mps, interval) {
                            h.log("[uhid] cfg_ep FAIL\n"); continue;
                        }

                        // HID SET_PROTOCOL(boot)
                        bmo_xhci::control_transfer(slot, 0x21, 0x0B, 0, *iface_num as u16, &mut [], false);

                        // HID SET_IDLE(0)
                        bmo_xhci::control_transfer(slot, 0x21, 0x0A, 0, *iface_num as u16, &mut [], false);

                        // Allocate DMA buffer for interrupt reports
                        let buf_size = if is_kbd { 8usize } else { 4usize };
                        let buf_phys = match h.alloc_dma_pages(1) { Some(p) => p, None => continue };
                        let buf_virt = h.phys_to_virt(buf_phys);
                        core::ptr::write_bytes(buf_virt, 0, 4096);

                        if is_kbd {
                            bmo_xhci::queue_interrupt_in(slot, dci, buf_phys, buf_size as u16);
                            bmo_xhci::ring_doorbell(slot, dci);
                            self.kbd = Some(KbdDev {
                                slot, dci, buf_phys, buf_virt,
                                prev_mod: 0, prev_keys: [0; 6], queued: true,
                            });
                            found_kbd = true;
                            h.log("[uhid] kbd ready\n");
                        } else {
                            bmo_xhci::queue_interrupt_in(slot, dci, buf_phys, buf_size as u16);
                            bmo_xhci::ring_doorbell(slot, dci);
                            self.mouse = Some(MouseDev {
                                slot, dci, buf_phys, buf_virt,
                                prev_buttons: 0, queued: true,
                            });
                            found_mouse = true;
                            h.log("[uhid] mouse ready\n");
                        }
                    }
                }

                if found_kbd && found_mouse { break; }
            }
        }

        self.initialized = true;
        self.kbd.is_some()
    }

    fn name(&self) -> &'static str { "USB-HID" }

    fn poll(&mut self, buf: &mut [InputEvent]) -> usize {
        if !self.initialized { return 0; }
        let mut count = 0usize;

        unsafe {
            // ── Poll transfer events (non-blocking) ──
            while let Some((ev_slot, ev_ep, cc)) = bmo_xhci::poll_transfer_event() {
                // Keyboard
                if let Some(ref mut k) = self.kbd {
                    if ev_slot == k.slot && ev_ep == k.dci {
                        k.queued = false;
                        if cc == 1 || cc == 13 {
                            let report = core::ptr::read_volatile(k.buf_virt as *const KbdReport);
                            // Diff modifiers
                            let mod_chg = report.modifiers ^ k.prev_mod;
                            if mod_chg & MOD_LCTRL != 0 {
                                let on = report.modifiers & MOD_LCTRL != 0;
                                if count < buf.len() { buf[count] = InputEvent::key(0x1D, on); count += 1; }
                            }
                            if mod_chg & MOD_LSHIFT != 0 {
                                let on = report.modifiers & MOD_LSHIFT != 0;
                                if count < buf.len() { buf[count] = InputEvent::key(0x2A, on); count += 1; }
                            }
                            if mod_chg & MOD_RSHIFT != 0 {
                                let on = report.modifiers & MOD_RSHIFT != 0;
                                if count < buf.len() { buf[count] = InputEvent::key(0x36, on); count += 1; }
                            }
                            if mod_chg & MOD_LALT != 0 {
                                let on = report.modifiers & MOD_LALT != 0;
                                if count < buf.len() { buf[count] = InputEvent::key(0x38, on); count += 1; }
                            }
                            if mod_chg & MOD_RCTRL != 0 || mod_chg & MOD_RCTRL != 0 {
                                // right ctrl = left ctrl scancode for now
                            }

                            // Diff keys
                            for &k_prev in &k.prev_keys {
                                if k_prev == 0 { continue; }
                                if !report.keys.contains(&k_prev) {
                                    if let Some(ps2) = hid_to_ps2(k_prev) {
                                        if count < buf.len() { buf[count] = InputEvent::key(ps2, false); count += 1; }
                                    }
                                }
                            }
                            for &k_new in &report.keys {
                                if k_new == 0 { continue; }
                                if !k.prev_keys.contains(&k_new) {
                                    if let Some(ps2) = hid_to_ps2(k_new) {
                                        if count < buf.len() { buf[count] = InputEvent::key(ps2, true); count += 1; }
                                    }
                                }
                            }

                            k.prev_mod = report.modifiers;
                            k.prev_keys = report.keys;
                        }
                        // Re-queue
                        bmo_xhci::queue_interrupt_in(k.slot, k.dci, k.buf_phys, 8);
                        bmo_xhci::ring_doorbell(k.slot, k.dci);
                        k.queued = true;
                    }
                }

                // Mouse
                if let Some(ref mut m) = self.mouse {
                    if ev_slot == m.slot && ev_ep == m.dci {
                        m.queued = false;
                        if cc == 1 || cc == 13 {
                            let report = core::ptr::read_volatile(m.buf_virt as *const MouseReport);
                            if report.dx != 0 || report.dy != 0 {
                                if count < buf.len() {
                                    buf[count] = InputEvent::mouse_move(report.dx as i16, report.dy as i16);
                                    count += 1;
                                }
                            }
                            if report.buttons != m.prev_buttons {
                                if count < buf.len() {
                                    buf[count] = InputEvent::mouse_button(report.buttons);
                                    count += 1;
                                }
                                m.prev_buttons = report.buttons;
                            }
                        }
                        // Re-queue
                        bmo_xhci::queue_interrupt_in(m.slot, m.dci, m.buf_phys, 4);
                        bmo_xhci::ring_doorbell(m.slot, m.dci);
                        m.queued = true;
                    }
                }
            }
        }

        count
    }

    fn pointer_mode(&self) -> PointerMode { PointerMode::Relative }
    fn is_ready(&self) -> bool { self.initialized }
}
