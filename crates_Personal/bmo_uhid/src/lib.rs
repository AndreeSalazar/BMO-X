//! USB HID driver — boot protocol keyboard (8-byte) + mouse (4-byte).
//!
//! Implements `InputHal` trait. Submits interrupt IN transfers on xHCI
//! endpoints and parses HID reports into InputEvents.

#![no_std]

use bmo_input::hal::{InputHal, PointerMode};
use bmo_input::event::InputEvent;

/// HID boot protocol keyboard report (8 bytes).
pub struct KbdReport {
    pub modifiers: u8,  // byte 0: Ctrl, Shift, Alt, GUI
    pub _reserved: u8,  // byte 1
    pub keys: [u8; 6],  // bytes 2-7: up to 6 simultaneous keycodes
}

/// HID boot protocol mouse report (4 bytes).
pub struct MouseReport {
    pub buttons: u8,   // byte 0
    pub dx: i8,         // byte 1 (signed)
    pub dy: i8,         // byte 2 (signed)
    pub wheel: i8,      // byte 3 (signed)
}

pub struct UsbHidHal {
    kbd_slot: Option<u8>,
    mouse_slot: Option<u8>,
    kbd_report_buf: [u8; 8],
    mouse_report_buf: [u8; 4],
    prev_keys: [u8; 6],
    prev_buttons: u8,
    initialized: bool,
}

impl UsbHidHal {
    pub const fn new() -> Self {
        Self {
            kbd_slot: None, mouse_slot: None,
            kbd_report_buf: [0; 8], mouse_report_buf: [0; 4],
            prev_keys: [0; 6], prev_buttons: 0,
            initialized: false,
        }
    }
}

impl InputHal for UsbHidHal {
    fn init(&mut self) -> bool {
        if self.initialized { return true; }
        let ctrl = match bmo_xhci::controller() {
            Some(c) => c,
            None => return false,
        };

        // Enumerate ports to find HID devices
        for port in 0..ctrl.max_ports {
            unsafe {
                if !bmo_xhci::port_reset(port) { continue; }
                // Try to address as low-speed device (HID is usually low/full speed)
                if let Some(slot) = bmo_xhci::address_device(port, 2) { // 2 = low speed
                    // Set boot protocol via control transfer
                    bmo_xhci::control_transfer(slot, 0x21, 0x0B, 0, 0, &mut [], false); // SET_PROTOCOL boot
                    bmo_xhci::control_transfer(slot, 0x21, 0x0A, 0, 0, &mut [], false); // SET_IDLE

                    self.kbd_slot = Some(slot);
                    break; // first HID device = keyboard for now
                }
                // Try full speed
                if let Some(slot) = bmo_xhci::address_device(port, 1) { // 1 = full speed
                    bmo_xhci::control_transfer(slot, 0x21, 0x0B, 0, 0, &mut [], false);
                    bmo_xhci::control_transfer(slot, 0x21, 0x0A, 0, 0, &mut [], false);
                    if self.kbd_slot.is_none() {
                        self.kbd_slot = Some(slot);
                    } else {
                        self.mouse_slot = Some(slot);
                        break;
                    }
                }
            }
        }

        self.initialized = true;
        true
    }

    fn name(&self) -> &'static str { "USB-HID" }

    fn poll(&mut self, buf: &mut [InputEvent]) -> usize {
        let mut count = 0usize;

        // Poll keyboard
        if let Some(slot) = self.kbd_slot {
            unsafe {
                // Submit interrupt IN transfer (8 bytes for boot keyboard)
                // In polling mode, read from the endpoint's transfer ring
                // For now, stub — the TRB mechanism needs the endpoint configured
                let _ = slot;
            }
        }

        // Poll mouse
        if let Some(slot) = self.mouse_slot {
            unsafe {
                let _ = slot;
            }
        }

        count
    }

    fn pointer_mode(&self) -> PointerMode { PointerMode::Relative }
    fn is_ready(&self) -> bool { self.initialized }
}
