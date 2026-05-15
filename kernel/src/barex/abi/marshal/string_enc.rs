//! Estimaciones de tamaño para conversión UTF-8 ↔ UTF-16.
//!
//! Las conversiones reales viven en `barex::abi::compat`. Estos helpers
//! sólo dimensionan el buffer destino (worst case).

/// Bytes necesarios para encajar `n_utf8_bytes` codeunits UTF-8 como UTF-16.
/// Worst-case: cada byte ASCII → 1 codeunit (2 bytes).
#[inline(always)]
pub const fn utf8_to_utf16_estimate(n_utf8_bytes: usize) -> usize {
    // Cada codepoint U+0000..U+FFFF → 1 surrogate (2B). U+10000.. → 2 surrogates (4B).
    // Si todos los bytes son ASCII (1B → 2B), peor que UTF-16 base.
    // Si todos son emoji 4B → 4B también. Cap superior: n_utf8_bytes * 2.
    n_utf8_bytes * 2
}

/// Bytes necesarios para encajar `n_utf16_codeunits * 2` bytes como UTF-8.
/// Worst-case: cada codeunit BMP → hasta 3 bytes. Surrogate pair → 4 bytes (2 codeunits).
#[inline(always)]
pub const fn utf16_to_utf8_estimate(n_utf16_codeunits: usize) -> usize {
    n_utf16_codeunits * 3
}
