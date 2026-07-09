//! Desktop input — HAL or direct PS/2 fallback.
//!
//! Tries HAL first. If stubbed (kernel has no input drivers),
//! falls back to direct PS/2 port I/O for keyboard + mouse.

use crate::hal;
use crate::dev::console::{serial_write, serial_write_u64};
use core::arch::asm;

pub const SC_ESC: u8 = 0x01;

/// If set by the module, called to poll USB HID events before PS/2.
pub static mut USB_HID_POLL: Option<fn() -> bool> = None;

// ── Direct PS/2 port I/O (Ring 0 fallback) ────────────────────────────

#[inline] unsafe fn inb(port: u16) -> u8 {
    let v: u8; asm!("in al, dx", in("dx") port, out("al") v, options(nostack, nomem)); v
}
#[inline] unsafe fn outb(port: u16, v: u8) {
    asm!("out dx, al", in("dx") port, in("al") v, options(nostack, nomem));
}
unsafe fn ps2_has_data() -> bool { inb(0x64) & 1 != 0 }
unsafe fn ps2_read_data() -> u8 { while inb(0x64) & 1 == 0 { core::hint::spin_loop(); } inb(0x60) }

// ── Init + poll tracking ───────────────────────────────────────────────

static mut PS2_FALLBACK: bool = false;
static mut MOUSE_CYCLE: u8 = 0;
static mut MOUSE_BUF: [u8; 3] = [0; 3];
static mut MOUSE_X: i32 = 0;
static mut MOUSE_Y: i32 = 0;
static mut MOUSE_BTNS: u8 = 0;

fn ensure_input_ready() {
    static mut INITIALIZED: bool = false;
    unsafe {
        if INITIALIZED { return; }
        INITIALIZED = true;

        if let Some(h) = hal::HAL.as_ref() {
            let ok = (h.input_init)();
            serial_write("[input] HAL input_init() = ");
            serial_write(if ok { "OK\n" } else { "FAIL (using direct PS/2)\n" });
            if !ok {
                PS2_FALLBACK = true;
                ps2_init_direct();
            }
        } else {
            PS2_FALLBACK = true;
            ps2_init_direct();
        }
    }
}

unsafe fn ps2_init_direct() {
    // Enable first PS/2 port (keyboard)
    while inb(0x64) & 2 != 0 { core::hint::spin_loop(); }
    outb(0x64, 0xAE);
    for _ in 0..1000 { core::hint::spin_loop(); }

    // Send "enable scanning" to keyboard
    while inb(0x64) & 2 != 0 { core::hint::spin_loop(); }
    outb(0x60, 0xF4);
    let ack = ps2_read_data();

    // Enable mouse (aux PS/2 port)
    while inb(0x64) & 2 != 0 { core::hint::spin_loop(); }
    outb(0x64, 0xA8);

    // Get config byte
    while inb(0x64) & 2 != 0 { core::hint::spin_loop(); }
    outb(0x64, 0x20);
    let cfg = inb(0x60);

    // Enable mouse IRQ + clock
    while inb(0x64) & 2 != 0 { core::hint::spin_loop(); }
    outb(0x64, 0x60);
    while inb(0x64) & 2 != 0 { core::hint::spin_loop(); }
    outb(0x60, cfg | 0x02);

    // Enable mouse data reporting
    while inb(0x64) & 2 != 0 { core::hint::spin_loop(); }
    outb(0x64, 0xD4);
    while inb(0x64) & 2 != 0 { core::hint::spin_loop(); }
    outb(0x60, 0xF4);
    let _ = inb(0x60); // ack (0xFA) or not

    serial_write("[input] direct PS/2 keyboard+mouse enabled\n");
}

