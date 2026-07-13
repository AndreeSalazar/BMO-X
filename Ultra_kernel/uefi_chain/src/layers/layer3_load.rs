//! Layer 3 — `uefi_loader`
//!
//! Responsibilities:
//! 1. Locate `EFI_SIMPLE_FILE_SYSTEM_PROTOCOL`.
//! 2. Open volume (root of ESP).
//! 3. Read `\EFI\BOOT\stage1.bin`, `stage2.bin`, `stage3.bin`, `kernel.bin`.
//! 4. Copy each to its fixed physical address (1MB, 2MB, 3MB, 4MB).
//! 5. Fill `ctx.stage_base[]`, `ctx.stage_size[]`, `ctx.stage_entry[]`.
//! 6. Jump to layer 4 (`uefi_exit`).

#![allow(dead_code)]

use core::arch::asm;
use boot_context::BootContext;

type EfiHandle = *mut core::ffi::c_void;
type EfiStatus = u64;

const EFI_SUCCESS: u64 = 0;

const STAGE1_ADDR: u64 = 0x100000;
const STAGE2_ADDR: u64 = 0x200000;
const STAGE3_ADDR: u64 = 0x300000;
const KERNEL_ADDR: u64 = 0x400000;
const MAX_FILE_SIZE: usize = 256 * 1024;

extern "C" {
    fn l4_entry(ctx: *mut BootContext, ih: EfiHandle, st: *mut core::ffi::c_void) -> !;
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

static mut FILE_SYSTEM_GUID: EfiGuid = EfiGuid {
    data1: 0x964e5b22, data2: 0x6409, data3: 0x47ef,
    data4: [0x97, 0xa2, 0xff, 0x06, 0xff, 0x38, 0xb0, 0xdf],
};

struct LoadEntry { name: &'static str, addr: u64 }

#[no_mangle]
pub extern "C" fn l3_entry(
    ctx_ptr: *mut BootContext,
    image_handle: EfiHandle,
    system_table: *mut EfiSystemTable,
) -> ! {
    crate::serial::puts("\n[L3 uefi_loader]\n");

    if ctx_ptr.is_null() || system_table.is_null() {
        crate::serial::puts("[L3] null handoff — halting\n");
        halt();
    }

    let ctx = unsafe { &mut *ctx_ptr };
    let st = unsafe { &*system_table };
    let bs = st.boot_services;

    if bs.is_null() {
        crate::serial::puts("[L3] BootServices null — halting\n");
        halt();
    }

    let mut fs_handle: EfiHandle = core::ptr::null_mut();
    let r = unsafe { locate_protocol(bs, &mut FILE_SYSTEM_GUID, &mut fs_handle) };
    if r != EFI_SUCCESS || fs_handle.is_null() {
        crate::serial::puts("[L3] Simple FS not found — halting\n");
        halt();
    }

    let sfsp = fs_handle as *const *mut core::ffi::c_void;
    let open_vol: extern "efiapi" fn(EfiHandle, &mut *mut core::ffi::c_void) -> EfiStatus =
        unsafe { core::mem::transmute(*sfsp.add(1)) };
    let mut root: *mut core::ffi::c_void = core::ptr::null_mut();
    if unsafe { open_vol(fs_handle, &mut root) } != EFI_SUCCESS || root.is_null() {
        crate::serial::puts("[L3] OpenVolume failed — halting\n");
        halt();
    }

    let file_base = root as *const *mut core::ffi::c_void;
    let open_fn: extern "efiapi" fn(
        *mut core::ffi::c_void,
        &mut *mut core::ffi::c_void,
        *const u16,
        u64, u64, *mut core::ffi::c_void,
    ) -> EfiStatus = unsafe { core::mem::transmute(*file_base.add(1)) };
    let read_fn: extern "efiapi" fn(*mut core::ffi::c_void, &mut usize, *mut u8) -> EfiStatus =
        unsafe { core::mem::transmute(*file_base.add(4)) };

    let stages = [
        LoadEntry { name: "stage1.bin", addr: STAGE1_ADDR },
        LoadEntry { name: "stage2.bin", addr: STAGE2_ADDR },
        LoadEntry { name: "stage3.bin", addr: STAGE3_ADDR },
        LoadEntry { name: "kernel.bin", addr: KERNEL_ADDR },
    ];

    let mut file_buf = [0u8; MAX_FILE_SIZE];
    let mut ok = true;

    for (i, s) in stages.iter().enumerate() {
        crate::serial::puts("[L3] load ");
        crate::serial::puts(s.name);
        crate::serial::puts(" -> 0x");
        crate::serial::hex(s.addr);
        crate::serial::puts(" ... ");

        let mut path = [0u16; 260];
        path[0] = b'\\' as u16;
        let prefix = b"EFI\\BOOT\\";
        let mut idx = 1;
        for &c in prefix { path[idx] = c as u16; idx += 1; }
        for &c in s.name.as_bytes() { path[idx] = c as u16; idx += 1; }
        path[idx] = 0;

        let mut file: *mut core::ffi::c_void = core::ptr::null_mut();
        if unsafe { open_fn(root, &mut file, path.as_ptr(), 0, 0, core::ptr::null_mut()) } != EFI_SUCCESS || file.is_null() {
            crate::serial::puts("OPEN-FAIL\n");
            ok = false;
            continue;
        }

        let mut size = file_buf.len();
        if unsafe { read_fn(file, &mut size, file_buf.as_mut_ptr()) } != EFI_SUCCESS || size == 0 {
            crate::serial::puts("READ-FAIL\n");
            ok = false;
            continue;
        }

        unsafe { copy_to_phys(s.addr, file_buf.as_ptr(), size.min(MAX_FILE_SIZE)); }
        ctx.stage_base[i] = s.addr;
        ctx.stage_size[i] = size as u64;
        ctx.stage_entry[i] = s.addr;

        crate::serial::dec(size);
        crate::serial::puts(" bytes\n");
    }

    if !ok {
        crate::serial::puts("[L3] missing stage files — halting\n");
        halt();
    }

    crate::serial::puts("[L3] jump -> layer4_exit\n");

    unsafe {
        asm!(
            "jmp {l4}",
            l4 = in(reg) l4_entry as *const () as u64,
            in("rdi") ctx_ptr,
            in("rsi") image_handle,
            in("rdx") system_table,
            options(noreturn)
        );
    }
}

unsafe fn copy_to_phys(dst: u64, src: *const u8, len: usize) {
    let dst_ptr = dst as *mut u8;
    for i in 0..len {
        dst_ptr.add(i).write(src.add(i).read());
    }
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
