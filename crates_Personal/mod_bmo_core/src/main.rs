//! mod_bmo_core — standalone Ring 3 desktop module loaded by kernel.elf.
//!
//! The kernel passes a HalServices pointer via RDI. This module initializes
//! a global allocator backed by the kernel's heap functions, then starts
//! the BMO Core desktop (windowing, UI, file system, BEF loader).

#![no_std]
#![no_main]

extern crate alloc;

use core::alloc::{GlobalAlloc, Layout};
use core::panic::PanicInfo;

// ── Global allocator ──────────────────────────────────────────────────

static mut HAL_PTR: *const bmo_hal_defs::HalServices = core::ptr::null();

struct KernelHeapAlloc;

unsafe impl GlobalAlloc for KernelHeapAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if !HAL_PTR.is_null() {
            let hal = &*HAL_PTR;
            (hal.heap_alloc)(layout.size(), layout.align())
        } else { core::ptr::null_mut() }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if !HAL_PTR.is_null() {
            let hal = &*HAL_PTR;
            (hal.heap_free)(ptr, layout.size(), layout.align())
        }
    }
}

#[global_allocator]
static ALLOCATOR: KernelHeapAlloc = KernelHeapAlloc;

// ── Framebuffer text (on-screen diagnostics, no serial needed) ────────

const FONT8: [[u8; 8]; 48] = [
    // space ! " # $ % & ' ( ) * + , - . /
    [0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00], // 32: space
    [0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00], // 33: !
    [0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00], // 34: "
    [0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00], // 35: #
    [0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00], // 36: $
    [0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00], // 37: %
    [0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00], // 38: &
    [0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00], // 39: '
    [0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00], // 40: (
    [0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00], // 41: )
    [0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00], // 42: *
    [0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00], // 43: +
    [0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00], // 44: ,
    [0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00], // 45: -
    [0x00,0x00,0x00,0x18,0x18,0x00,0x00,0x00], // 46: .
    [0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00], // 47: /
    // 0    1    2    3    4    5    6    7    8    9    :    ;    <    =    >    ?
    [0x3C,0x66,0x6E,0x76,0x66,0x66,0x3C,0x00], // 48: 0
    [0x18,0x38,0x18,0x18,0x18,0x18,0x7E,0x00], // 49: 1
    [0x3C,0x66,0x06,0x0C,0x18,0x30,0x7E,0x00], // 50: 2
    [0x3C,0x66,0x06,0x1C,0x06,0x66,0x3C,0x00], // 51: 3
    [0x0C,0x1C,0x2C,0x4C,0x7E,0x0C,0x0C,0x00], // 52: 4
    [0x7E,0x60,0x7C,0x06,0x06,0x66,0x3C,0x00], // 53: 5
    [0x3C,0x66,0x60,0x7C,0x66,0x66,0x3C,0x00], // 54: 6
    [0x7E,0x66,0x0C,0x18,0x18,0x18,0x18,0x00], // 55: 7
    [0x3C,0x66,0x66,0x3C,0x66,0x66,0x3C,0x00], // 56: 8
    [0x3C,0x66,0x66,0x3E,0x06,0x66,0x3C,0x00], // 57: 9
    [0x00,0x00,0x18,0x18,0x00,0x18,0x18,0x00], // 58: :
    [0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00], // 59-63: unused
    [0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00],
    [0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00],
    [0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00],
    [0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00],
    // @ A B C D E F G H I J K L M N O
    [0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00],
    [0x18,0x3C,0x66,0x66,0x7E,0x66,0x66,0x00], // 65: A
    [0x7C,0x66,0x66,0x7C,0x66,0x66,0x7C,0x00], // 66: B
    [0x3C,0x66,0x60,0x60,0x60,0x66,0x3C,0x00], // 67: C
    [0x78,0x6C,0x66,0x66,0x66,0x6C,0x78,0x00], // 68: D
    [0x7E,0x60,0x60,0x78,0x60,0x60,0x7E,0x00], // 69: E
    [0x7E,0x60,0x60,0x78,0x60,0x60,0x60,0x00], // 70: F
    [0x3C,0x66,0x60,0x6E,0x66,0x66,0x3C,0x00], // 71: G
    [0x66,0x66,0x66,0x7E,0x66,0x66,0x66,0x00], // 72: H
    [0x7E,0x18,0x18,0x18,0x18,0x18,0x7E,0x00], // 73: I
    [0x1E,0x0C,0x0C,0x0C,0x0C,0x6C,0x38,0x00], // 74: J
    [0x66,0x6C,0x78,0x70,0x78,0x6C,0x66,0x00], // 75: K
    [0x60,0x60,0x60,0x60,0x60,0x60,0x7E,0x00], // 76: L
    [0x66,0x7E,0x7E,0x66,0x66,0x66,0x66,0x00], // 77: M
    [0x66,0x76,0x7E,0x7E,0x6E,0x66,0x66,0x00], // 78: N
    [0x3C,0x66,0x66,0x66,0x66,0x66,0x3C,0x00], // 79: O
];

