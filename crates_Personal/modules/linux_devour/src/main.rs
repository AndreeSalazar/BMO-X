//! mod_linux_devour — standalone Ring 3 Linux ELF devourer module.
//!
//! Loads ELF binaries, translates Linux syscalls to BMO syscalls,
//! wraps them as BEF, and executes them.
//!
//! ## Architecture
//!   - goblin parses ELF → sections (PT_LOAD)
//!   - Patches `syscall` instructions to call our shim
//!   - Injects `syscall_shim.c` that translates Linux→BMO
//!   - BefBuilder wraps everything → .bef binary
//!   - Returns entry point to caller (desktop)

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

/// Entry point called by desktop (not by kernel loader directly).
/// Receives HalServices pointer and a buffer containing the ELF binary.
#[no_mangle]
pub extern "C" fn _module_start(hal_ptr: *const bmo_hal_defs::HalServices) -> ! {
    unsafe { HAL_PTR = hal_ptr; }

    if let Some(hal) = unsafe { HAL_PTR.as_ref() } {
        (hal.serial_write)("[linux_devour] module loaded at 0x3120000\n");
    }

    // This module is started by the desktop on-demand, not at boot.
    // The desktop calls our entry with an ELF buffer to devour.
    loop { unsafe { core::arch::asm!("hlt"); } }
}

/// Public API: devour an ELF binary and return a BEF binary.
/// Called by the desktop when a user opens an ELF file.
#[no_mangle]
pub extern "C" fn devour_elf(
    elf_data: *const u8,
    elf_size: usize,
    out_bef: *mut u8,
    out_capacity: usize,
) -> usize {
    if elf_data.is_null() || elf_size == 0 || out_bef.is_null() || out_capacity == 0 {
        return 0;
    }

    let elf_bytes = unsafe { core::slice::from_raw_parts(elf_data, elf_size) };
    let bef_bytes = match devour::devour_elf_to_bef(elf_bytes) {
        Ok(b) => b,
        Err(_) => return 0,
    };

    if bef_bytes.len() > out_capacity { return 0; }

    unsafe {
        core::ptr::copy_nonoverlapping(bef_bytes.as_ptr(), out_bef, bef_bytes.len());
    }
    bef_bytes.len()
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    if let Some(hal) = unsafe { HAL_PTR.as_ref() } {
        (hal.serial_write)("[linux_devour] PANIC: ");
        if let Some(s) = info.message().as_str() { (hal.serial_write)(s); }
        (hal.serial_write)("\n");
    }
    loop { unsafe { core::arch::asm!("hlt"); } }
}
