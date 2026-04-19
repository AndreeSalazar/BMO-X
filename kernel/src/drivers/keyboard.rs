//! PS/2 Keyboard Driver — Polling mode (no IRQ).
//! Ring 0, direct port I/O to 0x60/0x64.
//! No interrupts required — reads by polling status register.

use core::arch::asm;

const KB_DATA: u16 = 0x60;
const KB_STATUS: u16 = 0x64;

/// Shift key state.
static mut SHIFT_HELD: bool = false;

/// Initialize PS/2 keyboard (polling mode — no IRQ needed).
pub fn init_keyboard() {
    // Flush any pending data in the PS/2 buffer
    while inb(KB_STATUS) & 0x01 != 0 {
        inb(KB_DATA);
    }
}

/// Try to read a key (non-blocking). Returns 0 if no key available.
pub fn try_read_key() -> u8 {
    // Check if data is available in the PS/2 output buffer
    if inb(KB_STATUS) & 0x01 == 0 {
        return 0;
    }

    let scancode = inb(KB_DATA);

    // Track shift state
    unsafe {
        match scancode {
            0x2A | 0x36 => { SHIFT_HELD = true; return 0; }
            0xAA | 0xB6 => { SHIFT_HELD = false; return 0; }
            _ => {}
        }
    }

    // Ignore key releases (bit 7 set)
    if scancode & 0x80 != 0 {
        return 0;
    }

    scancode_to_ascii(scancode)
}

/// Read a key (blocking — waits with HLT until key arrives).
pub fn read_key() -> u8 {
    loop {
        let key = try_read_key();
        if key != 0 {
            return key;
        }
        // Brief pause to avoid hammering the port
        for _ in 0..10000u32 { core::hint::spin_loop(); }
    }
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
