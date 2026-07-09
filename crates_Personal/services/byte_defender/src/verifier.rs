//! `defense::verifier` — BEF integrity verification.
//!
//! Validates BEF binaries before execution using FNV-1a hashing
//! (BLAKE3 requires large dependency; deferred to v2.0).

#![allow(dead_code)]

/// BEF magic bytes.
const BEF_MAGIC: &[u8; 4] = b"BEF1";

/// Verify BEF header integrity.
/// Checks: magic, version, section count sanity.
pub fn verify_header(bytes: &[u8]) -> bool {
    if bytes.len() < 48 { return false; }
    if &bytes[0..4] != BEF_MAGIC { return false; }
    // Version (bytes[4..8]) — any non-zero is valid
    let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    if version == 0 { return false; }
    // Section count sanity (bytes[8..12])
    let sec_count = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
    if sec_count > 256 { return false; }
    // Total size sanity (bytes[12..16])
    let total_size = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
    if total_size as usize > bytes.len() || total_size < 48 { return false; }
    true
}

/// Verify section table integrity within the BEF.
pub fn verify_sections(bytes: &[u8]) -> bool {
    if bytes.len() < 48 { return false; }
    let sec_count = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
    let sec_table_off = 48usize;
    let sec_entry_size = 16usize; // offset(4) + size(4) + flags(4) + hash(4)
    if sec_table_off + sec_count * sec_entry_size > bytes.len() { return false; }

    for i in 0..sec_count {
        let base = sec_table_off + i * sec_entry_size;
        if base + 16 > bytes.len() { return false; }
        let off = u32::from_le_bytes([bytes[base], bytes[base+1], bytes[base+2], bytes[base+3]]) as usize;
        let size = u32::from_le_bytes([bytes[base+4], bytes[base+5], bytes[base+6], bytes[base+7]]) as usize;
        if off + size > bytes.len() { return false; }
        // Hash field (bytes[base+12..16]) — validate non-zero
        let hash = u32::from_le_bytes([bytes[base+12], bytes[base+13], bytes[base+14], bytes[base+15]]);
        if hash != 0 {
            // Compare stored hash with computed FNV-1a of section data
            let section_data = &bytes[off..off + size];
            let computed = fnv1a_32(section_data);
            if computed != hash { return false; }
        }
    }
    true
}

/// Full BEF integrity check: header + sections.
pub fn checksum_ok(bytes: &[u8]) -> bool {
    verify_header(bytes) && verify_sections(bytes)
}

/// FNV-1a 32-bit hash for per-section integrity.
pub fn fnv1a_32(bytes: &[u8]) -> u32 {
    let mut h: u32 = 0x811c9dc5;
    for &b in bytes {
        h ^= b as u32;
        h = h.wrapping_mul(0x01000193);
    }
    h
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
