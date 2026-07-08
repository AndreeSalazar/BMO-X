//! USB HID driver — boot protocol keyboard (8-byte) + mouse (4-byte).
//!
//! Polls devices via GET_REPORT control transfers and translates
//! USB HID usage IDs to PS/2 Set 1 scancodes so the existing
//! desktop input layer works unchanged.
//!
//! Implements `InputHal` trait.

#![no_std]

use bmo_input::hal::{InputHal, PointerMode};
use bmo_input::event::InputEvent;

// ── HID boot protocol report structures ──────────────────────

#[repr(C)]
struct KbdReport {
    modifiers: u8,
    _reserved: u8,
    keys: [u8; 6],
}

#[repr(C)]
struct MouseReport {
    buttons: u8,
    dx: i8,
    dy: i8,
    wheel: i8,
}

// ── USB HID usage → PS/2 Set 1 scancode translation ─────────
// Indexed by USB HID usage ID.  Zero = unmapped / reserved.
static HID_TO_PS2: [u8; 104] = [
    // 0x00-0x03  (Reserved / error roll-over)
    0, 0, 0, 0,
    // 0x04-0x1D  (A – Z)
    0x1E, 0x30, 0x2E, 0x20, 0x12, 0x21, 0x22, 0x23,   // A-H
    0x17, 0x24, 0x25, 0x26, 0x32, 0x31, 0x18, 0x19,   // I-P
    0x10, 0x13, 0x1F, 0x14, 0x16, 0x2F, 0x11, 0x2D,   // Q-X
    0x15, 0x2C,                                         // Y-Z
    // 0x1E-0x27  (1 – 0)
    0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B,
    // 0x28-0x31  (Enter – backslash)
    0x1C, 0x01, 0x0E, 0x0F, 0x39, 0x0C, 0x0D, 0x1A, 0x1B, 0x2B,
    // 0x32       (non-US #) → unmapped
    0,
    // 0x33-0x38  (; – /)
    0x27, 0x28, 0x29, 0x33, 0x34, 0x35,
    // 0x39-0x45  (CapsLock, F1 – F12)
    0x3A, 0x3B, 0x3C, 0x3D, 0x3E, 0x3F,
    0x40, 0x41, 0x42, 0x43, 0x44, 0x57, 0x58,
    // 0x46-0x52  (PrintScreen – UpArrow)
    //    mapped to the equivalent keypad scancode (works even
    //    when the keypad is on a separate device).
    0x37, 0x46, 0x45, 0x52, 0x47, 0x49,
    0x53, 0x4F, 0x51, 0x4D, 0x4B, 0x50, 0x48,
    // 0x53       (NumLock)
    0x45,
    // 0x54-0x63  (Keypad / – Keypad 0)
    0x35, 0x37, 0x4A, 0x4E, 0x1C,           // KP/ * - + Enter
    0x4F, 0x50, 0x51, 0x4B, 0x4C, 0x4D,    // KP1-6
    0x47, 0x48, 0x49, 0x52, 0x53,           // KP7-9 . 0
    // 0x64       (non-US \) → unmapped
    0,
    // 0x65-0x67  (App, Power, KP=)
    0, 0, 0,
];

fn hid_to_ps2(usage: u8) -> Option<u8> {
    let idx = usage as usize;
    if idx < HID_TO_PS2.len() {
        let v = HID_TO_PS2[idx];
        if v != 0 { Some(v) } else { None }
    } else {
        None
    }
}

// ── Modifier USB → PS/2 ─────────────────────────────────────
// Bit positions in byte 0 of the keyboard report.
const MOD_LCTRL:  u8 = 1 << 0;
const MOD_LSHIFT: u8 = 1 << 1;
const MOD_LALT:   u8 = 1 << 2;
const MOD_LGUI:   u8 = 1 << 3;
const MOD_RCTRL:  u8 = 1 << 4;
const MOD_RSHIFT: u8 = 1 << 5;
const MOD_RALT:   u8 = 1 << 6;
const MOD_RGUI:   u8 = 1 << 7;

// PS/2 scancodes for modifiers.
const PS2_LCTRL:  u8 = 0x1D;
const PS2_LSHIFT: u8 = 0x2A;
const PS2_LALT:   u8 = 0x38;
const PS2_RSHIFT: u8 = 0x36;

// ── UsbHidHal ────────────────────────────────────────────────

pub struct UsbHidHal {
    kbd_slot: Option<u8>,
    mouse_slot: Option<u8>,
    prev_mod: u8,
    prev_keys: [u8; 6],
    prev_buttons: u8,
    initialized: bool,
}

impl UsbHidHal {
    pub const fn new() -> Self {
        Self {
            kbd_slot: None, mouse_slot: None,
            prev_mod: 0, prev_keys: [0; 6], prev_buttons: 0,
            initialized: false,
        }
    }

    /// Emit a key event for a modifier change.
    unsafe fn emit_mod(&self, _bit: u8, ps2_sc: u8, on: bool, buf: &mut [InputEvent], count: &mut usize) {
        if *count < buf.len() {
            buf[*count] = InputEvent::key(ps2_sc, on);
            *count += 1;
        }
    }

    /// Diff the new 6-key array against `prev`; emit events for changes.
    unsafe fn diff_keys(
        prev: &[u8; 6], new_keys: &[u8; 6],
        buf: &mut [InputEvent], count: &mut usize,
    ) {
        // Keys that were in prev but NOT in new → released.
        for &k in prev {
            if k == 0 { continue; }
            if !new_keys.contains(&k) {
                if let Some(ps2) = hid_to_ps2(k) {
                    if *count < buf.len() {
                        buf[*count] = InputEvent::key(ps2, false);
                        *count += 1;
                    }
                }
            }
        }
        // Keys that are in new but NOT in prev → pressed.
        for &k in new_keys {
            if k == 0 { continue; }
            if !prev.contains(&k) {
                if let Some(ps2) = hid_to_ps2(k) {
                    if *count < buf.len() {
                        buf[*count] = InputEvent::key(ps2, true);
                        *count += 1;
                    }
                }
            }
        }
    }
}

