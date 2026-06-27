//! PS/2 Keyboard Driver (IRQ1, Set 1 scancodes).
//!
//! Translates PS/2 Set 1 scancodes to CP437 characters with Shift/CapsLock
//! support. Handles Spanish keyboard extras: Ñ(165)/ñ(164), á(160), é(130),
//! í(161), ó(162), ú(163), ü(129), ¡(173), ¿(168), º(167), ª(166).
//!
//! Architecture:
//!   - IRQ1 (vector 33) fires on each key event
//!   - Scancode translated to char via lookup table + modifier state
//!   - Result stored in a lock-free ring buffer (SPSC, single producer = IRQ)
//!   - Poll via `keyboard::read_char()` or `keyboard::available()`

use core::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};

// ── PS/2 I/O Ports ─────────────────────────────────────────────────

const PS2_DATA: u16 = 0x60;
const PS2_STATUS_REG: u16 = 0x64;

// ── Modifier State ─────────────────────────────────────────────────

static LSHIFT: AtomicBool = AtomicBool::new(false);
static RSHIFT: AtomicBool = AtomicBool::new(false);
static CAPSLOCK: AtomicBool = AtomicBool::new(false);
static CTRL: AtomicBool = AtomicBool::new(false);
static ALT: AtomicBool = AtomicBool::new(false);

fn shift_active() -> bool {
    LSHIFT.load(Ordering::Relaxed) ^ RSHIFT.load(Ordering::Relaxed)
}

fn caps_shift_active() -> bool {
    shift_active() ^ CAPSLOCK.load(Ordering::Relaxed)
}

// ── Event Ring Buffer (SPSC, 256 entries) ──────────────────────────

const RING_CAP: usize = 256;

static RING_HEAD: AtomicUsize = AtomicUsize::new(0);
static RING_TAIL: AtomicUsize = AtomicUsize::new(0);
static RING_BUF: [AtomicU8; RING_CAP] = {
    const ZERO: AtomicU8 = AtomicU8::new(0);
    [ZERO; RING_CAP]
};

/// Push a character into the ring buffer. Called from IRQ1 handler.
fn push(c: u8) {
    let head = RING_HEAD.load(Ordering::Relaxed);
    let tail = RING_TAIL.load(Ordering::Acquire);
    let next = (head + 1) % RING_CAP;
    if next == tail { return; } // full, drop
    RING_BUF[head].store(c, Ordering::Relaxed);
    RING_HEAD.store(next, Ordering::Release);
}

/// Read one character from the ring buffer. Returns None if empty.
pub fn read_char() -> Option<u8> {
    let tail = RING_TAIL.load(Ordering::Relaxed);
    let head = RING_HEAD.load(Ordering::Acquire);
    if tail == head { return None; }
    let c = RING_BUF[tail].load(Ordering::Relaxed);
    RING_TAIL.store((tail + 1) % RING_CAP, Ordering::Release);
    Some(c)
}

/// Returns how many characters are available.
pub fn available() -> usize {
    let head = RING_HEAD.load(Ordering::Acquire);
    let tail = RING_TAIL.load(Ordering::Relaxed);
    head.wrapping_sub(tail) % RING_CAP
}

// ── Scancode Set 1 → CP437 Translation (Spanish) ───────────────────

