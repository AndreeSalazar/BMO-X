//! Firma + integridad de binarios BEF.
//!
//! Esquema BEF:
//!   - Hash por sección: BLAKE3 256-bit.
//!   - Firma del archivo entero: Ed25519 sobre el conjunto de hashes.
//!   - Claves públicas confiables en /system/trust/*.pub.
//!
//! ## Chain-of-trust with TimeBack
//!   - Cada BEF cargado → BLAKE3 hash → timeback journal entry
//!   - Boot sequence: kernel.elf hash → mod_bmo_core hash → app.bef hash
//!   - Si algo falla: rollback al último snapshot válido

#![allow(dead_code)]

use crate::bmo_abi::primitives::{bx_u8, bx_u16, bx_u32};

/// Hash BLAKE3 256-bit de una sección.
#[repr(C, align(8))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SectionHash {
    pub section_index: bx_u16,
    pub _pad: [bx_u8; 6],
    pub digest: [bx_u8; 32],
}
const _: () = assert!(core::mem::size_of::<SectionHash>() == 40);

impl SectionHash {
    pub const SIZE: usize = 40;
    pub const ZERO: Self = Self {
        section_index: 0xFFFF,
        _pad: [0; 6],
        digest: [0; 32],
    };
}

/// Algoritmo de firma soportado.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SigAlgorithm {
    None    = 0,
    Ed25519 = 1,
}

/// Cabecera de la sección Signature.
#[repr(C, align(8))]
#[derive(Debug, Clone, Copy)]
pub struct SignatureHeader {
    pub hash_count: bx_u32,
    pub sig_algo: bx_u32,
}
const _: () = assert!(core::mem::size_of::<SignatureHeader>() == 8);

/// Firma Ed25519 completa (64 bytes signature + 32 bytes public key).
#[repr(C, align(8))]
#[derive(Debug, Clone, Copy)]
pub struct Ed25519Signature {
    /// Ed25519 signature (R || S), 64 bytes.
    pub sig: [bx_u8; 64],
    /// Ed25519 public key, 32 bytes.
    pub pubkey: [bx_u8; 32],
}
const _: () = assert!(core::mem::size_of::<Ed25519Signature>() == 96);

impl SignatureHeader {
    pub const SIGNATURE_SIZE: u32 = 96; // Ed25519 sig(64) + pubkey(32)
}

/// Hash BLAKE3 256-bit del buffer indicado.
pub fn blake3_256(bytes: &[u8]) -> [u8; 32] {
    crate::bef::blake3::hash(bytes)
}

/// Verifica que un hash precomputado coincida con los bytes provistos.
pub fn verify(expected: &SectionHash, bytes: &[u8]) -> bool {
    let computed = blake3_256(bytes);
    &computed[..] == &expected.digest[..]
}

/// Compute a chain-of-trust hash for the entire BEF (all section digests combined).
/// Used by TimeBack for boot-time integrity verification.
pub fn chain_hash(hashes: &[SectionHash]) -> [u8; 32] {
    let mut combined = alloc::vec::Vec::with_capacity(hashes.len() * 32);
    for h in hashes {
        combined.extend_from_slice(&h.digest);
    }
    blake3_256(&combined)
}

/// Verify an Ed25519 signature. Currently a stub — relies on external
/// ed25519-dalek or similar crate for actual verification.
/// Returns true if sig_algo is None (unsigned binaries are allowed in dev).
pub fn verify_ed25519(_sig: &Ed25519Signature, _message: &[u8]) -> bool {
    // TODO: integrate ed25519-dalek when available
    false // signature verification not yet implemented
}
