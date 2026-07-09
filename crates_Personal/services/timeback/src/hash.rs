//! `timeback::hash` — 20-byte content hash (FNV-1a 160-bit, Git-like).
//!
//! FNV-1a is fast, has good distribution, and is deterministic across
//! builds (unlike SHA which has optional hardware acceleration). We use a
//! 160-bit hash (5 × 32-bit FNV-1a rounds) to mimic SHA-1 sizing for
//! Git compatibility.

use core::fmt;

const FNV_OFFSET: u32 = 0x811C9DC5;
const FNV_PRIME: u32 = 0x01000193;

/// 20-byte hash.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Hash(pub [u8; 20]);

impl Hash {
    pub const ZERO: Hash = Hash([0u8; 20]);

    pub fn new() -> Self { Self([0u8; 20]) }

    /// Compute the hash of a byte slice.
    pub fn of(data: &[u8]) -> Hash {
        let mut h = [FNV_OFFSET; 5];
        for (i, &b) in data.iter().enumerate() {
            let slot = i % 5;
            h[slot] ^= b as u32;
            h[slot] = h[slot].wrapping_mul(FNV_PRIME);
        }
        let mut out = [0u8; 20];
        for i in 0..5 {
            let bytes = h[i].to_le_bytes();
            out[i*4..i*4+4].copy_from_slice(&bytes);
        }
        Hash(out)
    }

    /// Parse from a 40-char hex string.
    pub fn from_hex(s: &str) -> Option<Hash> {
        if s.len() != 40 { return None; }
        let mut out = [0u8; 20];
        for i in 0..20 {
            let byte = u8::from_str_radix(&s[i*2..i*2+2], 16).ok()?;
            out[i] = byte;
        }
        Some(Hash(out))
    }

    /// Hex string representation (40 chars).
    pub fn to_hex(&self) -> alloc::string::String {
        let mut s = alloc::string::String::with_capacity(40);
        for &b in &self.0 {
            let h = (b >> 4) & 0xF;
            let l = b & 0xF;
            s.push(if h < 10 { (b'0' + h) as char } else { (b'a' + h - 10) as char });
            s.push(if l < 10 { (b'0' + l) as char } else { (b'a' + l - 10) as char });
        }
        s
    }

    /// Short hex (7 chars, like Git).
    pub fn short(&self) -> alloc::string::String {
        let mut s = self.to_hex();
        s.truncate(7);
        s
    }

    /// Raw 20 bytes.
    pub fn bytes(&self) -> &[u8; 20] { &self.0 }

    /// Is this the zero hash (uninitialized)?
    pub fn is_zero(&self) -> bool { self.0 == [0u8; 20] }
}

impl fmt::Display for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

impl fmt::Debug for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Hash({})", self.short())
    }
}
