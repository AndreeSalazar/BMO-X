//! `defense::verifier` — Validación de integridad.

#![allow(dead_code)]

/// Verifica el checksum simple (suma de bytes módulo 256) sobre `bytes`.
/// v1.8.8: stub — el BEF real aún no tiene campo de checksum.
pub fn checksum_ok(bytes: &[u8]) -> bool {
    // Por ahora cualquier BEF con magic correcto pasa.
    bytes.len() >= 4 && &bytes[0..4] == b"BEF1"
}

/// Hash FNV-1a de 64 bits sobre `bytes` (para identificación rápida).
pub fn fnv1a_64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}