unsafe fn poll_direct_ps2(last_sc: &mut u8, last_mouse: &mut u64) {
    // Poll keyboard
    if ps2_has_data() {
        let sc = ps2_read_data();
        if sc == 0xE0 {
            // Extended — ignore for now
        } else if sc != 0xFA {
            *last_sc = sc;
        }
    }

    // Poll mouse (staggered: 61850 uses same port 0x60)
    // Mouse data arrives after keyboard data on port 0x60
    // The controller differentiates by the source bit
    // For simplicity, check if more data is available

    // Actually, PS/2 mouse data comes on port 0x60 too but the status
    // port bit 5 tells us the source (0=device 1, 1=device 2 = mouse)
    let status = inb(0x64);
    if status & 0x20 != 0 && status & 1 != 0 {
        // Mouse data
        let b = inb(0x60);
        MOUSE_CYCLE = MOUSE_CYCLE.wrapping_add(1);
        match MOUSE_CYCLE {
            1 => {
                if b & 0x08 == 0 { MOUSE_CYCLE = 0; return; }
                MOUSE_BUF[0] = b;
            }
            2 => { MOUSE_BUF[1] = b; }
            _ => {
                MOUSE_BUF[2] = b; MOUSE_CYCLE = 0;
                let dx = if MOUSE_BUF[0] & 0x10 != 0 {
                    (MOUSE_BUF[1] as i8) as i32
                } else { MOUSE_BUF[1] as i32 };
                let dy = if MOUSE_BUF[0] & 0x20 != 0 {
                    -((MOUSE_BUF[2] as i8) as i32)
                } else { -(MOUSE_BUF[2] as i32) };
                MOUSE_X = MOUSE_X.saturating_add(dx);
                MOUSE_Y = MOUSE_Y.saturating_add(dy);
                MOUSE_BTNS = MOUSE_BUF[0] & 0x07;
            }
        }
    }
}

// ── Public API ────────────────────────────────────────────────────────

pub fn poll_raw_scancode() -> u8 {
    ensure_input_ready();

    // USB HID first (module-provided)
    unsafe {
        if let Some(poll) = USB_HID_POLL { poll(); }
    }

    unsafe {
        if PS2_FALLBACK {
            let mut sc: u8 = 0;
            let mut _m: u64 = 0;
            poll_direct_ps2(&mut sc, &mut _m);
            sc
        } else if let Some(h) = hal::HAL.as_mut() {
            let mut buf = [bmo_hal_defs::InputEvent::empty(); 32];
            let n = (h.input_poll)(&mut buf);
            let mut last: u8 = 0;
            for ev in &buf[..n] {
                if matches!(ev.kind, bmo_hal_defs::InputEventKind::KeyDown) {
                    last = ev.code;
                } else if matches!(ev.kind, bmo_hal_defs::InputEventKind::KeyUp) {
                    last = ev.code | 0x80;
                }
            }
            last
        } else { 0 }
    }
}

pub fn poll_key() -> u8 {
    poll_raw_scancode()
}

pub fn poll_mouse() -> u64 {
    ensure_input_ready();
    unsafe {
        if PS2_FALLBACK {
            let mut _sc: u8 = 0;
            let mut m: u64 = 0;
            poll_direct_ps2(&mut _sc, &mut m);
            let x = MOUSE_X.clamp(-32768, 32767) as i16 as u16 as u64;
            let y = MOUSE_Y.clamp(-32768, 32767) as i16 as u16 as u64;
            MOUSE_X = 0; MOUSE_Y = 0;
            x | (y << 16) | ((MOUSE_BTNS as u64) << 32)
        } else if let Some(h) = hal::HAL.as_mut() {
            let mut buf = [bmo_hal_defs::InputEvent::empty(); 32];
            let n = (h.input_poll)(&mut buf);
            let mut x: i32 = 0; let mut y: i32 = 0; let mut btns: u64 = 0;
            for ev in &buf[..n] {
                match ev.kind {
                    bmo_hal_defs::InputEventKind::MouseMove => {
                        x = x.saturating_add(ev.mouse_dx() as i32);
                        y = y.saturating_add(ev.mouse_dy() as i32);
                    }
                    bmo_hal_defs::InputEventKind::MouseButton => { btns = ev.mouse_buttons() as u64; }
                    _ => {}
                }
            }
            let xi = (x.clamp(-32768, 32767) as i16) as u16 as u64;
            let yi = (y.clamp(-32768, 32767) as i16) as u16 as u64;
            xi | (yi << 16) | (btns << 32)
        } else { 0 }
    }
}
