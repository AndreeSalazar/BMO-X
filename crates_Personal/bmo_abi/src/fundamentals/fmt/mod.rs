//! `fmt` — BmoFormatter, formateador stack-allocated del BMO ABI.
//!
//! Reemplaza `sprintf`/`snprintf` de C y `core::fmt` de Rust con un
//! buffer stack-allocated (sin heap) y un subset de formatos.
//!
//! Útil para logging en Ring 0 (donde no hay heap disponible) y para
//! construir mensajes de error cortos sin dependencias.

use crate::bmo_abi::primitives::bx_u64;
use crate::bmo_abi::fundamentals::memory::BmoSlice;

const DEFAULT_CAPACITY: usize = 256;

/// Formateador stack-allocated, sin heap.
///
/// Almacena el resultado en un buffer interno fijo. Si el resultado excede
/// el buffer, se trunca (con flag `truncated`).
pub struct BmoFormatter {
    buf: [u8; DEFAULT_CAPACITY],
    pos: usize,
    truncated: bool,
}

impl BmoFormatter {
    pub const fn new() -> Self {
        Self {
            buf: [0u8; DEFAULT_CAPACITY],
            pos: 0,
            truncated: false,
        }
    }

    pub fn clear(&mut self) {
        self.pos = 0;
        self.truncated = false;
    }

    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.buf[..self.pos]).unwrap_or("")
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.buf[..self.pos]
    }

    pub fn as_bmo_slice(&self) -> BmoSlice {
        BmoSlice::new(self.buf.as_ptr(), self.pos as bx_u64)
    }

    pub fn len(&self) -> usize { self.pos }
    pub fn is_empty(&self) -> bool { self.pos == 0 }
    pub fn is_truncated(&self) -> bool { self.truncated }

    /// Write a single byte.
    pub fn write_byte(&mut self, b: u8) {
        if self.pos < self.buf.len() {
            self.buf[self.pos] = b;
            self.pos += 1;
        } else {
            self.truncated = true;
        }
    }

    /// Write a string slice.
    pub fn write_str(&mut self, s: &str) {
        let bytes = s.as_bytes();
        let avail = self.buf.len() - self.pos;
        if bytes.len() <= avail {
            self.buf[self.pos..self.pos + bytes.len()].copy_from_slice(bytes);
            self.pos += bytes.len();
        } else {
            self.buf[self.pos..].copy_from_slice(&bytes[..avail]);
            self.pos = self.buf.len();
            self.truncated = true;
        }
    }

    /// Write a character.
    pub fn write_char(&mut self, c: char) {
        let mut buf = [0u8; 4];
        let s = c.encode_utf8(&mut buf);
        self.write_str(s);
    }

    /// Write an unsigned integer in decimal.
    pub fn write_u64(&mut self, v: u64) {
        let mut buf = [0u8; 20];
        let mut n = v;
        let mut i = 0;
        if n == 0 {
            self.write_byte(b'0');
            return;
        }
        while n > 0 {
            buf[i] = b'0' + (n % 10) as u8;
            n /= 10;
            i += 1;
        }
        for j in (0..i).rev() {
            self.write_byte(buf[j]);
        }
    }

    /// Write a signed integer in decimal.
    pub fn write_i64(&mut self, v: i64) {
        if v < 0 {
            self.write_byte(b'-');
            self.write_u64((-(v + 1)) as u64 + 1);
        } else {
            self.write_u64(v as u64);
        }
    }

    /// Write a hex integer (lowercase, no prefix).
    pub fn write_hex(&mut self, v: u64) {
        let hex_chars = b"0123456789abcdef";
        let mut buf = [0u8; 16];
        let mut n = v;
        let mut i = 0;
        if n == 0 {
            self.write_byte(b'0');
            return;
        }
        while n > 0 {
            buf[i] = hex_chars[(n & 0xF) as usize];
            n >>= 4;
            i += 1;
        }
        for j in (0..i).rev() {
            self.write_byte(buf[j]);
        }
    }

    /// Write a boolean.
    pub fn write_bool(&mut self, v: bool) {
        self.write_str(if v { "true" } else { "false" });
    }

    /// Write a pointer as hex with 0x prefix.
    pub fn write_ptr(&mut self, ptr: *const u8) {
        self.write_str("0x");
        self.write_hex(ptr as u64);
    }
}

// ─── Convenience macro ─────────────────────────────────────────────

/// Format into a `BmoFormatter`.
#[macro_export]
macro_rules! bmo_fmt {
    ($fmt:expr, $lit:literal $(, $arg:expr)* $(,)?) => {{
        $fmt.write_str($lit);
        $(
            bmo_fmt_append!($fmt, $arg);
        )*
    }};
}

#[macro_export]
macro_rules! bmo_fmt_append {
    ($fmt:expr, $v:expr) => {
        match &$v {
            v @ (0_u8..=255_u8) => $fmt.write_u64(*v as u64),
            v @ (0_u16..=65535_u16) => $fmt.write_u64(*v as u64),
            v @ (0_u32..=4294967295_u32) => $fmt.write_u64(*v as u64),
            v: u64 => $fmt.write_u64(*v),
            v: i64 => $fmt.write_i64(*v),
            v: i32 => $fmt.write_i64(*v as i64),
            v: bool => $fmt.write_bool(*v),
            v: &str => $fmt.write_str(v),
            v: char => $fmt.write_char(*v),
            _ => $fmt.write_str(&core::format_args!("{:?}", $v).to_string()),
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fmt_u64() {
        let mut f = BmoFormatter::new();
        f.write_u64(42);
        assert_eq!(f.as_str(), "42");
    }

    #[test]
    fn fmt_hex() {
        let mut f = BmoFormatter::new();
        f.write_hex(0xDEAD);
        assert_eq!(f.as_str(), "dead");
    }

    #[test]
    fn fmt_str() {
        let mut f = BmoFormatter::new();
        f.write_str("hello ");
        f.write_u64(42);
        assert_eq!(f.as_str(), "hello 42");
    }

    #[test]
    fn fmt_truncation() {
        let mut f = BmoFormatter::new();
        let long = "a".repeat(300);
        f.write_str(&long);
        assert!(f.is_truncated());
        assert_eq!(f.len(), 256);
    }

    #[test]
    fn fmt_negative() {
        let mut f = BmoFormatter::new();
        f.write_i64(-1234);
        assert_eq!(f.as_str(), "-1234");
    }

    #[test]
    fn fmt_bool() {
        let mut f = BmoFormatter::new();
        f.write_bool(true);
        assert_eq!(f.as_str(), "true");
    }
}
