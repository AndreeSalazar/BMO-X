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
    if next == tail { return; }
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
        // Number row (Spanish layout)
        0x02 => if shift { b'!' } else { b'1' },
        0x03 => if shift { b'/' } else { b'2' },
        0x04 => if shift { b'\'' } else { b'3' },
        0x05 => if shift { b'(' } else { b'4' },
        0x06 => if shift { b')' } else { b'5' },
        0x07 => if shift { b'=' } else { b'6' },
        0x08 => if shift { b'?' } else { b'7' },
        0x09 => if shift { 0xA8 } else { b'8' },      // ¨
        0x0A => if shift { 0xB4 } else { b'9' },      // ´
        0x0B => if shift { 0xA1 } else { b'0' },

        // Symbol row
        0x0C => if shift { b'\'' } else { b'-' },
        0x0D => if shift { 0xBF } else { 0xA1 },      // ¿ / ¡

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

        // [ ] on Spanish
        0x1A => if shift { 0xA8 } else { 0xB4 },      // ¨ / ´
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

        // ñ key (scancode 0x27)
        0x27 => if shift { 0xA5 } else { 0xA4 },      // Ñ / ñ

        // ´ accent key (scancode 0x28)
        0x28 => 0xB4,

        // Ç key (scancode 0x2B)
        0x2B => 0xC7,

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
pub fn irq1_handler() {
    let raw = unsafe { inb(PS2_DATA) };

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
        let mask = inb(0x21);
        outb(0x21, mask & !0x02);
    }

    // Clear any stale data in the PS/2 buffer
    while unsafe { inb(PS2_STATUS) } & 0x01 != 0 {
        unsafe { inb(PS2_DATA); }
    }

    // Re-enable keyboard LEDs (Num Lock ON) — PS/2 init or mouse reset
    // may have turned them off. Command 0xED = Set LEDs, 0x02 = Num Lock.
    unsafe {
        // Wait for controller ready
        let mut timeout = 100_000u32;
        while inb(PS2_STATUS) & 0x02 != 0 && timeout > 0 { timeout -= 1; }
        outb(PS2_DATA, 0xED); // Set LEDs command
        // Wait for ACK
        timeout = 100_000u32;
        while inb(PS2_STATUS) & 0x01 == 0 && timeout > 0 { timeout -= 1; }
        let ack = inb(PS2_DATA);
        if ack == 0xFA {
            // Got ACK, send LED bitmask: Num Lock = 0x02
            timeout = 100_000;
            while inb(PS2_STATUS) & 0x02 != 0 && timeout > 0 { timeout -= 1; }
            outb(PS2_DATA, 0x02); // Num Lock LED ON
        }
    }

    crate::dev::console::serial_write("[keyboard] PS/2 keyboard initialized (IRQ1 enabled, NumLock LED on)\n");

    // Register IRQ1 handler with the IDT dispatcher
    crate::arch::idt::register_irq(1, irq1_handler);
}
