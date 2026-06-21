//! `fmt` — Formateo de strings del BMO ABI.
//!
//! Reemplaza `printf`, `sprintf`, `snprintf` de C con un sistema type-safe.
//! El formatter es un buffer stack-allocated (sin heap) con capacidad fija.

#![allow(dead_code)]

const FMT_BUF_SIZE: usize = 1024;

pub struct BmoFormatter {
    buf: [u8; FMT_BUF_SIZE],
    pos: usize,
}

impl BmoFormatter {
    pub const fn new() -> Self {
        Self { buf: [0; FMT_BUF_SIZE], pos: 0 }
    }

    pub fn write_str(&mut self, s: &str) {
        for &b in s.as_bytes() {
            if self.pos < FMT_BUF_SIZE {
                self.buf[self.pos] = b;
                self.pos += 1;
            }
        }
    }

    pub fn write_byte(&mut self, b: u8) {
        if self.pos < FMT_BUF_SIZE {
            self.buf[self.pos] = b;
            self.pos += 1;
        }
    }

    pub fn write_u64(&mut self, val: u64) {
        let mut tmp = [0u8; 20];
        let mut i = tmp.len();
        let mut v = val;
        if v == 0 {
            i -= 1;
            tmp[i] = b'0';
        } else {
            while v > 0 {
                i -= 1;
                tmp[i] = b'0' + (v % 10) as u8;
                v /= 10;
            }
        }
        self.write_str(core::str::from_utf8(&tmp[i..]).unwrap_or("0"));
    }

    pub fn write_i64(&mut self, val: i64) {
        if val < 0 {
            self.write_byte(b'-');
            self.write_u64((-(val as i128)) as u64);
        } else {
            self.write_u64(val as u64);
        }
    }

    pub fn write_hex(&mut self, val: u64) {
        self.write_str("0x");
        let mut hex_buf = [0u8; 16];
        for i in 0..16 {
            let nibble = (val >> (60 - i * 4)) & 0xF;
            hex_buf[i] = if nibble < 10 { b'0' + nibble as u8 } else { b'a' + (nibble - 10) as u8 };
        }
        self.write_str(core::str::from_utf8(&hex_buf).unwrap_or("0"));
    }

    pub fn write_bin(&mut self, val: u64, bits: u32) {
        self.write_str("0b");
        let mut found_one = false;
        let mut i = bits;
        while i > 0 {
            i -= 1;
            let bit = (val >> i) & 1;
            if bit == 1 { found_one = true; }
            if found_one || i == 0 {
                self.write_byte(if bit == 1 { b'1' } else { b'0' });
            }
        }
    }

    pub fn write_usize(&mut self, val: usize) {
        self.write_u64(val as u64);
    }

    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.buf[..self.pos]).unwrap_or("")
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.buf[..self.pos]
    }

    pub fn len(&self) -> usize {
        self.pos
    }

    pub fn is_empty(&self) -> bool {
        self.pos == 0
    }

    pub fn clear(&mut self) {
        self.pos = 0;
    }
}

impl Default for BmoFormatter {
    fn default() -> Self { Self::new() }
}

pub fn format_u64(val: u64) -> BmoFormatter {
    let mut f = BmoFormatter::new();
    f.write_u64(val);
    f
}

pub fn format_i64(val: i64) -> BmoFormatter {
    let mut f = BmoFormatter::new();
    f.write_i64(val);
    f
}

pub fn format_hex(val: u64) -> BmoFormatter {
    let mut f = BmoFormatter::new();
    f.write_hex(val);
    f
}