/// Translates PS/2 Set 1 scancode to CP437 character for Spanish keyboard.
///
/// Returns 0 for non-printable keys (modifiers, function keys, etc.).
fn translate_es(scancode: u8) -> u8 {
    // Modifier keys — handle state
    match scancode {
        0x2A => { LSHIFT.store(true, Ordering::Relaxed); return 0; }
        0x36 => { RSHIFT.store(true, Ordering::Relaxed); return 0; }
        0xAA => { LSHIFT.store(false, Ordering::Relaxed); return 0; }
        0xB6 => { RSHIFT.store(false, Ordering::Relaxed); return 0; }
        0x1D => { CTRL.store(true, Ordering::Relaxed); return 0; }
        0x9D => { CTRL.store(false, Ordering::Relaxed); return 0; }
        0x38 => { ALT.store(true, Ordering::Relaxed); return 0; }
        0xB8 => { ALT.store(false, Ordering::Relaxed); return 0; }
        0x3A => {
            CAPSLOCK.store(!CAPSLOCK.load(Ordering::Relaxed), Ordering::Relaxed);
            return 0;
        }
        0x80..=0xFF => return 0,
        _ => {}
    }

    let shift = shift_active();
    let caps = caps_shift_active();

    match scancode {
        // Number row (Spanish layout: / ' ( ) = ?)
        0x02 => if shift { b'!' } else { b'1' },
        0x03 => if shift { b'/' } else { b'2' },
        0x04 => if shift { b'\'' } else { b'3' },
        0x05 => if shift { b'(' } else { b'4' },
        0x06 => if shift { b')' } else { b'5' },
        0x07 => if shift { b'=' } else { b'6' },
        0x08 => if shift { b'?' } else { b'7' },
        0x09 => if shift { 0xA8 } else { b'8' },      // ¨ (168)
        0x0A => if shift { 0xB4 } else { b'9' },      // ´ (180)
        0x0B => if shift { 0xA1 } else { b'0' },      // ¡ (161) — actually ¡ is 173 in CP437

        // Symbol row
        0x0C => if shift { b'\'' } else { b'-' },     // Spanish: '  not _ (wait, - is unshifted)
        0x0D => if shift { 0xBF } else { 0xA1 },      // ¿ (191) / ¡ (161)

        // Letters Q-P
        0x10 => if caps { b'Q' } else { b'q' },
        0x11 => if caps { b'W' } else { b'w' },
        0x12 => if caps { b'E' } else { b'e' },
        0x13 => if caps { b'R' } else { b'r' },
        0x14 => if caps { b'T' } else { b't' },
        0x15 => if caps { b'Y' } else { b'y' },
        0x16 => if caps { b'U' } else { b'u' },
        0x17 => if caps { b'I' } else { b'i' },
        0x18 => if caps { b'O' } else { b'o' },
        0x19 => if caps { b'P' } else { b'p' },

        // [ ] on Spanish (above Enter row)
        0x1A => if shift { 0xA8 } else { 0xB4 },      // ¨ / ´ (dead keys)
        0x1B => if shift { b'*' } else { b'+' },

        // Letters A-L
        0x1E => if caps { b'A' } else { b'a' },
        0x1F => if caps { b'S' } else { b's' },
        0x20 => if caps { b'D' } else { b'd' },
        0x21 => if caps { b'F' } else { b'f' },
        0x22 => if caps { b'G' } else { b'g' },
        0x23 => if caps { b'H' } else { b'h' },
        0x24 => if caps { b'J' } else { b'j' },
        0x25 => if caps { b'K' } else { b'k' },
        0x26 => if caps { b'L' } else { b'l' },

        // ñ key (scancode 0x27 — where US has ;)
        0x27 => if shift { 0xA5 } else { 0xA4 },      // Ñ(165) / ñ(164)

        // ´ accent key (scancode 0x28)
        0x28 => 0xB4,                                   // ´ (180)

        // Ç key (scancode 0x2B — where US has \)
        0x2B => 0xC7,                                   // Ç (199)

        // Letters Z-M
        0x2C => if caps { b'Z' } else { b'z' },
        0x2D => if caps { b'X' } else { b'x' },
        0x2E => if caps { b'C' } else { b'c' },
        0x2F => if caps { b'V' } else { b'v' },
        0x30 => if caps { b'B' } else { b'b' },
        0x31 => if caps { b'N' } else { b'n' },
        0x32 => if caps { b'M' } else { b'm' },

        // , . - on Spanish
        0x33 => if shift { b';' } else { b',' },
        0x34 => if shift { b':' } else { b'.' },
        0x35 => if shift { b'_' } else { b'-' },

        // Special keys
        0x0E => 0x08,   // Backspace
        0x0F => 0x09,   // Tab
        0x1C => 0x0D,   // Enter
        0x39 => b' ',   // Space
        0x01 => 0x1B,   // Escape

        // Function/arrow keys → ignore
        0x3B..=0x44 | 0x57 | 0x58 | 0x48 | 0x50 | 0x4B | 0x4D => 0,

        _ => 0,
    }
}

// ── IRQ1 Handler ───────────────────────────────────────────────────

