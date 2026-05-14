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

/// Hash BLAKE3 256-bit del buffer indicado. Implementación nativa en
/// `crate::bef::blake3` (no_std, sin dependencias externas).
pub fn blake3_256(bytes: &[u8]) -> [u8; 32] {
    crate::bef::blake3::hash(bytes)
}

/// Verifica que un hash precomputado coincida con los bytes provistos.
pub fn verify(expected: &SectionHash, bytes: &[u8]) -> bool {
    let computed = blake3_256(bytes);
    computed == expected.digest
}
