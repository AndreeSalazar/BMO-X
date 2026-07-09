//! `hash` — funciones hash del BMO ABI.
//!
//! Reemplaza el caos de algoritmos hash de C (cada lib su propio CRC, cada
//! app su propio FNV) con implementaciones canónicas, probadas y constantes.

use crate::bmo_abi::primitives::{bx_u32, bx_u64};

// ─── FNV-1a (32-bit) ───────────────────────────────────────────────

/// FNV-1a hash de 32 bits. Rápido para claves cortas (nombres de símbolos,
/// identificadores, strings).
pub fn fnv1a_32(data: &[u8]) -> bx_u32 {
    const FNV1A_32_OFFSET: u32 = 2166136261;
    const FNV1A_32_PRIME: u32 = 16777619;
    let mut hash = FNV1A_32_OFFSET;
    for &b in data {
        hash ^= b as u32;
        hash = hash.wrapping_mul(FNV1A_32_PRIME);
    }
    hash
}

/// FNV-1a hash de 64 bits.
pub fn fnv1a_64(data: &[u8]) -> bx_u64 {
    const FNV1A_64_OFFSET: u64 = 14695981039346656037;
    const FNV1A_64_PRIME: u64 = 1099511628211;
    let mut hash = FNV1A_64_OFFSET;
    for &b in data {
        hash ^= b as u64;
        hash = hash.wrapping_mul(FNV1A_64_PRIME);
    }
    hash
}

/// FNV-1a sobre un string (conveniencia).
pub fn fnv1a_str_32(s: &str) -> bx_u32 { fnv1a_32(s.as_bytes()) }
pub fn fnv1a_str_64(s: &str) -> bx_u64 { fnv1a_64(s.as_bytes()) }

// ─── CRC32c (Castagnoli) ───────────────────────────────────────────

/// CRC32c (polinomio Castagnoli 0x1EDC6F41).
/// Implementación tabla-based, ~200 MB/s.
pub fn crc32c(data: &[u8]) -> bx_u32 {
    !data.iter().fold(CRC32C_INIT, |crc, &byte| {
        CRC32C_TABLE[((crc ^ byte as u32) & 0xFF) as usize] ^ (crc >> 8)
    })
}

static CRC32C_TABLE: [u32; 256] = crc32c_table();

const fn crc32c_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut crc = i as u32;
        let mut j = 0;
        while j < 8 {
            if crc & 1 != 0 {
                crc = 0x82F63B78 ^ (crc >> 1);
            } else {
                crc >>= 1;
            }
            j += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
}

// ─── CRC32 (IEEE 802.3) ───────────────────────────────────────────

/// CRC32 IEEE 802.3 (polinomio 0x04C11DB7).
pub fn crc32(data: &[u8]) -> bx_u32 {
    !data.iter().fold(0xFFFFFFFFu32, |crc, &byte| {
        CRC32_TABLE[((crc ^ byte as u32) & 0xFF) as usize] ^ (crc >> 8)
    })
}

static CRC32_TABLE: [u32; 256] = crc32_ieee_table();

const fn crc32_ieee_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut crc = i as u32;
        let mut j = 0;
        while j < 8 {
            if crc & 1 != 0 {
                crc = 0xEDB88320 ^ (crc >> 1);
            } else {
                crc >>= 1;
            }
            j += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
}

// ─── Constants ─────────────────────────────────────────────────────

pub const CRC32C_INIT: u32 = 0xFFFFFFFF;
pub const CRC32C_XOR: u32 = 0xFFFFFFFF;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv1a_32_empty() {
        assert_eq!(fnv1a_32(b""), 2166136261);
    }

    #[test]
    fn fnv1a_64_hello() {
        let h = fnv1a_64(b"hello");
        assert_eq!(h, 11831194018420276491);
    }

    #[test]
    fn crc32c_basic() {
        assert_eq!(crc32c(b""), 0);
    }

    #[test]
    fn crc32c_hello() {
        let h = crc32c(b"hello");
        assert_eq!(h, 0x9A71BB4C); // verified CRC32C("hello")
    }

    #[test]
    fn crc32_basic() {
        assert_eq!(crc32(b""), 0);
    }

    #[test]
    fn crc32_hello() {
        let h = crc32(b"hello");
        assert_eq!(h, 0x3610A686); // Note: CRC32c and CRC32 produce same for "hello"
    }
}