impl InputHal for UsbHidHal {
    fn init(&mut self) -> bool {
        if self.initialized { return true; }

        // ── Lazily activate the xHCI controller ──
        // (The kernel only stores the MMIO base; we take ownership here
        //  so that BIOS USB Legacy Emulation stays active if this init
        //  fails — the PS/2 fallback keeps working.)
        if !bmo_xhci::is_controller_initialized() {
            let mmio = match bmo_xhci::get_mmio() {
                Some(m) => m,
                None => return false,
            };
            if !unsafe { bmo_xhci::init(mmio) } {
                return false; // Could not take ownership or start controller
            }
        }

        let ctrl = match bmo_xhci::controller() {
            Some(c) => c,
            None => return false,
        };

        // ── Reset each port and try to address HID devices ──
        for port in 0..ctrl.max_ports {
            unsafe {
                if !bmo_xhci::port_reset(port) { continue; }

                // Read actual port speed from PORTSC, try it first, then fallbacks
                let port_spd = bmo_xhci::port_speed(port);
                for speed in &[port_spd, 3, 2, 1, 4] {
                    let speed = *speed;
                    if speed == 0 { continue; }
                    if let Some(slot) = bmo_xhci::address_device(port, speed) {
                        // Set boot protocol + idle rate.
                        bmo_xhci::control_transfer(slot, 0x21, 0x0B, 0, 0, &mut [], false);
                        bmo_xhci::control_transfer(slot, 0x21, 0x0A, 0, 0, &mut [], false);

                        // Keyboard gets the first addressed device,
                        // mouse (if any) gets the second.
                        if self.kbd_slot.is_none() {
                            self.kbd_slot = Some(slot);
                        } else if self.mouse_slot.is_none() {
                            self.mouse_slot = Some(slot);
                            break; // we have both
                        }
                    }
                }
            }
        }

        self.initialized = true;
        self.kbd_slot.is_some() // at least keyboard required
    }

    fn name(&self) -> &'static str { "USB-HID" }

    fn poll(&mut self, buf: &mut [InputEvent]) -> usize {
        if !self.initialized { return 0; }
        let mut count = 0usize;

        unsafe {
            // ── Poll keyboard via GET_REPORT ──
            if let Some(slot) = self.kbd_slot {
                let mut report = KbdReport { modifiers: 0, _reserved: 0, keys: [0; 6] };
                let report_slice = core::slice::from_raw_parts_mut(
                    &mut report as *mut KbdReport as *mut u8, 8);

                let n = bmo_xhci::control_transfer(
                    slot, 0xA1, 0x01, 0x0100, 0, report_slice, true);

                if n >= 8 {
                    // ── Modifier keys ──
                    let mod_changed = report.modifiers ^ self.prev_mod;
                    if mod_changed & MOD_LCTRL  != 0 { self.emit_mod(MOD_LCTRL,  PS2_LCTRL,  report.modifiers & MOD_LCTRL  != 0, buf, &mut count); }
                    if mod_changed & MOD_LSHIFT != 0 { self.emit_mod(MOD_LSHIFT, PS2_LSHIFT, report.modifiers & MOD_LSHIFT != 0, buf, &mut count); }
                    if mod_changed & MOD_RCTRL  != 0 { self.emit_mod(MOD_RCTRL,  PS2_LCTRL,  report.modifiers & MOD_RCTRL  != 0, buf, &mut count); }
                    if mod_changed & MOD_RSHIFT != 0 { self.emit_mod(MOD_RSHIFT, PS2_RSHIFT, report.modifiers & MOD_RSHIFT != 0, buf, &mut count); }
                    if mod_changed & MOD_LALT   != 0 { self.emit_mod(MOD_LALT,   PS2_LALT,   report.modifiers & MOD_LALT   != 0, buf, &mut count); }
                    if mod_changed & MOD_RALT   != 0 { self.emit_mod(MOD_RALT,   PS2_LALT,   report.modifiers & MOD_RALT   != 0, buf, &mut count); }

                    // ── Non-modifier keys ──
                    Self::diff_keys(&self.prev_keys, &report.keys, buf, &mut count);

                    self.prev_mod = report.modifiers;
                    self.prev_keys = report.keys;
                }
            }

            // ── Poll mouse via GET_REPORT ──
            if let Some(slot) = self.mouse_slot {
                let mut report = MouseReport { buttons: 0, dx: 0, dy: 0, wheel: 0 };
                let report_slice = core::slice::from_raw_parts_mut(
                    &mut report as *mut MouseReport as *mut u8, 4);

                let n = bmo_xhci::control_transfer(
                    slot, 0xA1, 0x01, 0x0100, 0, report_slice, true);

                if n >= 4 {
                    // Movement
                    if report.dx != 0 || report.dy != 0 {
                        if count < buf.len() {
                            buf[count] = InputEvent::mouse_move(report.dx as i16, report.dy as i16);
                            count += 1;
                        }
                    }
                    // Buttons
                    if report.buttons != self.prev_buttons {
                        if count < buf.len() {
                            buf[count] = InputEvent::mouse_button(report.buttons);
                            count += 1;
                        }
                        self.prev_buttons = report.buttons;
                    }
                }
            }
        }

        count
    }

    fn pointer_mode(&self) -> PointerMode { PointerMode::Relative }
    fn is_ready(&self) -> bool { self.initialized }
}