const WHITE: u32 = 0xFFFFFFFF;
const GREEN: u32 = 0xFF00FF00;
const RED:   u32 = 0xFFFF0000;
const BLACK: u32 = 0xFF000000;
const FONT_W: usize = 8;
const FONT_H: usize = 8;
const CHAR_H: usize = 10;

fn fb_put_pixel(hal: &bmo_hal_defs::HalServices, x: u32, y: u32, color: u32) {
    (hal.framebuffer_put_pixel)(x, y, color);
}

fn fb_draw_char(hal: &bmo_hal_defs::HalServices, x: u32, y: u32, c: u8, color: u32) {
    let idx = c.wrapping_sub(32) as usize;
    if idx >= FONT8.len() { return; }
    let glyph = &FONT8[idx];
    for row in 0..FONT_H {
        let bits = glyph[row];
        for col in 0..FONT_W {
            if bits & (1 << (7 - col)) != 0 {
                fb_put_pixel(hal, x + col as u32, y + row as u32, color);
            } else {
                fb_put_pixel(hal, x + col as u32, y + row as u32, BLACK);
            }
        }
    }
}

fn fb_draw_str(hal: &bmo_hal_defs::HalServices, x: u32, y: u32, s: &str, color: u32) {
    let mut cx = x;
    for b in s.bytes() {
        fb_draw_char(hal, cx, y, b, color);
        cx += FONT_W as u32;
    }
}

fn fb_fill_rect(hal: &bmo_hal_defs::HalServices, x: u32, y: u32, w: u32, h: u32, color: u32) {
    for dy in 0..h { for dx in 0..w { fb_put_pixel(hal, x + dx, y + dy, color); } }
}

fn show_diagnostics(hal: &bmo_hal_defs::HalServices, xhci_mmio: u64, xhci_ok: bool, hid_ok: bool, ps2_active: bool) {
    let x = 16u32;
    let mut y = 16u32;
    let w = hal.fb_width;
    let _h = hal.fb_height;

    // Background bar
    fb_fill_rect(hal, 0, 0, w, 80, 0xFF222222);

    // Title
    fb_draw_str(hal, x, y, "BMO INPUT DIAGNOSTICS", WHITE);
    y += CHAR_H as u32;
    y += 4;

    // XHCI line
    let xhci_color = if xhci_ok { GREEN } else { RED };
    if xhci_mmio == 0 {
        fb_draw_str(hal, x, y, "XHCI:  NONE (no controller found)", xhci_color);
    } else {
        fb_draw_str(hal, x, y, "XHCI:  OK mmio=", xhci_color);
        // Simple hex display of the address
        let mut buf = [0u8; 10];
        let mut v = xhci_mmio;
        for i in (0..10).rev() {
            let d = (v & 0xF) as u8;
            buf[i] = if d < 10 { b'0' + d } else { b'A' + (d - 10) };
            v >>= 4;
        }
        fb_draw_str(hal, x + 16 * FONT_W as u32, y, core::str::from_utf8(&buf).unwrap_or("0000000000"), xhci_color);
    }
    y += CHAR_H as u32;

    // HID line
    fb_draw_str(hal, x, y, "HID:   ", if hid_ok { GREEN } else { RED });
    fb_draw_str(hal, x + 7 * FONT_W as u32, y, if hid_ok { "OK" } else { "FAIL" }, if hid_ok { GREEN } else { RED });
    y += CHAR_H as u32;

    // PS/2 line
    fb_draw_str(hal, x, y, "PS/2:  ", WHITE);
    fb_draw_str(hal, x + 7 * FONT_W as u32, y, if ps2_active { "active" } else { "fallback" }, WHITE);

    // Wait 3 seconds
    for _ in 0..300 {
        (hal.busy_wait_ms)(10);
    }
}

// ── XhciHal implementation (wraps HalServices) ────────────────────────

struct ModuleXhciHal {
    hal: HalPtr,
}

#[derive(Copy, Clone)]
struct HalPtr(*const bmo_hal_defs::HalServices);

impl ModuleXhciHal {
    fn hal(&self) -> &bmo_hal_defs::HalServices {
        unsafe { &*self.hal.0 }
    }
}