/// Called from IDT vector 33 (IRQ1) ISR stub.
///
/// Reads scancode from PS/2 data port, translates to CP437 char,
/// pushes into ring buffer. Only fires on key PRESS (not release).
pub fn irq1_handler() {
    let raw = unsafe { ps2_inb(PS2_DATA) };

    // Key release (bit 7 set) — update modifier state
    if raw & 0x80 != 0 {
        match raw & 0x7F {
            0x2A => LSHIFT.store(false, Ordering::Relaxed),
            0x36 => RSHIFT.store(false, Ordering::Relaxed),
            0x1D => CTRL.store(false, Ordering::Relaxed),
            0x38 => ALT.store(false, Ordering::Relaxed),
            _ => {}
        }
        return;
    }

    // CapsLock toggle on press only
    if raw == 0x3A {
        CAPSLOCK.store(!CAPSLOCK.load(Ordering::Relaxed), Ordering::Relaxed);
        return;
    }

    // Translate and push
    let c = translate_es(raw);
    if c != 0 {
        push(c);
    }
}

// ── Init ───────────────────────────────────────────────────────────

/// Initialize the PS/2 keyboard controller.
///
/// Enables keyboard IRQ (IRQ1) by clearing bit 1 of PIC mask.
pub fn init() {
    unsafe {
        // Enable IRQ1 (keyboard) on the master PIC: port 0x21, bit 1
        let mask = ps2_inb(0x21);
        ps2_outb(0x21, mask & !0x02);
    }

    // Clear any stale data in the PS/2 buffer
    while unsafe { ps2_inb(PS2_STATUS_REG) } & 0x01 != 0 {
        unsafe { ps2_inb(PS2_DATA); }
    }

    crate::dev::console::serial_write("[keyboard] PS/2 keyboard initialized (IRQ1 enabled)\n");

    // Register IRQ1 handler with the IDT dispatcher
    crate::arch::idt::register_irq(1, irq1_handler);
}

// ── x86 I/O Port Access ───────────────────────────────────────────

#[inline]
unsafe fn ps2_inb(port: u16) -> u8 {
    let val: u8;
    unsafe {
        core::arch::asm!(
            "in al, dx",
            in("dx") port,
            out("al") val,
            options(nomem, nostack)
        );
    }
    val
}

#[inline]
unsafe fn ps2_outb(port: u16, val: u8) {
    unsafe {
        core::arch::asm!(
            "out dx, al",
            in("dx") port,
            in("al") val,
            options(nomem, nostack)
        );
    }
}

/// Wait for PS/2 controller input buffer to be empty (ready for write).
fn ps2_wait_input() {
    let mut timeout = 100_000u32;
    while unsafe { ps2_inb(PS2_STATUS_REG) } & 0x02 != 0 {
        timeout = timeout.saturating_sub(1);
        if timeout == 0 { return; }
    }
}

/// Wait for PS/2 controller output buffer to have data (ready for read).
fn ps2_wait_output() {
    let mut timeout = 100_000u32;
    while unsafe { ps2_inb(PS2_STATUS_REG) } & 0x01 == 0 {
        timeout = timeout.saturating_sub(1);
        if timeout == 0 { return; }
    }
}

// ═══════════════════════════════════════════════════════════════════
// PS/2 Mouse (IRQ12, 3-byte standard packets)
// ═══════════════════════════════════════════════════════════════════

/// PS/2 mouse event — button state + relative movement.
#[derive(Clone, Copy, Debug)]
pub struct MouseEvent {
    pub dx: i16,
    pub dy: i16,
    pub left: bool,
    pub right: bool,
    pub middle: bool,
}

/// Mouse absolute cursor position (pixels, for framebuffer).
static MOUSE_X: AtomicUsize = AtomicUsize::new(0);
static MOUSE_Y: AtomicUsize = AtomicUsize::new(0);

/// Mouse button state.
static MOUSE_LEFT: AtomicBool = AtomicBool::new(false);
static MOUSE_RIGHT: AtomicBool = AtomicBool::new(false);
static MOUSE_MIDDLE: AtomicBool = AtomicBool::new(false);

/// Mouse packet reassembly state (3 bytes per packet).
static MOUSE_PACKET: AtomicUsize = AtomicUsize::new(0); // byte index 0..3
static MOUSE_BYTE1: AtomicU8 = AtomicU8::new(0);
static MOUSE_BYTE2: AtomicU8 = AtomicU8::new(0);

