//! gdi32.dll compatibility — Device contexts, Bitmaps, Text, Fonts, Brushes.

#![allow(dead_code)]

/// TextOutA — draw text using the current font.
#[no_mangle]
pub extern "C" fn TextOutA(_dc: u64, _x: i32, _y: i32, _str: u64, _len: i32) -> u64 {
    // TODO: render text via BMO framebuffer
    1
}

/// TextOutW — draw text (UTF-16).
#[no_mangle]
pub extern "C" fn TextOutW(_dc: u64, _x: i32, _y: i32, _str: u64, _len: i32) -> u64 { 1 }

/// DrawTextA — draw formatted text.
#[no_mangle]
pub extern "C" fn DrawTextA(_dc: u64, _str: u64, _len: i32, _rect: u64, _format: u32) -> i32 { 0 }

/// DrawTextW — draw formatted text (UTF-16).
#[no_mangle]
pub extern "C" fn DrawTextW(_dc: u64, _str: u64, _len: i32, _rect: u64, _format: u32) -> i32 { 0 }

/// CreateSolidBrush — create a solid color brush.
#[no_mangle]
pub extern "C" fn CreateSolidBrush(_color: u32) -> u64 { 1 }

/// CreateFontIndirectA — create a font from LOGFONT.
#[no_mangle]
pub extern "C" fn CreateFontIndirectA(_logfont: u64) -> u64 { 1 }

/// CreateFontIndirectW — create a font from LOGFONT (UTF-16).
#[no_mangle]
pub extern "C" fn CreateFontIndirectW(_logfont: u64) -> u64 { 1 }

/// SelectObject — select an object into a DC.
#[no_mangle]
pub extern "C" fn SelectObject(_dc: u64, _obj: u64) -> u64 { 0 }

/// DeleteObject — delete a GDI object.
#[no_mangle]
pub extern "C" fn DeleteObject(_obj: u64) -> u64 { 1 }

/// BitBlt — block transfer between DCs.
#[no_mangle]
pub extern "C" fn BitBlt(
    _dst_dc: u64, _x: i32, _y: i32, _w: i32, _h: i32,
    _src_dc: u64, _sx: i32, _sy: i32, _rop: u32,
) -> u64 { 1 }

/// GetTextExtentPoint32A — get text dimensions.
#[no_mangle]
pub extern "C" fn GetTextExtentPoint32A(
    _dc: u64, _str: u64, _len: i32, _size: u64,
) -> u64 {
    if _size != 0 {
        unsafe {
            let s = &mut *(_size as *mut (i32, i32));
            *s = (_len * 8, 16); // 8x16 font
        }
    }
    1
}

/// GetDeviceCaps — get device capability.
#[no_mangle]
pub extern "C" fn GetDeviceCaps(_dc: u64, _index: i32) -> i32 {
    match _index {
        8 => 96,   // LOGPIXELSX
        10 => 96,  // LOGPIXELSY
        14 => 24,  // BITSPIXEL
        _ => 0,
    }
}
