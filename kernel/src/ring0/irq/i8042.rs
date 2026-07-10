//! Shared i8042 PS/2 controller.
//!
//! Port 0x60 is shared by the keyboard and auxiliary (mouse) devices.  A
//! single owner must drain it and route each byte using status bit 5;
//! otherwise the keyboard poller can consume mouse packets (and vice versa).

use core::arch::asm;
use core::sync::atomic::{AtomicU8, Ordering};

const DATA: u16 = 0x60;
const STATUS: u16 = 0x64;
const WAIT_SPINS: usize = 100_000;

const UNINITIALIZED: u8 = 0;
const INITIALIZING: u8 = 1;
const READY: u8 = 2;
const UNAVAILABLE: u8 = 3;

static STATE: AtomicU8 = AtomicU8::new(UNINITIALIZED);
static MOUSE_READY: AtomicU8 = AtomicU8::new(0);

#[inline]
unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    asm!("in al, dx", in("dx") port, out("al") value, options(nostack, nomem));
    value
}

#[inline]
unsafe fn outb(port: u16, value: u8) {
    asm!("out dx, al", in("dx") port, in("al") value, options(nostack, nomem));
}

unsafe fn wait_write() -> bool {
    for _ in 0..WAIT_SPINS {
        let status = inb(STATUS);
        if status == 0xFF { return false; }
        if status & 0x02 == 0 { return true; }
        core::hint::spin_loop();
    }
    false
}

unsafe fn read_timeout() -> Option<(u8, bool)> {
    for _ in 0..WAIT_SPINS {
        let status = inb(STATUS);
        if status == 0xFF { return None; }
        if status & 0x01 != 0 {
            return Some((inb(DATA), status & 0x20 != 0));
        }
        core::hint::spin_loop();
    }
    None
}

unsafe fn command(value: u8) -> bool {
    if !wait_write() { return false; }
    outb(STATUS, value);
    true
}

unsafe fn write_data(value: u8) -> bool {
    if !wait_write() { return false; }
    outb(DATA, value);
    true
}

unsafe fn write_mouse(value: u8) -> bool {
    command(0xD4) && write_data(value)
}

unsafe fn keyboard_command(value: u8) -> bool {
    for _ in 0..2 {
        if !write_data(value) { return false; }
        match read_timeout() {
            Some((0xFA, false)) => return true,
            Some((0xFE, false)) => continue,
            _ => return false,
        }
    }
    false
}

unsafe fn mouse_command(value: u8) -> bool {
    for _ in 0..2 {
        if !write_mouse(value) { return false; }
        match read_timeout() {
            Some((0xFA, true)) => return true,
            Some((0xFE, true)) => continue,
            _ => return false,
        }
    }
    false
}

unsafe fn set_mouse_sample_rate(rate: u8) -> bool {
    if !mouse_command(0xF3) { return false; }
    for _ in 0..2 {
        if !write_mouse(rate) { return false; }
        match read_timeout() {
            Some((0xFA, true)) => return true,
            Some((0xFE, true)) => continue,
            _ => return false,
        }
    }
    false
}

unsafe fn flush_output() {
    for _ in 0..64 {
        if inb(STATUS) & 0x01 == 0 { break; }
        let _ = inb(DATA);
    }
}

/// Initialize both PS/2 ports. All waits are bounded so machines without a
/// legacy controller continue booting normally.
pub fn init() -> bool {
    match STATE.compare_exchange(UNINITIALIZED, INITIALIZING, Ordering::AcqRel, Ordering::Acquire) {
        Ok(_) => {}
        Err(READY) => return true,
        Err(UNAVAILABLE) => return false,
        Err(_) => {
            for _ in 0..WAIT_SPINS {
                if STATE.load(Ordering::Acquire) != INITIALIZING { break; }
                core::hint::spin_loop();
            }
            return STATE.load(Ordering::Acquire) == READY;
        }
    }

    let available = unsafe { init_inner() };
    STATE.store(if available { READY } else { UNAVAILABLE }, Ordering::Release);
    available
}

unsafe fn init_inner() -> bool {
    if inb(STATUS) == 0xFF { return false; }

    // Disable both ports while changing controller configuration.
    if !command(0xAD) || !command(0xA7) { return false; }
    flush_output();

    if !command(0x20) { return false; }
    let mut config = match read_timeout() {
        Some((value, false)) => value,
        _ => return false,
    };

    // Polling is used until IOAPIC routing is operational. Keep IRQ1/IRQ12
    // masked and both clocks disabled while testing the interfaces.
    config = (config | 0x40 | 0x10 | 0x20) & !(0x01 | 0x02);
    if !command(0x60) || !write_data(config) { return false; }

    let keyboard_port = command(0xAB)
        && matches!(read_timeout(), Some((0x00, false)));

    // Test and enable the auxiliary port, then negotiate IntelliMouse mode.
    let mouse_port = command(0xA9)
        && matches!(read_timeout(), Some((0x00, false)))
        && command(0xA8);
    let mut mouse_ready = mouse_port && mouse_command(0xF6); // Defaults.
    if mouse_ready {
        let wheel_sequence = set_mouse_sample_rate(200)
            && set_mouse_sample_rate(100)
            && set_mouse_sample_rate(80);
        let has_wheel = if wheel_sequence && mouse_command(0xF2) {
            matches!(read_timeout(), Some((3, true)) | Some((4, true)))
        } else {
            false
        };
        crate::irq::mouse::set_packet_size(if has_wheel { 4 } else { 3 });
        mouse_ready = mouse_command(0xF4); // Enable data reporting.
        if mouse_ready { MOUSE_READY.store(1, Ordering::Release); }
    }

    // Enable keyboard scanning last so its asynchronous bytes cannot be
    // mistaken for replies during mouse negotiation.
    let keyboard_ready = keyboard_port && command(0xAE) && keyboard_command(0xF4);
    keyboard_ready || mouse_ready
}

/// Drain all pending controller bytes and route them to the correct parser.
pub fn poll() {
    if STATE.load(Ordering::Acquire) != READY { return; }
    for _ in 0..64 {
        let status = unsafe { inb(STATUS) };
        if status == 0xFF || status & 0x01 == 0 { break; }
        let byte = unsafe { inb(DATA) };
        // Discard parity/timeout errors rather than desynchronizing a packet.
        if status & 0xC0 != 0 {
            if status & 0x20 != 0 {
                crate::irq::mouse::reset_packet();
            } else {
                crate::irq::keyboard::reset_prefix();
            }
            continue;
        }
        if status & 0x20 != 0 {
            if MOUSE_READY.load(Ordering::Acquire) != 0 {
                crate::irq::mouse::handle_byte(byte);
            }
        } else {
            crate::irq::keyboard::handle_byte(byte);
        }
    }
}
