//! PS/2 Mouse Driver (IRQ12, 3-byte standard packets).
//!
//! Handles PS/2 mouse input via IRQ12 (IDT vector 44). Reassembles
//! 3-byte packets from the auxiliary PS/2 port, tracks button state
//! and relative cursor movement.
//!
//! Architecture:
//!   - IRQ12 (vector 44) fires on each byte from the auxiliary port
//!   - 3-byte packets reassembled across consecutive IRQ12 firings
//!   - 9-bit signed deltas (X/Y) with overflow detection
//!   - Cursor position tracked in screen pixel coordinates
//!   - Results stored in a lock-free ring buffer (SPSC)
//!   - Poll via `mouse::read_event()` or `mouse::pos()`
//!
//! Public API:
//!   - `init()` — enable IRQ12, reset mouse, start data reporting
//!   - `read_event()` — get next MouseEvent (dx, dy, buttons)
//!   - `pos()` — current cursor position (x, y) in pixels
//!   - `buttons()` — current button state (left, right, middle)

use core::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};

// ── PS/2 I/O Ports ─────────────────────────────────────────────────

const PS2_DATA: u16 = 0x60;
const PS2_STATUS: u16 = 0x64;

#[inline]
unsafe fn inb(port: u16) -> u8 {
    let val: u8;
    unsafe {
        core::arch::asm!("in al, dx", in("dx") port, out("al") val, options(nomem, nostack));
    }
    val
}

#[inline]
unsafe fn outb(port: u16, val: u8) {
    unsafe {
        core::arch::asm!("out dx, al", in("dx") port, in("al") val, options(nomem, nostack));
    }
}

/// Wait for PS/2 controller input buffer to be empty (ready for write).
fn wait_input() {
    let mut timeout = 100_000u32;
    while unsafe { inb(PS2_STATUS) } & 0x02 != 0 {
        timeout = timeout.saturating_sub(1);
        if timeout == 0 { return; }
    }
}

/// Wait for PS/2 controller output buffer to have data (ready for read).
fn wait_output() {
    let mut timeout = 100_000u32;
    while unsafe { inb(PS2_STATUS) } & 0x01 == 0 {
        timeout = timeout.saturating_sub(1);
        if timeout == 0 { return; }
    }
}

// ── Mouse Event ────────────────────────────────────────────────────

/// PS/2 mouse event — button state + relative movement.
#[derive(Clone, Copy, Debug)]
pub struct MouseEvent {
    pub dx: i16,
    pub dy: i16,
    pub left: bool,
    pub right: bool,
    pub middle: bool,
}

// ── State ──────────────────────────────────────────────────────────

/// Mouse absolute cursor position (pixels, for framebuffer).
static CURSOR_X: AtomicUsize = AtomicUsize::new(0);
static CURSOR_Y: AtomicUsize = AtomicUsize::new(0);

/// Mouse button state.
static BTN_LEFT: AtomicBool = AtomicBool::new(false);
static BTN_RIGHT: AtomicBool = AtomicBool::new(false);
static BTN_MIDDLE: AtomicBool = AtomicBool::new(false);

/// Mouse packet reassembly state (3 bytes per packet).
static PKT_INDEX: AtomicUsize = AtomicUsize::new(0);
static PKT_BYTE1: AtomicU8 = AtomicU8::new(0);
static PKT_BYTE2: AtomicU8 = AtomicU8::new(0);

/// Mouse event ring buffer (SPSC, 64 events).
const RING_CAP: usize = 64;
static RING_HEAD: AtomicUsize = AtomicUsize::new(0);
static RING_TAIL: AtomicUsize = AtomicUsize::new(0);
/// Ring storage: each event is 7 bytes (5 used, 2 padding for alignment).
static RING: [AtomicU8; RING_CAP * 7] = {
    const ZERO: AtomicU8 = AtomicU8::new(0);
    [ZERO; RING_CAP * 7]
};

// ── Ring Buffer Operations ─────────────────────────────────────────

