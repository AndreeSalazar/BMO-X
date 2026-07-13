//! Layer 2 ??? `uefi_efi_getgop`
//!
//! Responsibilities:
//! 1. Locate `EFI_GRAPHICS_OUTPUT_PROTOCOL` via `LocateProtocol`.
//! 2. Read current mode's framebuffer base/size/width/height/format.
//! 3. Fill `ctx.fb_addr`, `ctx.fb_width`, `ctx.fb_height`,
//!    `ctx.fb_stride`, `ctx.fb_pixel_format`.
//! 4. Jump to layer 3 (`uefi_loader`).
//!
//! If GOP is not available (headless firmware), this layer still
//! succeeds ??? fields are left zero and the chain continues.

#![allow(dead_code)]

use core::arch::asm;
use boot_context::BootContext;

type EfiHandle = *mut core::ffi::c_void;
type EfiStatus = u64;

const EFI_SUCCESS: u64 = 0;

extern "C" {
    fn l3_entry(ctx: *mut BootContext, ih: EfiHandle, st: *mut core::ffi::c_void) -> !;
}

#[repr(C)]
struct EfiTableHeader {
    signature: u64,
    revision: u32,
    header_size: u32,
    crc32: u32,
    _reserved: u32,
}

#[repr(C)]
struct EfiBootServices {
    hdr: EfiTableHeader,
    _pad: [u8; 44 * 8],
}

#[repr(C)]
struct EfiSystemTable {
    hdr: EfiTableHeader,
    _firmware: *mut core::ffi::c_void,
    _cin_handle: EfiHandle,
    _con_in: *mut core::ffi::c_void,
    _cout_handle: EfiHandle,
    _con_out: *mut core::ffi::c_void,
    _cerr_handle: EfiHandle,
    _con_err: *mut core::ffi::c_void,
    _runtime: *mut core::ffi::c_void,
    boot_services: *mut EfiBootServices,
    _num_tables: usize,
    _config_tables: *mut core::ffi::c_void,
}

#[repr(C)]
struct EfiGuid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

#[repr(C)]
struct EfiGraphicsOutputProtocolMode {
    max_mode: u32,
    mode: u32,
    info: *mut u8,
    size_of_info: usize,
    frame_buffer_base: u64,
    frame_buffer_size: usize,
}

#[repr(C)]
struct EfiGraphicsOutputProtocol {
    query_mode: extern "efiapi" fn(*mut Self, u32, &mut usize, &mut *mut u8) -> EfiStatus,
    set_mode: extern "efiapi" fn(*mut Self, u32) -> EfiStatus,
    blt: *mut core::ffi::c_void,
    mode: *mut EfiGraphicsOutputProtocolMode,
}

static mut GOP_GUID: EfiGuid = EfiGuid {
    data1: 0x9042a9de, data2: 0x23dc, data3: 0x4a38,
    data4: [0x96, 0xfb, 0x72, 0xde, 0x52, 0xfe, 0xc4, 0x49],
};

