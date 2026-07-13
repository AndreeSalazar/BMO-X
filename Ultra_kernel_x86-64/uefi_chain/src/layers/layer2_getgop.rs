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
    let info = unsafe { &*(mode.info as *const [u32; 8]) };
    let w = info[0];
    let h = info[1];
    let fmt = info[2];
    // EFI_GRAPHICS_OUTPUT_MODE_INFORMATION.PixelsPerScanLine is the eighth
    // u32. FrameBufferSize may include firmware padding and is not a stride.
    let stride = info[7];

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