/// Push a MouseEvent into the ring buffer.
fn push_event(ev: MouseEvent) {
    let head = RING_HEAD.load(Ordering::Relaxed);
    let tail = RING_TAIL.load(Ordering::Acquire);
    let next = (head + 1) % RING_CAP;
    if next == tail { return; } // full, drop
    let base = head * 7;
    let dx = ev.dx as u16;
    let dy = ev.dy as u16;
    let btns = ((ev.left as u8) << 0) | ((ev.right as u8) << 1) | ((ev.middle as u8) << 2);
    RING[base].store((dx & 0xFF) as u8, Ordering::Relaxed);
    RING[base + 1].store((dx >> 8) as u8, Ordering::Relaxed);
    RING[base + 2].store((dy & 0xFF) as u8, Ordering::Relaxed);
    RING[base + 3].store((dy >> 8) as u8, Ordering::Relaxed);
    RING[base + 4].store(btns, Ordering::Relaxed);
    RING[base + 5].store(0, Ordering::Relaxed);
    RING[base + 6].store(0, Ordering::Relaxed);
    RING_HEAD.store(next, Ordering::Release);
}

// ── Public API ─────────────────────────────────────────────────────

/// Read one MouseEvent from the ring buffer. Returns None if empty.
pub fn read_event() -> Option<MouseEvent> {
    let tail = RING_TAIL.load(Ordering::Relaxed);
    let head = RING_HEAD.load(Ordering::Acquire);
    if tail == head { return None; }
    let base = tail * 7;
    let dx_lo = RING[base].load(Ordering::Relaxed) as u16;
    let dx_hi = RING[base + 1].load(Ordering::Relaxed) as u16;
    let dy_lo = RING[base + 2].load(Ordering::Relaxed) as u16;
    let dy_hi = RING[base + 3].load(Ordering::Relaxed) as u16;
    let btns = RING[base + 4].load(Ordering::Relaxed);
    RING_TAIL.store((tail + 1) % RING_CAP, Ordering::Release);
    Some(MouseEvent {
        dx: (dx_lo | (dx_hi << 8)) as i16,
        dy: (dy_lo | (dy_hi << 8)) as i16,
        left: btns & 0x01 != 0,
        right: btns & 0x02 != 0,
        middle: btns & 0x04 != 0,
    })
}

/// Current mouse cursor position (pixels).
pub fn pos() -> (usize, usize) {
    (
        CURSOR_X.load(Ordering::Relaxed),
        CURSOR_Y.load(Ordering::Relaxed),
    )
}

/// Current mouse button state (left, right, middle).
pub fn buttons() -> (bool, bool, bool) {
    (
        BTN_LEFT.load(Ordering::Relaxed),
        BTN_RIGHT.load(Ordering::Relaxed),
        BTN_MIDDLE.load(Ordering::Relaxed),
    )
}

/// How many mouse events are available.
pub fn available() -> usize {
    let head = RING_HEAD.load(Ordering::Acquire);
    let tail = RING_TAIL.load(Ordering::Relaxed);
    head.wrapping_sub(tail) % RING_CAP
}

// ── IRQ12 Handler ──────────────────────────────────────────────────