#[no_mangle]
pub extern "C" fn l2_entry(
    ctx_ptr: *mut BootContext,
    image_handle: EfiHandle,
    system_table: *mut EfiSystemTable,
) -> ! {
    crate::serial::puts("\n[L2 uefi_efi_getgop]\n");

    if ctx_ptr.is_null() || system_table.is_null() {
        crate::serial::puts("[L2] null handoff ??? halting\n");
        halt();
    }

    let ctx = unsafe { &mut *ctx_ptr };
    let st = unsafe { &*system_table };
    let bs = st.boot_services;

    if bs.is_null() {
        crate::serial::puts("[L2] BootServices null ??? headless, fb=0\n");
        jump_next(ctx_ptr, image_handle, system_table);
    }

    let mut gop_handle: EfiHandle = core::ptr::null_mut();
    let r = unsafe { locate_protocol(bs, &mut GOP_GUID, &mut gop_handle) };

    if r != EFI_SUCCESS || gop_handle.is_null() {
        crate::serial::puts("[L2] GOP not found ??? headless, fb=0\n");
        jump_next(ctx_ptr, image_handle, system_table);
    }

    let gop = unsafe { &*(gop_handle as *const EfiGraphicsOutputProtocol) };
    let mode = unsafe { &*gop.mode };
    // EFI_GRAPHICS_OUTPUT_MODE_INFORMATION layout (UEFI spec 2.10 §11.4):
    //   [0] UINT32 Version
    //   [1] UINT32 HorizontalResolution
    //   [2] UINT32 VerticalResolution
    //   [3] UINT32 PixelFormat
    //   [4..7] EFI_PIXEL_BITMASK {Red,Green,Blue,Reserved} = 4 × UINT32
    //   [8] UINT32 PixelsPerScanLine
    let info = unsafe { &*(mode.info as *const [u32; 9]) };
    
    // DEBUG: log all info fields to serial
    crate::serial::puts("[L2] GOP info[0..8]: ");
    for i in 0..9 {
        crate::serial::hex(info[i] as u64);
        crate::serial::puts(" ");
    }
    crate::serial::puts("\n");
    
    let w = info[1];      // HorizontalResolution
    let h = info[2];      // VerticalResolution
    let fmt = info[3];    // PixelFormat (0=BGR, 1=RGB, 2=blt-only)
    let stride = info[8]; // PixelsPerScanLine
    
    crate::serial::puts("[L2] parsed: w=");
    crate::serial::dec(w as usize);
    crate::serial::puts(" h=");
    crate::serial::dec(h as usize);
    crate::serial::puts(" fmt=");
    crate::serial::dec(fmt as usize);
    crate::serial::puts(" stride=");
    crate::serial::dec(stride as usize);
    crate::serial::puts(" fb_base=0x");
    crate::serial::hex(mode.frame_buffer_base);
    crate::serial::puts("\n");

    // PixelBltOnly (3) has no linear framebuffer. The kernel currently
    // supports only the standard 32-bit RGB/BGR GOP modes.
    if w == 0 || h == 0 || stride < w || fmt > 1 || mode.frame_buffer_base == 0 {
        crate::serial::puts("[L2] unsupported GOP mode ??? headless, fb=0\n");
        jump_next(ctx_ptr, image_handle, system_table);
    }

    ctx.fb_addr = mode.frame_buffer_base;
    ctx.fb_width = w;
    ctx.fb_height = h;
    ctx.fb_stride = stride;
    ctx.fb_pixel_format = fmt;

    crate::serial::puts("[L2] GOP fb=0x");
    crate::serial::hex(ctx.fb_addr);
    crate::serial::puts(" ");
    crate::serial::dec(w as usize);
    crate::serial::puts("x");
    crate::serial::dec(h as usize);
    crate::serial::puts("\n");

    // Log GOP info to NVRAM for debugging
    nvram_log::log("GOP detected");
    let mut gop_info = [0u8; 128];
    let mut pos = 0;
    // Write "fb=" + hex address
    gop_info[pos..pos+3].copy_from_slice(b"fb=");
    pos += 3;
    for i in (0..64).step_by(4) {
        let nibble = ((ctx.fb_addr >> (60 - i)) & 0xF) as u8;
        gop_info[pos] = if nibble < 10 { b'0' + nibble } else { b'a' + nibble - 10 };
        pos += 1;
    }
    gop_info[pos] = b' ';
    pos += 1;
    // Write width
    let w_str = format_dec(w as u64);
    let w_len = w_str.iter().position(|&b| b == 0).unwrap_or(20);
    gop_info[pos..pos+w_len].copy_from_slice(&w_str[..w_len]);
    pos += w_len;
    gop_info[pos] = b'x';
    pos += 1;
    // Write height
    let h_str = format_dec(h as u64);
    let h_len = h_str.iter().position(|&b| b == 0).unwrap_or(20);
    gop_info[pos..pos+h_len].copy_from_slice(&h_str[..h_len]);
    pos += h_len;
    gop_info[pos] = b'\n';
    pos += 1;
    nvram_log::log(core::str::from_utf8(&gop_info[..pos]).unwrap_or("GOP info"));

    // Pre-ExitBootServices splash: draw a colored rectangle to verify GOP works
    // This runs while UEFI still owns the display, so we can see if framebuffer is accessible
    if ctx.fb_addr != 0 && w > 0 && h > 0 {
        crate::serial::puts("[L2] drawing pre-ExitBootServices splash...\n");
        nvram_log::log("drawing pre-ExitBootServices splash");

        // Fill screen with dark blue
        let fb = ctx.fb_addr as *mut u32;
        let total = (stride as usize) * (h as usize);
        for i in 0..total {
            unsafe { fb.add(i).write_volatile(0xFF0A_0F1Du32); }
        }
        unsafe { core::arch::asm!("mfence", options(nostack, preserves_flags)); }

        // Draw cyan horizontal bar near top
        let bar_h = 4u32;
        let bar_y = h / 8;
        let bar_base = (bar_y as usize) * (stride as usize);
        for dy in 0..bar_h {
            for dx in 0..stride {
                unsafe { fb.add(bar_base + (dy as usize) * (stride as usize) + (dx as usize))
                    .write_volatile(0xFF00_E5FFu32); }
            }
        }
        unsafe { core::arch::asm!("mfence", options(nostack, preserves_flags)); }

        // Draw centered cyan rectangle (logo placeholder)
        let cx = stride / 2;
        let cy = h / 2;
        let bw = 120u32;
        let bh = 40u32;
        let bx = cx - bw / 2;
        let by = cy - bh / 2;
        for dy in 0..bh {
            for dx in 0..bw {
                unsafe {
                    fb.add(((by + dy) as usize) * (stride as usize) + ((bx + dx) as usize))
                        .write_volatile(0xFF00_E5FFu32);
                }
            }
        }
        unsafe { core::arch::asm!("mfence", options(nostack, preserves_flags)); }

        crate::serial::puts("[L2] splash drawn\n");
        nvram_log::log("splash drawn successfully");
    }

    jump_next(ctx_ptr, image_handle, system_table);
}

