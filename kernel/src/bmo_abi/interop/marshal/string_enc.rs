//! Conversión UTF-8 ↔ UTF-16 ↔ ASCII C-string.
//!
//! UTF-16 es el encoding nativo de Win32 (WCHAR = u16 LE) y de Java/C#/.NET.
//! BMO usa UTF-8. La conversión se hace en runtime, sin asignación
//! dinámica (caller reserva el buffer).
//!
//! ## Endianness
//!
//! UTF-16 little-endian (UTF-16LE) es el estándar en x86/x86-64. La salida
//! de `utf16_from_utf8` es little-endian.

use crate::barex::BxError;
use crate::bmo_abi::primitives::bx_u32;

// ─── Estimaciones de tamaño (worst case) ─────────────────────────────

/// Bytes necesarios para encajar `n_utf8_bytes` codeunits UTF-8 como UTF-16.
/// Worst-case: cada byte ASCII → 1 codeunit (2 bytes).
#[inline(always)]
pub const fn utf8_to_utf16_estimate(n_utf8_bytes: usize) -> usize {
    n_utf8_bytes * 2
}

/// Bytes necesarios para encajar `n_utf16_bytes` bytes como UTF-8.
#[inline(always)]
pub const fn utf16_to_utf8_estimate(n_utf16_bytes: usize) -> usize {
    // Cada BMP → hasta 3 bytes. Surrogate pair (4 bytes) → 4 bytes. Cap: 3x.
    n_utf16_bytes * 3 / 2 + 1
}

// ─── UTF-8 → UTF-16 ──────────────────────────────────────────────────

/// Convierte un buffer UTF-8 a UTF-16LE.
///
/// - `src`: bytes UTF-8.
/// - `dst`: buffer destino UTF-16 (cada elemento es un u16).
/// - Retorna: cantidad de codeunits UTF-16 escritas (sin contar null terminator).
///
/// SAFETY: `src` debe ser UTF-8 válido. `dst` debe tener al menos
/// `utf8_to_utf16_estimate(src.len()) / 2` elementos.
pub fn utf16_from_utf8(src: &[u8], dst: &mut [u16]) -> Result<usize, BxError> {
    let mut di = 0;
    let mut si = 0;
    let src_bytes = src;

    while si < src_bytes.len() {
        let b = src_bytes[si];
        let (cp, advance) = decode_utf8(src_bytes, si)?;
        si += advance;

        if (cp as u32) < 0x10000 {
            // BMP → 1 codeunit
            if di >= dst.len() {
                return Err(BxError::BufferTooSmall);
            }
            dst[di] = cp as u16;
            di += 1;
        } else {
            // Supplemental plane → surrogate pair
            if di + 1 >= dst.len() {
                return Err(BxError::BufferTooSmall);
            }
            let cp_adj = (cp as u32) - 0x10000;
            let high = 0xD800 + ((cp_adj >> 10) as u16);
            let low  = 0xDC00 + ((cp_adj & 0x3FF) as u16);
            dst[di] = high;
            dst[di + 1] = low;
            di += 2;
        }
        let _ = b; // suppress warning when advance = 1 for ASCII
    }

    Ok(di)
}

fn decode_utf8(bytes: &[u8], i: usize) -> Result<(char, usize), BxError> {
    let b0 = bytes[i];
    if b0 < 0x80 {
        // ASCII
        return Ok((b0 as char, 1));
    }
    if b0 < 0xC2 {
        return Err(BxError::InvalidArgument);
    }
    if b0 < 0xE0 {
        // 2-byte sequence
        if i + 1 >= bytes.len() { return Err(BxError::InvalidArgument); }
        let b1 = bytes[i + 1];
        if (b1 & 0xC0) != 0x80 { return Err(BxError::InvalidArgument); }
        let cp = ((b0 & 0x1F) as u32) << 6 | (b1 & 0x3F) as u32;
        if cp < 0x80 { return Err(BxError::InvalidArgument); }
        let c = char::from_u32(cp).ok_or(BxError::InvalidArgument)?;
        Ok((c, 2))
    } else if b0 < 0xF0 {
        // 3-byte sequence
        if i + 2 >= bytes.len() { return Err(BxError::InvalidArgument); }
        let b1 = bytes[i + 1];
        let b2 = bytes[i + 2];
        if (b1 & 0xC0) != 0x80 || (b2 & 0xC0) != 0x80 {
            return Err(BxError::InvalidArgument);
        }
        let cp = ((b0 & 0x0F) as u32) << 12
               | ((b1 & 0x3F) as u32) << 6
               | (b2 & 0x3F) as u32;
        if cp < 0x800 { return Err(BxError::InvalidArgument); }
        let c = char::from_u32(cp).ok_or(BxError::InvalidArgument)?;
        Ok((c, 3))
    } else if b0 < 0xF5 {
        // 4-byte sequence
        if i + 3 >= bytes.len() { return Err(BxError::InvalidArgument); }
        let b1 = bytes[i + 1];
        let b2 = bytes[i + 2];
        let b3 = bytes[i + 3];
        if (b1 & 0xC0) != 0x80 || (b2 & 0xC0) != 0x80 || (b3 & 0xC0) != 0x80 {
            return Err(BxError::InvalidArgument);
        }
        let cp = ((b0 & 0x07) as u32) << 18
               | ((b1 & 0x3F) as u32) << 12
               | ((b2 & 0x3F) as u32) << 6
               | (b3 & 0x3F) as u32;
        if cp < 0x10000 || cp > 0x10FFFF { return Err(BxError::InvalidArgument); }
        let c = char::from_u32(cp).ok_or(BxError::InvalidArgument)?;
        Ok((c, 4))
    } else {
        Err(BxError::InvalidArgument)
    }
}

