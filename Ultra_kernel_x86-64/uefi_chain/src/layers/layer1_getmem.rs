//! Layer 1 ??? `uefi_efi_getmem`
//!
//! Responsibilities:
//! 1. Resolve `BootServices` from the SystemTable.
//! 2. Call `GetMemoryMap` (twice: once for size, once for content).
//! 3. Filter descriptors of type `EfiConventionalMemory`.
//! 4. Fill `ctx.memory_map` and `ctx.memory_map_count`.
//! 5. Jump to layer 2 (`uefi_efi_getgop`).

#![allow(dead_code)]

use core::arch::asm;
use boot_context::{BootContext, MemoryEntry, MAX_MEMORY_ENTRIES};

const EFI_SUCCESS: u64 = 0;
const EFI_BUFFER_TOO_SMALL: u64 = 5;
const EFI_CONVENTIONAL_MEMORY: u32 = 7;

type EfiHandle = *mut core::ffi::c_void;

extern "C" {
    fn l2_entry(ctx: *mut BootContext, ih: EfiHandle, st: *mut core::ffi::c_void) -> !;
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
struct EfiMemoryDescriptor {
    mem_type: u32,
    _pad: u32,
    phys_start: u64,
    virt_start: u64,
    num_pages: u64,
    attrib: u64,
}

#[no_mangle]
pub extern "C" fn l1_entry(
    ctx_ptr: *mut BootContext,
    image_handle: EfiHandle,
    system_table: *mut EfiSystemTable,
) -> ! {
    crate::serial::puts("\n[L1 uefi_efi_getmem]\n");

    if ctx_ptr.is_null() || system_table.is_null() {
        crate::serial::puts("[L1] null handoff ??? halting\n");
        halt();
    }

    let ctx = unsafe { &mut *ctx_ptr };
    let st = unsafe { &*system_table };
    let bs = st.boot_services;

    if bs.is_null() {
        crate::serial::puts("[L1] BootServices null ??? halting\n");
        halt();
    }

    const MEM_BUF_SIZE: usize = 32768;
    let mut mem_buf = [0u8; MEM_BUF_SIZE];
    let mut map_size = MEM_BUF_SIZE;
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
    if r != EFI_SUCCESS && r != EFI_BUFFER_TOO_SMALL {
        crate::serial::puts("[L1] GetMemoryMap #1 failed: 0x");
        crate::serial::hex(r);
        crate::serial::puts("\n");
        halt();
    }

    if desc_size == 0 || map_size == 0 {
        crate::serial::puts("[L1] empty memory map ??? halting\n");
        halt();
    }

    let num = map_size / desc_size;
    let mut entries = [MemoryEntry { base: 0, size: 0, kind: 0 }; MAX_MEMORY_ENTRIES];
    let mut ec: usize = 0;

    for i in 0..num.min(MAX_MEMORY_ENTRIES) {
        let desc = unsafe {
            &*(mem_buf.as_ptr().add(i * desc_size) as *const EfiMemoryDescriptor)
        };
        if desc.mem_type == EFI_CONVENTIONAL_MEMORY && desc.num_pages > 0 && ec < MAX_MEMORY_ENTRIES {
            entries[ec] = MemoryEntry {
                base: desc.phys_start,
                size: desc.num_pages * 4096,
                kind: 1,
            };
            ec += 1;
        }
    }

    ctx.set_memory_map(&entries[..ec]);
    ctx.memory_map_count = ec as u32;

    crate::serial::puts("[L1] memory map: ");
    crate::serial::dec(ec);
    crate::serial::puts(" conventional entries, key=0x");
    crate::serial::hex(map_key as u64);
    crate::serial::puts("\n");

    crate::serial::puts("[L1] jump -> layer2_getgop\n");

    unsafe { l2_entry(ctx_ptr, image_handle, system_table.cast()) }
}

unsafe fn get_memory_map(
    bs: *mut EfiBootServices,
    map_size: &mut usize,
    buf: *mut u8,
    map_key: &mut usize,
    desc_size: &mut usize,
    desc_ver: &mut u32,
) -> u64 {
    let base = &(*bs).hdr as *const EfiTableHeader as *const *mut core::ffi::c_void;
    let fnptr: extern "efiapi" fn(
        *mut usize, *mut u8, *mut usize, *mut usize, *mut u32,
    ) -> u64 = core::mem::transmute(*base.add(3 + 4));
    fnptr(map_size, buf, map_key, desc_size, desc_ver)
}

#[inline(never)]
fn halt() -> ! { loop { unsafe { asm!("hlt"); } } }