/// Mouse event ring buffer (SPSC, 64 events).
const MOUSE_RING_CAP: usize = 64;
static MOUSE_HEAD: AtomicUsize = AtomicUsize::new(0);
static MOUSE_TAIL: AtomicUsize = AtomicUsize::new(0);
static MOUSE_RING: [AtomicU8; MOUSE_RING_CAP * 7] = {
    const ZERO: AtomicU8 = AtomicU8::new(0);
    [ZERO; MOUSE_RING_CAP * 7]
};

/// Push a MouseEvent into the mouse ring buffer.
fn mouse_push_event(ev: MouseEvent) {
    let head = MOUSE_HEAD.load(Ordering::Relaxed);
    let tail = MOUSE_TAIL.load(Ordering::Acquire);
    let next = (head + 1) % MOUSE_RING_CAP;
    if next == tail { return; } // full, drop
    let base = head * 7;
    // Pack event: dx(i16) dy(i16) buttons(u8) = 5 bytes, pad to 7
    let dx = ev.dx as u16;
    let dy = ev.dy as u16;
    let btns = ((ev.left as u8) << 0) | ((ev.right as u8) << 1) | ((ev.middle as u8) << 2);
    MOUSE_RING[base].store((dx & 0xFF) as u8, Ordering::Relaxed);
    MOUSE_RING[base + 1].store((dx >> 8) as u8, Ordering::Relaxed);
    MOUSE_RING[base + 2].store((dy & 0xFF) as u8, Ordering::Relaxed);
    MOUSE_RING[base + 3].store((dy >> 8) as u8, Ordering::Relaxed);
    MOUSE_RING[base + 4].store(btns, Ordering::Relaxed);
    MOUSE_RING[base + 5].store(0, Ordering::Relaxed);
    MOUSE_RING[base + 6].store(0, Ordering::Relaxed);
    MOUSE_HEAD.store(next, Ordering::Release);
}

/// Read one MouseEvent from the ring buffer. Returns None if empty.
pub fn read_mouse() -> Option<MouseEvent> {
    let tail = MOUSE_TAIL.load(Ordering::Relaxed);
    let head = MOUSE_HEAD.load(Ordering::Acquire);
    if tail == head { return None; }
    let base = tail * 7;
    let dx_lo = MOUSE_RING[base].load(Ordering::Relaxed) as u16;
    let dx_hi = MOUSE_RING[base + 1].load(Ordering::Relaxed) as u16;
    let dy_lo = MOUSE_RING[base + 2].load(Ordering::Relaxed) as u16;
    let dy_hi = MOUSE_RING[base + 3].load(Ordering::Relaxed) as u16;
    let btns = MOUSE_RING[base + 4].load(Ordering::Relaxed);
    MOUSE_TAIL.store((tail + 1) % MOUSE_RING_CAP, Ordering::Release);
    Some(MouseEvent {
        dx: (dx_lo | (dx_hi << 8)) as i16,
        dy: (dy_lo | (dy_hi << 8)) as i16,
        left: btns & 0x01 != 0,
        right: btns & 0x02 != 0,
        middle: btns & 0x04 != 0,
    })
}

/// Returns current mouse cursor position (pixels).
pub fn mouse_pos() -> (usize, usize) {
    (
        MOUSE_X.load(Ordering::Relaxed),
        MOUSE_Y.load(Ordering::Relaxed),
    )
}

/// Returns current mouse button state.
pub fn mouse_buttons() -> (bool, bool, bool) {
    (
        MOUSE_LEFT.load(Ordering::Relaxed),
        MOUSE_RIGHT.load(Ordering::Relaxed),
        MOUSE_MIDDLE.load(Ordering::Relaxed),
    )
}

/// How many mouse events are available.
pub fn mouse_available() -> usize {
    let head = MOUSE_HEAD.load(Ordering::Acquire);
    let tail = MOUSE_TAIL.load(Ordering::Relaxed);
    head.wrapping_sub(tail) % MOUSE_RING_CAP
}

