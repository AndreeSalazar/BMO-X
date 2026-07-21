//! Minimal PS/2 (i8042) keyboard reader for the Ring 0 shell.
//!
//! Polling, Scancode Set 1 (what the firmware leaves the controller in,
//! including USB-legacy emulation which most BIOSes keep alive in SMM
//! after ExitBootServices). This is a stopgap so the shell is usable from
//! the physical keyboard; the real keyboard driver will be a Ring 3 server
//! over a BMO Channel (F4). s2_mem already enabled the controller and sent
//! 0xF4 (enable scanning), so port 0x60 produces scancodes here.

const DATA: u16 = 0x60;
const STATUS: u16 = 0x64;

#[inline]
fn inb(port: u16) -> u8 {
    let v: u8;
    unsafe { core::arch::asm!("in al, dx", in("dx") port, out("al") v, options(nomem, nostack)); }
    v
}

/// Left/right shift held? Tracked across polls for upper/lower case.
static mut SHIFT: bool = false;

/// Poll the controller once. Returns an ASCII byte if a printable key (or
/// Enter/Backspace/Tab) was pressed since the last poll, else `None`.
/// Never blocks.
pub fn poll_ascii() -> Option<u8> {
    let status = inb(STATUS);
    if status & 0x01 == 0 {
        return None; // output buffer empty — no byte waiting
    }
    if status & 0x20 != 0 {
        // Bit 5 set = second-port (mouse) byte. Drain it and ignore so it
        // does not desync the keyboard stream.
        let _ = inb(DATA);
        return None;
    }
    let code = inb(DATA);
    match code {
        0x2A | 0x36 => { unsafe { SHIFT = true; } None }   // shift make
        0xAA | 0xB6 => { unsafe { SHIFT = false; } None }  // shift break
        c if c & 0x80 != 0 => None,                        // any other release
        c => translate(c, unsafe { SHIFT }),
    }
}

/// Scancode Set 1 make code → ASCII for a US QWERTY layout. `None` for keys
/// with no shell meaning (function keys, arrows, modifiers, keypad, ...).
fn translate(code: u8, shift: bool) -> Option<u8> {
    let c = match code {
        0x02 => if shift { b'!' } else { b'1' },
        0x03 => if shift { b'@' } else { b'2' },
        0x04 => if shift { b'#' } else { b'3' },
        0x05 => if shift { b'$' } else { b'4' },
        0x06 => if shift { b'%' } else { b'5' },
        0x07 => if shift { b'^' } else { b'6' },
        0x08 => if shift { b'&' } else { b'7' },
        0x09 => if shift { b'*' } else { b'8' },
        0x0A => if shift { b'(' } else { b'9' },
        0x0B => if shift { b')' } else { b'0' },
        0x0C => if shift { b'_' } else { b'-' },
        0x0D => if shift { b'+' } else { b'=' },
        0x0E => 0x08,  // Backspace
        0x0F => b'\t', // Tab
        0x10 => if shift { b'Q' } else { b'q' },
        0x11 => if shift { b'W' } else { b'w' },
        0x12 => if shift { b'E' } else { b'e' },
        0x13 => if shift { b'R' } else { b'r' },
        0x14 => if shift { b'T' } else { b't' },
        0x15 => if shift { b'Y' } else { b'y' },
        0x16 => if shift { b'U' } else { b'u' },
        0x17 => if shift { b'I' } else { b'i' },
        0x18 => if shift { b'O' } else { b'o' },
        0x19 => if shift { b'P' } else { b'p' },
        0x1A => if shift { b'{' } else { b'[' },
        0x1B => if shift { b'}' } else { b']' },
        0x1C => b'\r', // Enter
        0x1E => if shift { b'A' } else { b'a' },
        0x1F => if shift { b'S' } else { b's' },
        0x20 => if shift { b'D' } else { b'd' },
        0x21 => if shift { b'F' } else { b'f' },
        0x22 => if shift { b'G' } else { b'g' },
        0x23 => if shift { b'H' } else { b'h' },
        0x24 => if shift { b'J' } else { b'j' },
        0x25 => if shift { b'K' } else { b'k' },
        0x26 => if shift { b'L' } else { b'l' },
        0x27 => if shift { b':' } else { b';' },
        0x28 => if shift { b'"' } else { b'\'' },
        0x29 => if shift { b'~' } else { b'`' },
        0x2B => if shift { b'|' } else { b'\\' },
        0x2C => if shift { b'Z' } else { b'z' },
        0x2D => if shift { b'X' } else { b'x' },
        0x2E => if shift { b'C' } else { b'c' },
        0x2F => if shift { b'V' } else { b'v' },
        0x30 => if shift { b'B' } else { b'b' },
        0x31 => if shift { b'N' } else { b'n' },
        0x32 => if shift { b'M' } else { b'm' },
        0x33 => if shift { b'<' } else { b',' },
        0x34 => if shift { b'>' } else { b'.' },
        0x35 => if shift { b'?' } else { b'/' },
        0x39 => b' ', // Space
        _ => return None,
    };
    Some(c)
}
