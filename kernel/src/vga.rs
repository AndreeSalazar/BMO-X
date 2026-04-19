//! VGA text-mode driver (80×25, 16 colors) at 0xB8000.

const VGA_BUFFER: usize = 0xB8000;
const VGA_WIDTH: usize = 80;
const VGA_HEIGHT: usize = 25;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum Color {
    Black = 0, Blue = 1, Green = 2, Cyan = 3,
    Red = 4, Magenta = 5, Brown = 6, LightGray = 7,
    DarkGray = 8, LightBlue = 9, LightGreen = 10, LightCyan = 11,
    LightRed = 12, Pink = 13, Yellow = 14, White = 15,
}

pub struct VgaWriter {
    pub col: usize,
    pub row: usize,
    color: u8,
}

impl VgaWriter {
    pub fn new() -> Self {
        Self { col: 0, row: 0, color: 0x07 }
    }

    pub fn clear(&mut self) {
        let buf = VGA_BUFFER as *mut u16;
        for i in 0..(VGA_WIDTH * VGA_HEIGHT) {
            unsafe { buf.add(i).write_volatile(0x0720); }
        }
        self.col = 0;
        self.row = 0;
    }

    pub fn write_byte(&mut self, byte: u8) {
        if byte == b'\n' { self.newline(); return; }
        if self.row >= VGA_HEIGHT { self.scroll(); }
        let off = self.row * VGA_WIDTH + self.col;
        let buf = VGA_BUFFER as *mut u16;
        unsafe { buf.add(off).write_volatile((self.color as u16) << 8 | byte as u16); }
        self.col += 1;
        if self.col >= VGA_WIDTH { self.newline(); }
    }

    pub fn write_str(&mut self, s: &str) {
        for b in s.bytes() { self.write_byte(b); }
    }

    pub fn write_str_color(&mut self, s: &str, fg: Color) {
        let old = self.color;
        self.color = fg as u8;
        self.write_str(s);
        self.color = old;
    }

    pub fn newline(&mut self) {
        self.col = 0;
        self.row += 1;
        if self.row >= VGA_HEIGHT { self.scroll(); }
    }

    fn scroll(&mut self) {
        let buf = VGA_BUFFER as *mut u16;
        for r in 1..VGA_HEIGHT {
            for c in 0..VGA_WIDTH {
                unsafe {
                    let v = buf.add(r * VGA_WIDTH + c).read_volatile();
                    buf.add((r - 1) * VGA_WIDTH + c).write_volatile(v);
                }
            }
        }
        for c in 0..VGA_WIDTH {
            unsafe { buf.add((VGA_HEIGHT - 1) * VGA_WIDTH + c).write_volatile(0x0720); }
        }
        self.row = VGA_HEIGHT - 1;
    }

    pub fn write_u64(&mut self, mut val: u64) {
        if val == 0 { self.write_byte(b'0'); return; }
        let mut buf = [0u8; 20];
        let mut i = 0;
        while val > 0 { buf[i] = b'0' + (val % 10) as u8; val /= 10; i += 1; }
        while i > 0 { i -= 1; self.write_byte(buf[i]); }
    }

    pub fn write_hex16(&mut self, val: u16) {
        const HEX: &[u8] = b"0123456789ABCDEF";
        for shift in (0..16).rev().step_by(4) {
            self.write_byte(HEX[((val >> shift) & 0xF) as usize]);
        }
    }

    pub fn write_hex32(&mut self, val: u32) {
        const HEX: &[u8] = b"0123456789ABCDEF";
        for shift in (0..32).rev().step_by(4) {
            self.write_byte(HEX[((val >> shift) & 0xF) as usize]);
        }
    }
}
