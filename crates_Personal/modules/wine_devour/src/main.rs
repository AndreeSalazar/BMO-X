//! mod_wine_devour — standalone Ring 3 Windows PE devourer module.
//!
//! Converts Windows .exe (PE/COFF) binaries to BEF via goblin PE parsing.
//! Translates NT syscalls to BMO syscalls for Fase 1 (Hello World).
//!
//! ## Registry
//!   provides: pe.devour, pe.translate, pe.execute
//!   requires: heap.alloc, serial.write

#![no_std]
#![no_main]

extern crate alloc;

use core::alloc::{GlobalAlloc, Layout};
use core::panic::PanicInfo;

static mut HAL_PTR: *const bmo_hal_defs::HalServices = core::ptr::null();

struct KernelHeapAlloc;

unsafe impl GlobalAlloc for KernelHeapAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if let Some(hal) = HAL_PTR.as_ref() { (hal.heap_alloc)(layout.size(), layout.align()) }
        else { core::ptr::null_mut() }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if let Some(hal) = HAL_PTR.as_ref() { (hal.heap_free)(ptr, layout.size(), layout.align()); }
    }
}

#[global_allocator]
static ALLOCATOR: KernelHeapAlloc = KernelHeapAlloc;

mod devour;
mod shim;

#[no_mangle]
pub extern "C" fn _module_start(hal_ptr: *const bmo_hal_defs::HalServices) -> ! {
    unsafe { HAL_PTR = hal_ptr; }
    if let Some(hal) = unsafe { HAL_PTR.as_ref() } {
        (hal.serial_write)("[wine_devour] module loaded at 0x3130000\n");
    }

    // Register devour_pe with the BEF Linker so other BEFs can import it.
    bmo_abi::bef::linker::register_symbol(
        "bmo:module",
        "devour_pe",
        devour_pe as *const () as u64,
    );

    loop { unsafe { core::arch::asm!("hlt"); } }
}

/// Public API: devour a Windows .exe (PE) and return BEF bytes.
/// Called by desktop when user opens a .exe file.
#[no_mangle]
pub extern "C" fn devour_pe(
    pe_data: *const u8,
    pe_size: usize,
    out_bef: *mut u8,
    out_capacity: usize,
) -> usize {
    if pe_data.is_null() || pe_size == 0 || out_bef.is_null() || out_capacity == 0 {
        return 0;
    }
    let pe_bytes = unsafe { core::slice::from_raw_parts(pe_data, pe_size) };
    let bef_bytes = match devour::devour_pe_to_bef(pe_bytes) {
        Ok(b) => b,
        Err(_) => return 0,
    };
    if bef_bytes.len() > out_capacity { return 0; }
    unsafe { core::ptr::copy_nonoverlapping(bef_bytes.as_ptr(), out_bef, bef_bytes.len()); }
    bef_bytes.len()
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    if let Some(hal) = unsafe { HAL_PTR.as_ref() } {
        (hal.serial_write)("[wine_devour] PANIC: ");
        if let Some(s) = info.message().as_str() { (hal.serial_write)(s); }
        (hal.serial_write)("\n");
    }
    loop { unsafe { core::arch::asm!("hlt"); } }
}
