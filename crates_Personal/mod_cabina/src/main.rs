//! mod_cabina — standalone Ring 3 telemetry/logging module loaded by kernel.elf.
//!
//! Provides cabina-daemon (ring buffer, telemetry counters, serial sink)
//! running as a Ring 3 module independent of kernel.elf.

#![no_std]
#![no_main]

extern crate alloc;

use core::alloc::{GlobalAlloc, Layout};
use core::panic::PanicInfo;

static mut HAL_PTR: *const bmo_hal_defs::HalServices = core::ptr::null();

struct KernelHeapAlloc;

unsafe impl GlobalAlloc for KernelHeapAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if !HAL_PTR.is_null() {
            let hal = &*HAL_PTR;
            (hal.heap_alloc)(layout.size(), layout.align())
        } else {
            core::ptr::null_mut()
        }
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

#[no_mangle]
pub extern "C" fn _module_start(hal_ptr: *const bmo_hal_defs::HalServices) -> ! {
    unsafe { HAL_PTR = hal_ptr; }

    if let Some(hal) = unsafe { HAL_PTR.as_ref() } {
        (hal.serial_write)("[mod_cabina] module loaded\n");

        // Initialize cabina daemon
        (hal.cabina_init)();
        (hal.register_cabina_sink)();
        (hal.serial_write)("[mod_cabina] cabina daemon initialized\n");
    }

    loop { unsafe { core::arch::asm!("hlt"); } }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    if let Some(hal) = unsafe { HAL_PTR.as_ref() } {
        (hal.serial_write)("[mod_cabina] PANIC: ");
        if let Some(s) = info.message().as_str() {
            (hal.serial_write)(s);
        }
        (hal.serial_write)("\n");
    }
    loop { unsafe { core::arch::asm!("hlt"); } }
}