/// Helper to format a u64 as decimal string (no_std compatible)
fn format_dec(mut val: u64) -> [u8; 20] {
    let mut buf = [0u8; 20];
    if val == 0 {
        buf[0] = b'0';
        return buf;
    }
    let mut pos = 20;
    while val > 0 {
        pos -= 1;
        buf[pos] = b'0' + (val % 10) as u8;
        val /= 10;
    }
    // Shift to start
    let mut result = [0u8; 20];
    let len = 20 - pos;
    result[..len].copy_from_slice(&buf[pos..]);
    result
}

fn jump_next(
    ctx_ptr: *mut BootContext,
    image_handle: EfiHandle,
    system_table: *mut EfiSystemTable,
) -> ! {
    crate::serial::puts("[L2] jump -> layer3_load\n");
    unsafe { l3_entry(ctx_ptr, image_handle, system_table.cast()) }
}

unsafe fn locate_protocol(
    bs: *mut EfiBootServices,
    guid: *mut EfiGuid,
    handle: &mut EfiHandle,
) -> EfiStatus {
    let base = &(*bs).hdr as *const EfiTableHeader as *const *mut core::ffi::c_void;
    let fnptr: extern "efiapi" fn(*mut EfiGuid, *mut core::ffi::c_void, &mut EfiHandle) -> EfiStatus =
        core::mem::transmute(*base.add(3 + 37));
    fnptr(guid, core::ptr::null_mut(), handle)
}

#[inline(never)]
fn halt() -> ! { loop { unsafe { asm!("hlt"); } } }