// ─── UTF-16 → UTF-8 ──────────────────────────────────────────────────

/// Convierte un buffer UTF-16LE a UTF-8.
///
/// - `src`: codeunits UTF-16LE.
/// - `dst`: buffer destino UTF-8.
/// - Retorna: cantidad de bytes UTF-8 escritos.
pub fn utf8_from_utf16(src: &[u16], dst: &mut [u8]) -> Result<usize, BxError> {
    let mut di = 0;
    let mut si = 0;

    while si < src.len() {
        let w1 = src[si];
        si += 1;

        if (w1 as u32) < 0xD800 || (w1 as u32) > 0xDFFF {
            // BMP (no surrogate)
            encode_utf8(w1 as u32, dst, &mut di)?;
        } else if (w1 as u32) < 0xDC00 {
            // High surrogate → esperar low surrogate
            if si >= src.len() {
                return Err(BxError::InvalidArgument);
            }
            let w2 = src[si];
            si += 1;
            if (w2 as u32) < 0xDC00 || (w2 as u32) > 0xDFFF {
                return Err(BxError::InvalidArgument);
            }
            let cp = 0x10000
                   + (((w1 as u32) - 0xD800) << 10)
                   + ((w2 as u32) - 0xDC00);
            encode_utf8(cp, dst, &mut di)?;
        } else {
            return Err(BxError::InvalidArgument);
        }
    }

    Ok(di)
}

fn encode_utf8(cp: u32, dst: &mut [u8], di: &mut usize) -> Result<(), BxError> {
    if cp < 0x80 {
        if *di >= dst.len() { return Err(BxError::BufferTooSmall); }
        dst[*di] = cp as u8;
        *di += 1;
    } else if cp < 0x800 {
        if *di + 1 >= dst.len() { return Err(BxError::BufferTooSmall); }
        dst[*di]     = 0xC0 | (cp >> 6) as u8;
        dst[*di + 1] = 0x80 | (cp & 0x3F) as u8;
        *di += 2;
    } else if cp < 0x10000 {
        if *di + 2 >= dst.len() { return Err(BxError::BufferTooSmall); }
        dst[*di]     = 0xE0 | (cp >> 12) as u8;
        dst[*di + 1] = 0x80 | ((cp >> 6) & 0x3F) as u8;
        dst[*di + 2] = 0x80 | (cp & 0x3F) as u8;
        *di += 3;
    } else if cp < 0x110000 {
        if *di + 3 >= dst.len() { return Err(BxError::BufferTooSmall); }
        dst[*di]     = 0xF0 | (cp >> 18) as u8;
        dst[*di + 1] = 0x80 | ((cp >> 12) & 0x3F) as u8;
        dst[*di + 2] = 0x80 | ((cp >> 6) & 0x3F) as u8;
        dst[*di + 3] = 0x80 | (cp & 0x3F) as u8;
        *di += 4;
    } else {
        return Err(BxError::InvalidArgument);
    }
    Ok(())
}

// ─── UTF-8 → ASCII C-string ─────────────────────────────────────────

/// Convierte UTF-8 a ASCII C-string (null-terminated).
///
/// Reemplaza bytes no-ASCII con `?`. Retorna longitud (sin null).
pub fn ascii_cstr_from_utf8(src: &[u8], dst: &mut [u8]) -> Result<usize, BxError> {
    if dst.is_empty() { return Err(BxError::BufferTooSmall); }
    let mut di = 0;
    for &b in src {
        if di + 1 >= dst.len() {
            return Err(BxError::BufferTooSmall);
        }
        dst[di] = if b < 0x80 { b } else { b'?' };
        di += 1;
    }
    dst[di] = 0;
    Ok(di)
}

// ─── UTF-8 → UTF-32 (scaffolding, no usado aún) ────────────────────

/// Tamaño en codeunits UTF-32 (siempre = #chars). Helper de planificación.
pub fn utf32_count(src: &[u8]) -> Result<usize, BxError> {
    let mut count = 0;
    let mut si = 0;
    while si < src.len() {
        let (_, advance) = decode_utf8(src, si)?;
        si += advance;
        count += 1;
    }
    Ok(count)
}

/// Trivial constant for layout probes.
pub const EMPTY_U32: bx_u32 = 0;
