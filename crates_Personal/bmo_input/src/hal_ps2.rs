//! PS/2 keyboard/mouse backend — controller init, port configuration, polling.
//!
//! Implements `InputHal` trait. Reads raw PS/2 ports 0x60/0x64 and converts
//! bytes to `InputEvent`s.

use crate::hal::{InputHal, PointerMode};
use crate::event::InputEvent;

/// PS/2 controller I/O ports.
const PS2_DATA: u16 = 0x60;
const PS2_STATUS: u16 = 0x64;

#[inline]
unsafe fn inb(port: u16) -> u8 {
    let v: u8;
    core::arch::asm!("in al, dx", out("al") v, in("dx") port, options(nomem, nostack));
    v
}

#[inline]
unsafe fn outb(port: u16, val: u8) {
    core::arch::asm!("out dx, al", in("dx") port, in("al") val, options(nomem, nostack));
}

unsafe fn wait_input() -> bool {
    let mut timeout = 100000u32;
    let s = inb(PS2_STATUS);
    if s == 0xFF { return false; }
    while (s & 0x02) != 0 && timeout > 0 {
        timeout -= 1; core::hint::spin_loop();
        let next = inb(PS2_STATUS);
        if next == 0xFF { return false; }
    }
    timeout > 0
}

unsafe fn wait_output() -> Option<u8> {
    for _ in 0..5000 {
        let s = inb(PS2_STATUS);
        if s == 0xFF { return None; }
        if (s & 0x01) != 0 { return Some(inb(PS2_DATA)); }
    }
    None
}

unsafe fn send_cmd(cmd: u8) -> bool {
    if !wait_input() { return false; }
    outb(PS2_STATUS, cmd);
    true
}

unsafe fn send_data(data: u8) -> bool {
    if !wait_input() { return false; }
    outb(PS2_DATA, data);
    true
}

// ── Mouse packet reassembly ─────────────────────────────────────

static mut MOUSE_PKT: [u8; 3] = [0; 3];
static mut MOUSE_PKT_IDX: usize = 0;
static mut MOUSE_DX_ACC: i32 = 0;
static mut MOUSE_DY_ACC: i32 = 0;
static mut MOUSE_BTNS: u8 = 0;

unsafe fn process_mouse_byte(b: u8, events: &mut [InputEvent], ev_idx: &mut usize) {
    MOUSE_PKT[MOUSE_PKT_IDX] = b;
    MOUSE_PKT_IDX += 1;
    if MOUSE_PKT_IDX < 3 { return; }
    MOUSE_PKT_IDX = 0;

    let b0 = MOUSE_PKT[0];
    if (b0 & 0x08) == 0 { return; } // not a valid packet
    if (b0 & 0xC0) != 0 { return; } // overflow

    let dx_raw = MOUSE_PKT[1] as i16;
    let dy_raw = MOUSE_PKT[2] as i16;
    let dx: i16 = if (b0 & 0x10) != 0 { dx_raw - 256 } else { dx_raw };
    let dy: i16 = if (b0 & 0x20) != 0 { dy_raw - 256 } else { dy_raw };
    let btns = b0 & 0x07;

    MOUSE_DX_ACC = MOUSE_DX_ACC.saturating_add(dx as i32);
    MOUSE_DY_ACC = MOUSE_DY_ACC.saturating_add(dy as i32);

    if *ev_idx < events.len() {
        events[*ev_idx] = InputEvent::mouse_move(dx, dy);
        *ev_idx += 1;
    }
    if btns != MOUSE_BTNS {
        if *ev_idx < events.len() {
            events[*ev_idx] = InputEvent::mouse_button(btns);
            *ev_idx += 1;
        }
        MOUSE_BTNS = btns;
    }
}

// ── Ps2Hal struct ────────────────────────────────────────────────

pub struct Ps2Hal {
    initialized: bool,
    has_mouse: bool,
}

impl Ps2Hal {
    pub const fn new() -> Self {
        Self { initialized: false, has_mouse: false }
    }
}

impl InputHal for Ps2Hal {
    fn init(&mut self) -> bool {
        if self.initialized { return true; }
        self.initialized = true;

        unsafe {
            // Check if PS/2 controller exists
            let s = inb(PS2_STATUS);
            if s == 0xFF {
                self.initialized = false;
                return false;
            }

            // 1. Disable ports
            send_cmd(0xAD);
            send_cmd(0xA7);

            // 2. Flush output buffer
            while let Some(_) = wait_output() {}

            // 3. Read config byte
            send_cmd(0x20);
            let config = match wait_output() {
                Some(b) => b,
                None => return false,
            };

            // 4. Clear translation bit, enable IRQ1
            let new_config = (config & !0x40) | 0x01;
            send_cmd(0x60);
            send_data(new_config);

            // 5. Self-test
            send_cmd(0xAA);
            if wait_output() != Some(0x55) { return false; }

            // 6. Enable port 1 (keyboard)
            send_cmd(0xAE);

            // 7. Reset keyboard
            send_data(0xFF);
            wait_output(); // ACK
            wait_output(); // BAT result (0xAA or 0xFC)

            // 8. Enable scanning
            send_data(0xF4);
            wait_output();

            // 9. Try port 2 (mouse)
            send_cmd(0xA8);
            send_cmd(0x20);
            let c2 = wait_output().unwrap_or(0);
            if (c2 & 0x20) != 0 {
                send_cmd(0x60);
                send_data(new_config | 0x02);
                self.has_mouse = true;

                // Reset mouse
                send_cmd(0xD4); send_data(0xFF);
                wait_output(); wait_output(); wait_output();
                // Enable reporting
                send_cmd(0xD4); send_data(0xF4);
                wait_output();
            }

            // 10. Num Lock LED
            send_data(0xED); wait_output();
            send_data(0x02);
        }
        true
    }

    fn name(&self) -> &'static str { "PS/2" }

    fn poll(&mut self, buf: &mut [InputEvent]) -> usize {
        if !self.initialized { return 0; }
        let mut count = 0usize;

        unsafe {
            // Drain PS/2 bytes
            for _ in 0..64 {
                let status = inb(PS2_STATUS);
                if status == 0xFF || (status & 0x01) == 0 { break; }

                let byte = inb(PS2_DATA);
                if (status & 0x20) != 0 {
                    // Mouse data
                    process_mouse_byte(byte, buf, &mut count);
                } else {
                    // Keyboard data
                    if count < buf.len() {
                        let pressed = (byte & 0x80) == 0;
                        let sc = byte & 0x7F;
                        buf[count] = InputEvent::key(sc, pressed);
                        count += 1;
                    }
                }
            }
        }
        count
    }

    fn pointer_mode(&self) -> PointerMode { PointerMode::Relative }
    fn is_ready(&self) -> bool { self.initialized }
}