/// Called from IDT vector 44 (IRQ12) ISR stub.
///
/// Reassembles 3-byte PS/2 mouse packets:
///   Byte 1: [Y ovf | X ovf | Y sign | X sign | 1 | mid | right | left]
///   Byte 2: X movement (8-bit signed)
///   Byte 3: Y movement (8-bit signed)
pub fn irq12_handler() {
    let status = unsafe { inb(PS2_STATUS) };

    // Bit 5 = auxiliary data (mouse). If not set, discard.
    if status & 0x20 == 0 {
        unsafe { inb(PS2_DATA); } // drain stale byte
        return;
    }

    let data = unsafe { inb(PS2_DATA) };
    let idx = PKT_INDEX.load(Ordering::Relaxed);

    match idx {
        0 => {
            PKT_BYTE1.store(data, Ordering::Relaxed);
            PKT_INDEX.store(1, Ordering::Relaxed);
        }
        1 => {
            PKT_BYTE2.store(data, Ordering::Relaxed);
            PKT_INDEX.store(2, Ordering::Relaxed);
        }
        2 => {
            let b1 = PKT_BYTE1.load(Ordering::Relaxed);
            let b2 = PKT_BYTE2.load(Ordering::Relaxed);
            let b3 = data;

            // Sign-extend 8-bit deltas using sign bits from byte 1
            let raw_x = b2 as u16;
            let raw_y = b3 as u16;
            let mut dx = if b1 & 0x10 != 0 {
                (raw_x | 0xFF00) as i16
            } else {
                raw_x as i16
            };
            let mut dy = if b1 & 0x20 != 0 {
                (raw_y | 0xFF00) as i16
            } else {
                raw_y as i16
            };

            // Overflow: discard movement
            if b1 & 0x40 != 0 { dx = 0; }
            if b1 & 0x80 != 0 { dy = 0; }

            let left   = b1 & 0x01 != 0;
            let right  = b1 & 0x02 != 0;
            let middle = b1 & 0x04 != 0;

            // Update global button state
            BTN_LEFT.store(left, Ordering::Relaxed);
            BTN_RIGHT.store(right, Ordering::Relaxed);
            BTN_MIDDLE.store(middle, Ordering::Relaxed);

            // Update cursor position (screen coordinates, Y inverted for PS/2)
            let w = unsafe { crate::info::FB_WIDTH } as usize;
            let h = unsafe { crate::info::FB_HEIGHT } as usize;
            let old_x = CURSOR_X.load(Ordering::Relaxed);
            let old_y = CURSOR_Y.load(Ordering::Relaxed);
            let new_x = (old_x as i32 + dx as i32).max(0).min(w as i32 - 1) as usize;
            let new_y = (old_y as i32 - dy as i32).max(0).min(h as i32 - 1) as usize;
            CURSOR_X.store(new_x, Ordering::Relaxed);
            CURSOR_Y.store(new_y, Ordering::Relaxed);

            // Push event
            push_event(MouseEvent { dx, dy, left, right, middle });

            PKT_INDEX.store(0, Ordering::Relaxed);
        }
        _ => {
            PKT_INDEX.store(0, Ordering::Relaxed);
        }
    }
}

// ── Init ───────────────────────────────────────────────────────────

/// Initialize PS/2 mouse (auxiliary device).
///
/// Steps:
///   1. Enable auxiliary device on PS/2 controller (command 0xA8)
///   2. Enable IRQ12 in controller configuration byte (bit 1)
///   3. Enable cascade (IRQ2) on master PIC
///   4. Reset mouse (command 0xFF)
///   5. Enable data reporting (command 0xF4)
///   6. Register IRQ12 handler with IDT
pub fn init() {
    // Step 1: Enable auxiliary device
    wait_input();
    unsafe { outb(PS2_STATUS, 0xA8); }

    // Step 2: Enable IRQ12 in configuration byte
    wait_input();
    unsafe { outb(PS2_STATUS, 0x20); } // Read command byte
    wait_output();
    let config = unsafe { inb(PS2_DATA) };
    let config_new = config | 0x02; // Set bit 1 = enable IRQ12
    wait_input();
    unsafe { outb(PS2_STATUS, 0x60); } // Write command byte
    wait_input();
    unsafe { outb(PS2_DATA, config_new); }

    // Step 3: Enable cascade (IRQ2) on master PIC so IRQ12 can fire
    unsafe {
        let mask = inb(0x21);
        outb(0x21, mask & !0x04); // clear bit 2 → enable IRQ2
    }

    // Step 4: Reset mouse (must send 0xD4 first to select auxiliary port)
    wait_input();
    unsafe { outb(PS2_STATUS, 0xD4); } // Tell controller: next byte is for mouse
    wait_input();
    unsafe { outb(PS2_DATA, 0xFF); }   // Reset command → goes to mouse
    wait_output();
    let _ack = unsafe { inb(PS2_DATA) }; // 0xFA (ACK) or 0xAA (BAT OK)

    // Step 5: Enable data reporting (also needs 0xD4 prefix)
    wait_input();
    unsafe { outb(PS2_STATUS, 0xD4); } // Select auxiliary port
    wait_input();
    unsafe { outb(PS2_DATA, 0xF4); }   // Enable data reporting → goes to mouse
    wait_output();
    let _ack2 = unsafe { inb(PS2_DATA) }; // 0xFA = ACK

    // Step 6: Register IRQ12 handler
    crate::arch::idt::register_irq(12, irq12_handler);

    crate::dev::console::serial_write("[mouse] PS/2 mouse initialized (IRQ12 enabled)\n");
}
