//! Keyboard translation tables: PS/2 Set 1 scancode → VK code.

/// Translate a PS/2 Set 1 scancode to a Virtual Key (VK) code.
/// Returns (vk, released) where released = (sc & 0x80) != 0.
pub fn scancode_to_vk(sc: u8) -> Option<(u8, bool)> {
    let released = (sc & 0x80) != 0;
    let sc = sc & 0x7F;
    let vk = match sc {
        0x01 => 0x1B, // ESC
        0x0E => 0x08, // Backspace
        0x0F => 0x09, // Tab
        0x1C => 0x0D, // Enter
        0x1D => return None, // Ctrl
        0x2A => return None, // LShift
        0x36 => return None, // RShift
        0x38 => return None, // Alt
        0x39 => 0x20, // Space
        0x3A => return None, // CapsLock
        0x4B => 0x25, // Left
        0x4D => 0x27, // Right
        0x48 => 0x26, // Up
        0x50 => 0x28, // Down
        0x47 => 0x24, // Home
        0x4F => 0x23, // End
        0x49 => 0x21, // PageUp
        0x51 => 0x22, // PageDown
        0x52 => 0x2D, // Insert
        0x53 => 0x2E, // Delete
        0x3B => 0x70, // F1
        0x3C => 0x71, // F2
        0x3D => 0x72, // F3
        0x3E => 0x73, // F4
        0x3F => 0x74, // F5
        0x40 => 0x75, // F6
        0x41 => 0x76, // F7
        0x42 => 0x77, // F8
        0x43 => 0x78, // F9
        0x44 => 0x79, // F10
        0x57 => 0x7A, // F11
        0x58 => 0x7B, // F12
        0x9D => return None, // Ctrl release
        0xAA => return None, // LShift release
        0xB6 => return None, // RShift release
        0xB8 => return None, // Alt release
        0xBA => return None, // CapsLock release
        // Alphanumeric (Set 1: 0x02-0x0B = 1-0, 0x10-0x19 = Q-P, 0x1E-0x26 = A-L, 0x2C-0x32 = Z-M)
        c @ 0x02..=0x0B => c + b'1' - 2, // digits 1-0
        c @ 0x10..=0x19 => c + b'Q' - 0x10,
        c @ 0x1E..=0x26 => c + b'A' - 0x1E,
        c @ 0x2C..=0x32 => c + b'Z' - 0x2C,
        _ => return None,
    };
    Some((vk, released))
}

/// USB HID Usage ID → VK code translation table.
/// HID Usage IDs for keyboard boot protocol (0x04-0x65 range).
pub fn hid_usage_to_vk(usage: u8) -> Option<u8> {
    Some(match usage {
        0x04..=0x1D => usage - 4 + b'a', // a-z
        0x1E..=0x27 => usage - 0x1E + b'1', // 1-0
        0x28 => 0x0D, // Enter
        0x29 => 0x1B, // Escape
        0x2A => 0x08, // Backspace
        0x2B => 0x09, // Tab
        0x2C => 0x20, // Space
        0x4F => 0x27, // Right
        0x50 => 0x25, // Left
        0x51 => 0x28, // Down
        0x52 => 0x26, // Up
        0xE0 => return None, // Left Ctrl
        0xE1 => return None, // Left Shift
        0xE2 => return None, // Left Alt
        0xE6 => return None, // Right Ctrl
        _ => return None,
    })
}
