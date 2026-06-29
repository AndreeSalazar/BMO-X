//! Framebuffer driver — UEFI GOP + static backbuffer.

use fastos_boot_protocol::PixelFormat;

/// Minimal Ring 0 framebuffer handle for backbuffer blits.
#[derive(Debug, Clone, Copy)]
pub struct Framebuffer {
    base: u64,
    stride: u64,
    width: u32,
    height: u32,
}

impl Framebuffer {
    pub fn new(base: u64, stride: u64, width: u32, height: u32) -> Self {
        Self { base, stride, width, height }
    }

    pub fn blit_to(&self, dest: &Framebuffer) {
        let src = self.base as *const u32;
        let dst = dest.base as *mut u32;
        let copy_w = self.width.min(dest.width) as usize;
        let copy_h = self.height.min(dest.height) as usize;
        let src_stride = (self.stride / 4) as usize;
        let dst_stride = (dest.stride / 4) as usize;
        for y in 0..copy_h {
            for x in 0..copy_w {
                unsafe {
                    let pixel = *src.add(y * src_stride + x);
                    *dst.add(y * dst_stride + x) = pixel;
                }
            }
        }
    }
}

/// ARGB color (32-bit: 0xAARRGGBB).
#[derive(Debug, Clone, Copy)]
pub struct Color(pub u32);

impl Color {
    pub const BLACK:   Color = Color(0xFF000000);
    pub const WHITE:   Color = Color(0xFFFFFFFF);
    pub const RED:     Color = Color(0xFFFF0000);
    pub const GREEN:   Color = Color(0xFF00FF00);
    pub const BLUE:    Color = Color(0xFF0000FF);
    pub const YELLOW:  Color = Color(0xFFFFFF00);
    pub const CYAN:    Color = Color(0xFF00FFFF);
    pub const MAGENTA: Color = Color(0xFFFF00FF);
    pub const GRAY:    Color = Color(0xFF808080);
    pub const DARK_BG: Color = Color(0xFF1A1A2E);
    pub const ACCENT:  Color = Color(0xFF16213E);
    pub const HIGHLIGHT: Color = Color(0xFF0F3460);
    pub const NEON:    Color = Color(0xFFE94560);

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color(0xFF000000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32))
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Color(((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32))
    }

    pub const fn alpha(self) -> u8 { ((self.0 >> 24) & 0xFF) as u8 }
    pub const fn r(self) -> u8 { ((self.0 >> 16) & 0xFF) as u8 }
    pub const fn g(self) -> u8 { ((self.0 >> 8) & 0xFF) as u8 }
    pub const fn b(self) -> u8 { (self.0 & 0xFF) as u8 }

}

#[derive(Debug, Clone, Copy)]
pub struct GopDisplay {
    pub base: *mut u32,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub pixel_format: PixelFormat,
}

static mut GOP_DISPLAY: Option<GopDisplay> = None;

pub fn init() {
    // Initialized by init_gop (called from boot) with the values
    // delivered by the UEFI bootloader.
}

pub fn init_gop(fb_base: u64, width: u32, height: u32, stride: u32, pixel_format: PixelFormat) {
    unsafe {
        GOP_DISPLAY = Some(GopDisplay {
            base: fb_base as *mut u32,
            width,
            height,
            stride,
            pixel_format,
        });
    }
}

#[allow(static_mut_refs)]
pub fn display() -> Option<&'static GopDisplay> {
    unsafe { GOP_DISPLAY.as_ref() }
}

#[inline]
pub fn put_pixel(x: u32, y: u32, color: Color) {
    if let Some(disp) = display() {
        if x < disp.width && y < disp.height {
            unsafe {
                let offset = (y * disp.stride + x) as usize;
                *disp.base.add(offset) = color.0;
            }
        }
    }
}

pub fn fill_rect(x: u32, y: u32, w: u32, h: u32, color: Color) {
    if let Some(disp) = display() {
        for dy in 0..h {
            let py = y + dy;
            if py >= disp.height { break; }
            for dx in 0..w {
                let px = x + dx;
                if px >= disp.width { break; }
                unsafe {
                    let offset = (py * disp.stride + px) as usize;
                    *disp.base.add(offset) = color.0;
                }
            }
        }
    }
}



// ── Static Backbuffer ────────────────────────────────────────────────
const BACKBUFFER_WIDTH: usize = 1920;
const BACKBUFFER_HEIGHT: usize = 1080;
const BACKBUFFER_SIZE: usize = BACKBUFFER_WIDTH * BACKBUFFER_HEIGHT;
static mut BACKBUFFER_MEM: [u32; BACKBUFFER_SIZE] = [0; BACKBUFFER_SIZE];

#[allow(static_mut_refs)]
pub fn get_backbuffer_fb() -> Framebuffer {
    unsafe {
        let addr = BACKBUFFER_MEM.as_mut_ptr() as u64;
        let (width, height) = if let Some(disp) = display() {
            (
                (disp.width as usize).min(BACKBUFFER_WIDTH) as u32,
                (disp.height as usize).min(BACKBUFFER_HEIGHT) as u32,
            )
        } else {
            (BACKBUFFER_WIDTH as u32, BACKBUFFER_HEIGHT as u32)
        };
        Framebuffer::new(addr, (BACKBUFFER_WIDTH * 4) as u64, width, height)
    }
}

pub fn backbuffer_ptr() -> *mut u32 { unsafe { BACKBUFFER_MEM.as_mut_ptr() } }

pub fn present() {
    if let Some(disp) = display() {
        let backbuffer = get_backbuffer_fb();
        let dest = Framebuffer::new(
            disp.base as u64,
            (disp.stride as u64) * 4,
            disp.width,
            disp.height,
        );
        backbuffer.blit_to(&dest);
    }
}

pub fn clear_backbuffer(color: Color) { unsafe { BACKBUFFER_MEM = [color.0; BACKBUFFER_SIZE]; } }

pub fn get_backbuffer_pixel(x: u32, y: u32) -> Color {
    if (x as usize) < BACKBUFFER_WIDTH && (y as usize) < BACKBUFFER_HEIGHT {
        unsafe { Color(BACKBUFFER_MEM[y as usize * BACKBUFFER_WIDTH + x as usize]) }
    } else {
        Color::BLACK
    }
}

pub fn put_backbuffer_pixel(x: u32, y: u32, color: Color) {
    if (x as usize) < BACKBUFFER_WIDTH && (y as usize) < BACKBUFFER_HEIGHT {
        unsafe { BACKBUFFER_MEM[y as usize * BACKBUFFER_WIDTH + x as usize] = color.0; }
    }
}
