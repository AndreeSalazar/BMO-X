//! Layer 4 — `uefi_exit`
//!
//! The point of no return.
//!
//! Responsibilities:
//! 1. Re-fetch a fresh `GetMemoryMap` (UEFI rule: key must be from the
//!    most recent call before `ExitBootServices`).
//! 2. Call `ExitBootServices(IH, fresh_key)` — after this, UEFI
//!    services are gone forever.
//! 3. **JUMP** to `ctx.stage_entry[0]` (stage1_arch). This is the
//!    boundary between the UEFI world and the Ring 0 world.

#![allow(dead_code)]

use core::arch::asm;
use boot_context::BootContext;

type EfiHandle = *mut core::ffi::c_void;
type EfiStatus = u64;

const EFI_SUCCESS: u64 = 0;

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

#[no_mangle]
pub extern "C" fn l4_entry(
    ctx_ptr: *mut BootContext,
    image_handle: EfiHandle,
    system_table: *mut EfiSystemTable,
) -> ! {
    crate::serial::puts("\n[L4 uefi_exit]\n");

    if ctx_ptr.is_null() || system_table.is_null() {
        crate::serial::puts("[L4] null handoff — halting\n");
        halt();
    }

    let ctx = unsafe { &*ctx_ptr };
    let st = unsafe { &*system_table };
    let bs = st.boot_services;

    if bs.is_null() {
        crate::serial::puts("[L4] BootServices null — halting\n");
        halt();
    }

    let mut mem_buf = [0u8; 32768];
    let mut map_size = mem_buf.len();
    let mut map_key: usize = 0;
    let mut desc_size: usize = 0;
    let mut desc_ver: u32 = 0;

    let r = unsafe {
        get_memory_map(
            bs,
            &mut map_size,
            mem_buf.as_mut_ptr(),
            &mut map_key,
            &mut desc_size,
            &mut desc_ver,
        )
    };
    if r != EFI_SUCCESS {
        crate::serial::puts("[L4] GetMemoryMap(refresh) failed: 0x");
        crate::serial::hex(r);
        crate::serial::puts("\n");
        halt();
    }

    let r = unsafe { exit_boot_services(bs, image_handle, map_key) };
    crate::serial::puts("[L4] ExitBootServices status=0x");
    crate::serial::hex(r);
    crate::serial::puts("\n");
    if r != EFI_SUCCESS {
        crate::serial::puts("[L4] ExitBootServices failed — halting\n");
        halt();
    }

    let entry = ctx.stage_entry[0];
    if entry == 0 {
        crate::serial::puts("[L4] stage_entry[0] == 0 — halting\n");
        halt();
    }

    crate::serial::puts("[L4] ===> JUMP Ring 0 -> 0x");
    crate::serial::hex(entry);
    crate::serial::puts("\n");

    unsafe {
        asm!("sfence", options(nostack, preserves_flags));
        asm!(
            "mov rdi, {ctx}",
            "xor rbp, rbp",
            "jmp {entry}",
            ctx = in(reg) ctx_ptr,
            entry = in(reg) entry,
            options(noreturn)
        );
    }
}

unsafe fn get_memory_map(
    bs: *mut EfiBootServices,
    map_size: &mut usize,
    buf: *mut u8,
    map_key: &mut usize,
    desc_size: &mut usize,
    desc_ver: &mut u32,
) -> EfiStatus {
    let base = &(*bs).hdr as *const EfiTableHeader as *const *mut core::ffi::c_void;
    let fnptr: extern "efiapi" fn(
        *mut usize, *mut u8, *mut usize, *mut usize, *mut u32,
    ) -> EfiStatus = core::mem::transmute(*base.add(3 + 4));
    fnptr(map_size, buf, map_key, desc_size, desc_ver)
}

unsafe fn exit_boot_services(bs: *mut EfiBootServices, handle: EfiHandle, key: usize) -> EfiStatus {
    let base = &(*bs).hdr as *const EfiTableHeader as *const *mut core::ffi::c_void;
    let fnptr: extern "efiapi" fn(EfiHandle, usize) -> EfiStatus =
        core::mem::transmute(*base.add(3 + 26));
    fnptr(handle, key)
}

#[inline(never)]
fn halt() -> ! { loop { unsafe { asm!("hlt"); } } }
