//! PS/2 Keyboard Driver — IRQ1, scan code set 1.
//! Ring 0, direct port I/O to 0x60/0x64.

use core::arch::asm;
use core::sync::atomic::{AtomicUsize, Ordering};

const KB_DATA: u16 = 0x60;
const KB_STATUS: u16 = 0x64;

/// Circular key buffer.
const BUF_SIZE: usize = 256;
static mut KEY_BUF: [u8; BUF_SIZE] = [0; BUF_SIZE];
static BUF_HEAD: AtomicUsize = AtomicUsize::new(0);
static BUF_TAIL: AtomicUsize = AtomicUsize::new(0);

/// Shift key state.
static mut SHIFT_HELD: bool = false;

/// Initialize PS/2 keyboard.
pub fn init_keyboard() {
    // Register IRQ1 handler
    super::super::arch::idt::register_irq(1, keyboard_irq_handler);

    // Flush any pending data
    while inb(KB_STATUS) & 0x01 != 0 {
        inb(KB_DATA);
    }
}

/// IRQ1 handler — called from IDT.
fn keyboard_irq_handler() {
    let scancode = inb(KB_DATA);

    // Track shift state
    unsafe {
        match scancode {
            0x2A | 0x36 => { SHIFT_HELD = true; return; }   // Shift pressed
            0xAA | 0xB6 => { SHIFT_HELD = false; return; }  // Shift released
            _ => {}
        }
    }

    // Ignore key releases (bit 7 set)
    if scancode & 0x80 != 0 {
        return;
    }

    // Convert scan code to ASCII
    let ascii = scancode_to_ascii(scancode);
    if ascii != 0 {
        buf_push(ascii);
    }
}

/// Try to read a key (non-blocking). Returns 0 if no key available.
pub fn try_read_key() -> u8 {
    buf_pop().unwrap_or(0)
}

/// Read a key (blocking — waits with HLT until key arrives).
pub fn read_key() -> u8 {
    loop {
        if let Some(key) = buf_pop() {
            return key;
        }
        unsafe { asm!("hlt"); }
    }
}

/// Read a full line (blocking). Returns when Enter is pressed.
/// Writes into the provided buffer, returns length.
pub fn read_line(buf: &mut [u8]) -> usize {
    let mut len = 0;
    loop {
        let key = read_key();
        match key {
            b'\n' => return len,
            8 => {
                // Backspace
                if len > 0 {
                    len -= 1;
                    // Signal backspace to caller
                    buf[len] = 0;
                }
            }
            _ => {
                if len < buf.len() - 1 {
                    buf[len] = key;
                    len += 1;
                }
            }
        }
    }
}

// ── Buffer operations ──────────────────────────────────────────────────────

fn buf_push(key: u8) {
    let head = BUF_HEAD.load(Ordering::Relaxed);
    let next = (head + 1) % BUF_SIZE;
    if next != BUF_TAIL.load(Ordering::Relaxed) {
        unsafe { KEY_BUF[head] = key; }
        BUF_HEAD.store(next, Ordering::Release);
    }
}

fn buf_pop() -> Option<u8> {
    let tail = BUF_TAIL.load(Ordering::Relaxed);
    if tail == BUF_HEAD.load(Ordering::Acquire) {
        return None;
    }
    let key = unsafe { KEY_BUF[tail] };
    BUF_TAIL.store((tail + 1) % BUF_SIZE, Ordering::Release);
    Some(key)
}

// ── Scan code set 1 → ASCII ────────────────────────────────────────────────

fn scancode_to_ascii(code: u8) -> u8 {
    let shifted = unsafe { SHIFT_HELD };

    let normal: u8 = match code {
        0x01 => 27,     // Esc
        0x02 => b'1', 0x03 => b'2', 0x04 => b'3', 0x05 => b'4',
        0x06 => b'5', 0x07 => b'6', 0x08 => b'7', 0x09 => b'8',
        0x0A => b'9', 0x0B => b'0',
        0x0C => b'-', 0x0D => b'=',
        0x0E => 8,      // Backspace
        0x0F => b'\t',
        0x10 => b'q', 0x11 => b'w', 0x12 => b'e', 0x13 => b'r',
        0x14 => b't', 0x15 => b'y', 0x16 => b'u', 0x17 => b'i',
        0x18 => b'o', 0x19 => b'p',
        0x1A => b'[', 0x1B => b']',
        0x1C => b'\n',  // Enter
        0x1E => b'a', 0x1F => b's', 0x20 => b'd', 0x21 => b'f',
        0x22 => b'g', 0x23 => b'h', 0x24 => b'j', 0x25 => b'k',
        0x26 => b'l',
        0x27 => b';', 0x28 => b'\'',
        0x29 => b'`',
        0x2B => b'\\',
        0x2C => b'z', 0x2D => b'x', 0x2E => b'c', 0x2F => b'v',
        0x30 => b'b', 0x31 => b'n', 0x32 => b'm',
        0x33 => b',', 0x34 => b'.', 0x35 => b'/',
        0x39 => b' ',   // Space
        _ => 0,
    };

    if normal == 0 { return 0; }

    if shifted {
        match normal {
            b'a'..=b'z' => normal - 32,  // Uppercase
            b'1' => b'!', b'2' => b'@', b'3' => b'#', b'4' => b'$',
            b'5' => b'%', b'6' => b'^', b'7' => b'&', b'8' => b'*',
            b'9' => b'(', b'0' => b')',
            b'-' => b'_', b'=' => b'+',
            b'[' => b'{', b']' => b'}',
            b';' => b':', b'\'' => b'"',
            b',' => b'<', b'.' => b'>', b'/' => b'?',
            b'\\' => b'|', b'`' => b'~',
            _ => normal,
        }
    } else {
        normal
    }
}

// ── Port I/O ───────────────────────────────────────────────────────────────

#[inline]
fn inb(port: u16) -> u8 {
    let val: u8;
    unsafe { asm!("in al, dx", out("al") val, in("dx") port, options(nostack, preserves_flags)); }
    val
}
