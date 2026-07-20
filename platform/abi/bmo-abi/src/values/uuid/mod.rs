//! `uuid` — BmoUuid, identificador único universal de 128 bits.
//!
//! Reemplaza `UUID` / `GUID` de COM/Win32 y `uuid_t` de POSIX.
//! Compatible con RFC 4122.

use crate::bmo_abi::primitives::{bx_u64, bx_u8};

/// UUID de 128 bits (RFC 4122).
///
/// # Layout (16 bytes)
/// ```text
/// [0..3]   time_low:               u32 (big-endian)
/// [4..5]   time_mid:               u16 (big-endian)
/// [6..7]   time_hi_and_version:    u16 (big-endian)
/// [8]      clock_seq_hi_and_reserved: u8
/// [9]      clock_seq_low:          u8
/// [10..15] node:                   [u8; 6]
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BmoUuid {
    bytes: [bx_u8; 16],
}
const _: () = assert!(core::mem::size_of::<BmoUuid>() == 16);

impl BmoUuid {
    pub const NIL: Self = Self { bytes: [0u8; 16] };

    pub const fn from_bytes(bytes: [bx_u8; 16]) -> Self {
        Self { bytes }
    }

    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.bytes
    }

    pub fn to_u64_pair(&self) -> (bx_u64, bx_u64) {
        let hi = u64::from_be_bytes([
            self.bytes[0],
            self.bytes[1],
            self.bytes[2],
            self.bytes[3],
            self.bytes[4],
            self.bytes[5],
            self.bytes[6],
            self.bytes[7],
        ]);
        let lo = u64::from_be_bytes([
            self.bytes[8],
            self.bytes[9],
            self.bytes[10],
            self.bytes[11],
            self.bytes[12],
            self.bytes[13],
            self.bytes[14],
            self.bytes[15],
        ]);
        (hi, lo)
    }

    pub fn from_u64_pair(hi: bx_u64, lo: bx_u64) -> Self {
        let mut bytes = [0u8; 16];
        bytes[0..8].copy_from_slice(&hi.to_be_bytes());
        bytes[8..16].copy_from_slice(&lo.to_be_bytes());
        Self { bytes }
    }

    /// Create a nil/zero UUID.
    pub const fn nil() -> Self {
        Self::NIL
    }

    pub fn is_nil(&self) -> bool {
        self.bytes == [0u8; 16]
    }
}

impl From<[u8; 16]> for BmoUuid {
    fn from(bytes: [u8; 16]) -> Self {
        Self { bytes }
    }
}

impl From<BmoUuid> for [u8; 16] {
    fn from(u: BmoUuid) -> Self {
        u.bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nil_uuid() {
        assert!(BmoUuid::nil().is_nil());
    }

    #[test]
    fn u64_pair_roundtrip() {
        let u = BmoUuid::from_bytes([
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E,
            0x0F, 0x10,
        ]);
        let (hi, lo) = u.to_u64_pair();
        let u2 = BmoUuid::from_u64_pair(hi, lo);
        assert_eq!(u, u2);
    }

    #[test]
    fn bytes_roundtrip() {
        let bytes = [0xAB; 16];
        let u = BmoUuid::from_bytes(bytes);
        assert_eq!(*u.as_bytes(), bytes);
    }
}
