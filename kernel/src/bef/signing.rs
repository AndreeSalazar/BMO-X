//! Firma + integridad de binarios BEF.
//!
//! Reemplaza:
//!   - PE: Authenticode (X.509 + PKCS#7) — pesado, slow, dependiente de CAs.
//!   - ELF: nada estándar (depende de envoltorios externos como signify).
//!
//! Esquema BEF:
//!   - **Hash** por sección: BLAKE3 256-bit (≈ 1 GB/s en Zen 3 single-thread).
//!   - **Firma** del archivo entero: Ed25519 sobre el conjunto de hashes.
//!   - Las claves públicas confiables viven en `/system/trust/*.pub`.

#![allow(dead_code)]

use crate::barex::abi::primitives::{bx_u8, bx_u16, bx_u32};

/// Hash BLAKE3 256-bit de una sección.
#[repr(C, align(8))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SectionHash {
    /// Índice de la sección que este hash describe.
    pub section_index: bx_u16,
    /// Padding.
    pub _pad: [bx_u8; 6],
    /// 32 bytes BLAKE3.
    pub digest: [bx_u8; 32],
}

impl SectionHash {
    pub const SIZE: usize = 40;
    pub const ZERO: Self = Self {
        section_index: 0xFFFF,
        _pad: [0; 6],
        digest: [0; 32],
    };
}

/// Cabecera de la sección Signature.
///
/// Si `sig_algo != 0`, después de los `hash_count` `SectionHash` viene una
/// firma de 64 bytes (Ed25519) seguida de 32 bytes de clave pública.
/// Total trailing = 96 bytes después de los hashes.
#[repr(C, align(8))]
#[derive(Debug, Clone, Copy)]
pub struct SignatureHeader {
    /// Cantidad de `SectionHash` que siguen.
    pub hash_count: bx_u32,
    /// Algoritmo de firma (1 = Ed25519, 0 = sin firma).
    pub sig_algo: bx_u32,
}

/// Stub de hash BLAKE3 (placeholder hasta integrar crate `blake3`).
///
/// FNV-1a expandido a 32 bytes — suficiente como interfaz; reemplazar por
/// blake3-rs cuando se permita una dep extra.
pub fn blake3_256(bytes: &[u8]) -> [u8; 32] {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    let mut digest = [0u8; 32];
    for (i, chunk) in bytes.chunks(8).enumerate() {
        let mut block = [0u8; 8];
        block[..chunk.len()].copy_from_slice(chunk);
        let v = u64::from_le_bytes(block);
        h ^= v;
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
        let pos = (i * 5) & 0x1F;
        digest[pos] ^= (h & 0xFF) as u8;
    }
    digest
}

/// Verifica que un hash precomputado coincida con los bytes provistos.
pub fn verify(expected: &SectionHash, bytes: &[u8]) -> bool {
    let computed = blake3_256(bytes);
    computed == expected.digest
}