/// IRQ12 handler — PS/2 mouse. Called from IDT vector 44.
pub fn irq12_handler() {
    let status = unsafe { ps2_inb(PS2_STATUS_REG) };

    // Bit 5 = auxiliary data (mouse). If not set, discard.
    if status & 0x20 == 0 {
        // Drain stale byte to keep OBF clear
        unsafe { ps2_inb(PS2_DATA); }
        return;
    }

    let data = unsafe { ps2_inb(PS2_DATA) };
    let pkt_idx = MOUSE_PACKET.load(Ordering::Relaxed);

    match pkt_idx {
        0 => {
            // Byte 1: buttons + flags
            MOUSE_BYTE1.store(data, Ordering::Relaxed);
            MOUSE_PACKET.store(1, Ordering::Relaxed);
        }
        1 => {
            // Byte 2: X movement
            MOUSE_BYTE2.store(data, Ordering::Relaxed);
            MOUSE_PACKET.store(2, Ordering::Relaxed);
        }
        2 => {
            // Byte 3: Y movement — packet complete
            let b1 = MOUSE_BYTE1.load(Ordering::Relaxed);
            let b2 = MOUSE_BYTE2.load(Ordering::Relaxed);
            let b3 = data;

            // Sign-extend 8-bit delta to i16 using sign bit from byte 1
            // PS/2 mouse: byte1 bits 4-5 are sign bits for 9-bit deltas
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
            MOUSE_LEFT.store(left, Ordering::Relaxed);
            MOUSE_RIGHT.store(right, Ordering::Relaxed);
            MOUSE_MIDDLE.store(middle, Ordering::Relaxed);

            // Update cursor position (screen coordinates, Y inverted for PS/2)
            let w = unsafe { crate::info::FB_WIDTH } as usize;
            let h = unsafe { crate::info::FB_HEIGHT } as usize;
            let old_x = MOUSE_X.load(Ordering::Relaxed);
            let old_y = MOUSE_Y.load(Ordering::Relaxed);
            let new_x = (old_x as i32 + dx as i32).max(0).min(w as i32 - 1) as usize;
            let new_y = (old_y as i32 - dy as i32).max(0).min(h as i32 - 1) as usize; // Y inverted
            MOUSE_X.store(new_x, Ordering::Relaxed);
            MOUSE_Y.store(new_y, Ordering::Relaxed);

            // Push event
            mouse_push_event(MouseEvent { dx, dy, left, right, middle });

            MOUSE_PACKET.store(0, Ordering::Relaxed);
        }
        _ => {
            MOUSE_PACKET.store(0, Ordering::Relaxed);
        }
    }
}

/// Initialize PS/2 mouse (auxiliary device).
///
/// Enables IRQ12 on slave PIC, sends enable-data-reporting command.
pub fn init_mouse() {
    // Step 1: Enable auxiliary device (mouse) on PS/2 controller
    ps2_wait_input();
    unsafe { ps2_outb(PS2_STATUS_REG, 0xA8); } // Enable auxiliary device

    // Step 2: Enable IRQ12 in controller configuration byte
    ps2_wait_input();
    unsafe { ps2_outb(PS2_STATUS_REG, 0x20); } // Read command byte
    ps2_wait_output();
    let config = unsafe { ps2_inb(PS2_DATA) };
    let config_new = config | 0x02; // Set bit 1 = enable IRQ12
    ps2_wait_input();
    unsafe { ps2_outb(PS2_STATUS_REG, 0x60); } // Write command byte
    ps2_wait_input();
    unsafe { ps2_outb(PS2_DATA, config_new); }

    // Step 3: Enable cascade (IRQ2) on master PIC so IRQ12 can fire
    unsafe {
        let mask = ps2_inb(0x21);
        ps2_outb(0x21, mask & !0x04); // clear bit 2 → enable IRQ2
    }

    // Step 4: Reset mouse
    ps2_wait_input();
    unsafe { ps2_outb(PS2_DATA, 0xFF); } // Mouse reset
    ps2_wait_output();
    let ack = unsafe { ps2_inb(PS2_DATA) }; // Should be 0xFA (ACK) or 0xAA (BAT OK)

    // Step 5: Enable data reporting
    ps2_wait_input();
    unsafe { ps2_outb(PS2_DATA, 0xF4); } // Enable data reporting
    ps2_wait_output();
    let _ack2 = unsafe { ps2_inb(PS2_DATA) }; // 0xFA = ACK

    // Register IRQ12 handler (vector 44, IRQ12)
    crate::arch::idt::register_irq(12, irq12_handler);

    crate::dev::console::serial_write("[keyboard] PS/2 mouse initialized (IRQ12 enabled)\n");
}
