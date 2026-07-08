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

// ── Global allocator: delegates to kernel via HalServices ────────────

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

// ── Entry point called by kernel module loader ──────────────────────

#[no_mangle]
pub extern "C" fn _module_start(hal_ptr: *const bmo_hal_defs::HalServices) -> ! {
    unsafe { HAL_PTR = hal_ptr; }

    // Enable serial output early for diagnostics
    if let Some(hal) = unsafe { HAL_PTR.as_ref() } {
        (hal.register_cabina_sink)();
        (hal.cabina_init)();
        (hal.serial_write)("[mod_bmo_core] module loaded, HAL wired\n");

        // Initialize bmo_core with the kernel's HalServices
        unsafe {
            bmo_core::hal::init(*hal_ptr);
        }
        (hal.serial_write)("[mod_bmo_core] bmo_core HAL init complete\n");
        (hal.write_crash_marker)(6);
        (hal.write_boot_stage)("coord_init");

        // Start desktop (welcome screen)
        bmo_core::coord::init();

        (hal.serial_write)("[mod_bmo_core] coord::init complete, entering desktop\n");
        (hal.write_crash_marker)(8);
        (hal.write_boot_stage)("welcome_dispatch");

        bmo_core::desktop::commands::enter_desktop();
    }

    loop { unsafe { core::arch::asm!("hlt"); } }
}

// ── Panic handler ──────────────────────────────────────────────────

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    if let Some(hal) = unsafe { HAL_PTR.as_ref() } {
        (hal.serial_write)("[mod_bmo_core] PANIC: ");
        let msg = info.message();
        if let Some(s) = msg.as_str() {
            (hal.serial_write)(s);
        }
        (hal.serial_write)("\n");
    }
    loop { unsafe { core::arch::asm!("hlt"); } }
}

// ── Alloc error handler (default handle_alloc_error → panic) ────────────

