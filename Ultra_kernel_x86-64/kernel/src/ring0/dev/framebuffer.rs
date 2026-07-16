//! Framebuffer driver ? UEFI GOP + static backbuffer.

/// Local pixel format enum (replaces the legacy `bmo_boot_protocol::PixelFormat`).
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    Bgr = 0,
    Rgb = 1,
    Unknown = 255,
}

/// Minimal Ring 0 framebuffer handle for backbuffer blits.
#[derive(Debug, Clone, Copy)]
pub struct Framebuffer {
    base: u64,
    /// Stride in **bytes** (UEFI GOP returns stride in pixels, multiply by 4).
    stride_bytes: u64,
    width: u32,
    height: u32,
    /// Pixel format of THIS framebuffer (backbuffer is always XRGB8888).
    format: PixelFormat,
}

impl Framebuffer {
    pub fn new(base: u64, stride_bytes: u64, width: u32, height: u32, format: PixelFormat) -> Self {
        Self { base, stride_bytes, width, height, format }
    }

    /// Convert a 0xAARRGGBB value to the target pixel format.
    /// BGR and RGB swap bytes 0/2 (red <-> blue) but keep alpha at byte 3.
    #[inline]
    fn convert_color(c: u32, dst_fmt: PixelFormat) -> u32 {
        match dst_fmt {
            PixelFormat::Rgb  => c,
            PixelFormat::Bgr  => (c & 0xFF00_FF00) | ((c & 0xFF) << 16) | ((c >> 16) & 0xFF),
            _ => c,
        }
    }

    pub fn blit_to(&self, dest: &Framebuffer) {
        let src = self.base as *const u32;
        let dst = dest.base as *mut u32;
        let copy_w = self.width.min(dest.width) as usize;
        let copy_h = self.height.min(dest.height) as usize;
        let src_stride = (self.stride_bytes / 4) as usize;
        let dst_stride = (dest.stride_bytes / 4) as usize;
        let same_format = self.format as u32 == dest.format as u32;
        for y in 0..copy_h {
            for x in 0..copy_w {
                unsafe {
                    let pixel = *src.add(y * src_stride + x);
                    let out = if same_format { pixel } else { Self::convert_color(pixel, dest.format) };
                    *dst.add(y * dst_stride + x) = out;
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
    if fb_base == 0 || width == 0 || height == 0 || stride == 0 {
        crate::ring0::dev::console::serial_write("[fb] invalid GOP geometry: base=");
        crate::ring0::dev::console::serial_write_u64(fb_base, 16);
        crate::ring0::dev::console::serial_write(" w=");
        crate::ring0::dev::console::serial_write_u64(width as u64, 10);
        crate::ring0::dev::console::serial_write(" h=");
        crate::ring0::dev::console::serial_write_u64(height as u64, 10);
        crate::ring0::dev::console::serial_write(" stride=");
        crate::ring0::dev::console::serial_write_u64(stride as u64, 10);
        crate::ring0::dev::console::serial_write("\n");
        return;
    }
    unsafe {
        GOP_DISPLAY = Some(GopDisplay {
            base: fb_base as *mut u32,
            width,
            height,
            stride,
            pixel_format,
        });
    }
    crate::ring0::dev::console::serial_write("[fb] GOP initialized: ");
    crate::ring0::dev::console::serial_write_u64(width as u64, 10);
    crate::ring0::dev::console::serial_write("x");
    crate::ring0::dev::console::serial_write_u64(height as u64, 10);
    crate::ring0::dev::console::serial_write(" stride=");
    crate::ring0::dev::console::serial_write_u64(stride as u64, 10);
    crate::ring0::dev::console::serial_write("px base=0x");
    crate::ring0::dev::console::serial_write_u64(fb_base, 16);
    crate::ring0::dev::console::serial_write(" fmt=");
    crate::ring0::dev::console::serial_write_u64(pixel_format as u64, 10);
    crate::ring0::dev::console::serial_write("\n");
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



// ?????? Static Backbuffer ????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????
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
        // Backbuffer is always XRGB8888 (PixelFormat::Rgb == 0)
        Framebuffer::new(addr, (BACKBUFFER_WIDTH * 4) as u64, width, height, PixelFormat::Rgb)
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
            disp.pixel_format,
        );
        backbuffer.blit_to(&dest);
        // Store fence: WC framebuffer writes MUST hit VRAM before consumer (display HW) sees them
        unsafe { core::arch::asm!("mfence", options(nostack)); }
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
