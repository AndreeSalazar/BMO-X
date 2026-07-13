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
    let w = info[1];      // HorizontalResolution
    let h = info[2];      // VerticalResolution
    let fmt = info[3];    // PixelFormat (0=BGR, 1=RGB, 2=blt-only)
    let stride = info[8]; // PixelsPerScanLine

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

    // ── Early splash BEFORE ExitBootServices ────────────────────
    // On NVIDIA GPUs (RTX 3060), the GOP display controller stops
    // scanning from the framebuffer after ExitBootServices. To work
    // around this, we draw a simple boot splash NOW, while UEFI
    // boot services are still active and the GPU IS scanning.
    // After ExitBootServices, the last rendered frame stays visible
    // even if the kernel can't update the display.
    if mode.frame_buffer_base != 0 {
        crate::serial::puts("[L2] drawing pre-ExitBootServices splash...\n");
        let fb = mode.frame_buffer_base as *mut u32;
        let stride_px = stride as usize;
        let total = stride_px * (h as usize);

        // Fill the entire screen with a dark blue background
        // using a simple loop (rep stosd isn't available in asm
        // on all nightly versions, so we use a tight loop).
        for i in 0..total {
            unsafe { fb.add(i).write_volatile(0xFF0A_0F1Du32); }
        }
        // Fence so the writes reach VRAM before we move on.
        unsafe { core::arch::asm!("mfence", options(nostack, preserves_flags)); }

        // Draw a white horizontal bar near the top
        let bar_h = 4u32;
        let bar_y = (h / 8);
        let bar_base = (bar_y as usize) * stride_px;
        for dy in 0..bar_h {
            for dx in 0..(stride as u32) {
                unsafe { fb.add(bar_base + (dy as usize) * stride_px + (dx as usize))
                    .write_volatile(0xFF00_E5FFu32); } // cyan
            }
        }
        unsafe { core::arch::asm!("mfence", options(nostack, preserves_flags)); }

        // Draw a centered text approximation (simple block letters)
        // We don't have a font renderer in UEFI asm, so just draw
        // three colored rectangles as a logo placeholder.
        let cx = stride / 2;
        let cy = h / 2;
        // Center block: cyan rectangle
        let bw = 120u32; let bh = 40u32;
        let bx = cx - bw / 2; let by = cy - bh / 2;
        for dy in 0..bh {
            for dx in 0..bw {
                unsafe {
                    fb.add(((by + dy) as usize) * stride_px + ((bx + dx) as usize))
                        .write_volatile(0xFF00_E5FFu32);
                }
            }
        }
        unsafe { core::arch::asm!("mfence", options(nostack, preserves_flags)); }
        crate::serial::puts("[L2] splash drawn\n");
    }

    jump_next(ctx_ptr, image_handle, system_table);
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