impl bmo_xhci::XhciHal for ModuleXhciHal {
    fn alloc_dma_pages(&self, count: usize) -> Option<u64> {
        let phys = (self.hal().alloc_pages_contiguous)(count);
        if phys == 0 { None } else { Some(phys) }
    }
    fn phys_to_virt(&self, phys: u64) -> *mut u8 {
        if phys >= self.hal().HIGH_MEM_BASE {
            phys as *mut u8
        } else {
            // Below 4GB, identity-mapped by UEFI
            phys as *mut u8
        }
    }
    fn log(&self, msg: &str) { (self.hal().serial_write)(msg); }
    fn log_u64(&self, msg: &str, val: u64) { (self.hal().serial_write_u64)(val, 16); }
}

// ── Entry point ──────────────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn _module_start(hal_ptr: *const bmo_hal_defs::HalServices) -> ! {
    unsafe { HAL_PTR = hal_ptr; }

    if let Some(hal) = unsafe { HAL_PTR.as_ref() } {
        (hal.serial_write)("[mod_bmo_core] module loaded\n");

        // Init XHCI + USB HID for keyboard/mouse
        let (xhci_mmio, xhci_ok, hid_ok) = init_xhci(hal);

        // Show diagnostics ON SCREEN (no keyboard needed)
        show_diagnostics(hal, xhci_mmio, xhci_ok, hid_ok, xhci_mmio == 0);

        // Wire USB HID poll into bmo_core's input layer
        unsafe {
            bmo_core::desktop::input::USB_HID_POLL = Some(poll_usb_hid);
        }

        // Initialize bmo_core with the kernel's HalServices
        unsafe { bmo_core::hal::init(*hal_ptr); }
        (hal.serial_write)("[mod_bmo_core] bmo_core HAL init complete\n");
        (hal.write_boot_stage)("coord_init");

        bmo_core::coord::init();

        (hal.write_crash_marker)(8);
        (hal.write_boot_stage)("welcome_dispatch");
        bmo_core::desktop::commands::enter_desktop();
    }

    loop { unsafe { core::arch::asm!("hlt"); } }
}

fn init_xhci(hal: &bmo_hal_defs::HalServices) -> (u64, bool, bool) {
    let xhci_mmio = unsafe {
        if (hal.boot_info).is_null() { 0 }
        else { (*(hal.boot_info)).xhci_mmio }
    };

    if xhci_mmio == 0 {
        (hal.serial_write)("[mod_bmo_core] no XHCI controller (xhci_mmio=0), input via PS/2 only\n");
        return (0, false, false);
    }

    (hal.serial_write)("[mod_bmo_core] XHCI mmio=0x");
    (hal.serial_write_u64)(xhci_mmio, 16);
    (hal.serial_write)("\n");

    let backend = alloc::boxed::Box::new(ModuleXhciHal { hal: HalPtr(hal as *const _) });
    let static_backend: &'static ModuleXhciHal = alloc::boxed::Box::leak(backend);

    bmo_xhci::init_hal(static_backend as &'static dyn bmo_xhci::XhciHal);
    bmo_xhci::set_mmio(xhci_mmio);

    (hal.serial_write)("[mod_bmo_core] initializing XHCI controller...\n");
    let xhci_ok = unsafe { bmo_xhci::init(xhci_mmio) };
    if !xhci_ok {
        (hal.serial_write)("[mod_bmo_core] XHCI controller init FAILED\n");
        return (xhci_mmio, false, false);
    }
    (hal.serial_write)("[mod_bmo_core] XHCI controller initialized OK\n");

    let mut uhid = bmo_uhid::UsbHidHal::new();
    let hid_ok = {
        use bmo_input::hal::InputHal;
        uhid.init()
    };
    if hid_ok {
        (hal.serial_write)("[mod_bmo_core] USB HID ready\n");
    } else {
        (hal.serial_write)("[mod_bmo_core] USB HID init failed\n");
        return (xhci_mmio, true, false);
    }

    unsafe { UHID_PTR = Some(uhid); }
    (xhci_mmio, true, true)
}

static mut UHID_PTR: Option<bmo_uhid::UsbHidHal> = None;

/// Public API for input layer: poll USB HID if XHCI is available.
/// Returns true if any event was available.
pub fn poll_usb_hid() -> bool {
    unsafe {
        if let Some(ref mut uhid) = UHID_PTR {
            let mut buf = [bmo_input::event::InputEvent::empty(); 32];
            use bmo_input::hal::InputHal;
            let n = uhid.poll(&mut buf);
            n > 0
        } else { false }
    }
}

// ── Panic handler ────────────────────────────────────────────────────

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    if let Some(hal) = unsafe { HAL_PTR.as_ref() } {
        (hal.serial_write)("[mod_bmo_core] PANIC: ");
        if let Some(s) = info.message().as_str() {
            (hal.serial_write)(s);
        }
        (hal.serial_write)("\n");
    }
    loop { unsafe { core::arch::asm!("hlt"); } }
}
