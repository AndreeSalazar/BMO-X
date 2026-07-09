use crate::dev::console;

const HEX_TABLE: &[u8; 16] = b"0123456789ABCDEF";

pub fn hex(val: u64) {
    console::serial_write("0x");
    for i in (0..16).rev() {
        console::serial_write_byte(HEX_TABLE[((val >> (i * 4)) & 0xF) as usize]);
    }
}

pub fn u64_dec(mut val: u64) {
    if val == 0 {
        console::serial_write_byte(b'0');
        return;
    }
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    while val > 0 {
        i -= 1;
        buf[i] = b'0' + (val % 10) as u8;
        val /= 10;
    }
    let s = core::str::from_utf8(&buf[i..]).unwrap_or("0");
    console::serial_write(s);
}

pub fn u32_dec(val: u32) {
    u64_dec(val as u64);
}

pub fn hex_bytes(data: &[u8]) {
    for &b in data {
        console::serial_write_byte(HEX_TABLE[(b >> 4) as usize]);
        console::serial_write_byte(HEX_TABLE[(b & 0xF) as usize]);
    }
}
