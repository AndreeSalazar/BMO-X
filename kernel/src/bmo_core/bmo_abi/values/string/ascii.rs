//! Helpers ASCII para protocolos legacy (HTTP headers, USB string descriptors,
//! firmware logs). Reemplaza `<ctype.h>` con funciones `const`-evaluables.

use crate::bmo_core::bmo_abi::primitives::bx_u8;

#[inline(always)]
pub const fn is_digit(b: bx_u8) -> bool { b >= b'0' && b <= b'9' }

#[inline(always)]
pub const fn is_lower(b: bx_u8) -> bool { b >= b'a' && b <= b'z' }

#[inline(always)]
pub const fn is_upper(b: bx_u8) -> bool { b >= b'A' && b <= b'Z' }

#[inline(always)]
pub const fn is_alpha(b: bx_u8) -> bool { is_lower(b) || is_upper(b) }

#[inline(always)]
pub const fn is_alnum(b: bx_u8) -> bool { is_alpha(b) || is_digit(b) }

#[inline(always)]
pub const fn is_space(b: bx_u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0B | 0x0C)
}

#[inline(always)]
pub const fn is_hex(b: bx_u8) -> bool {
    is_digit(b) || (b >= b'a' && b <= b'f') || (b >= b'A' && b <= b'F')
}

#[inline(always)]
pub const fn is_ascii(b: bx_u8) -> bool {
    b < 0x80
}

#[inline(always)]
pub const fn to_lower(b: bx_u8) -> bx_u8 {
    if is_upper(b) { b + 32 } else { b }
}

#[inline(always)]
pub const fn to_upper(b: bx_u8) -> bx_u8 {
    if is_lower(b) { b - 32 } else { b }
}

#[inline(always)]
pub const fn to_uppercase(b: bx_u8) -> bx_u8 {
    if is_lower(b) { b - 32 } else { b }
}

#[inline(always)]
pub const fn to_lowercase(b: bx_u8) -> bx_u8 {
    if is_upper(b) { b + 32 } else { b }
}

pub const fn ascii_cmp(a: &[bx_u8], b: &[bx_u8], len: usize) -> bool {
    if a.len() < len || b.len() < len {
        return false;
    }
    let mut i = 0;
    while i < len {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

pub const fn hex_value(b: bx_u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}
