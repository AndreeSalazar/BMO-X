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

// ── Global allocator ──────────────────────────────────────────────────

static mut HAL_PTR: *const bmo_hal_defs::HalServices = core::ptr::null();

struct KernelHeapAlloc;

unsafe impl GlobalAlloc for KernelHeapAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if !HAL_PTR.is_null() {
            let hal = &*HAL_PTR;
            (hal.heap_alloc)(layout.size(), layout.align())
        } else { core::ptr::null_mut() }
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

// ── XhciHal implementation (wraps HalServices) ────────────────────────

struct ModuleXhciHal {
    hal: HalPtr,
}

#[derive(Copy, Clone)]
struct HalPtr(*const bmo_hal_defs::HalServices);

impl ModuleXhciHal {
    fn hal(&self) -> &bmo_hal_defs::HalServices {
        unsafe { &*self.hal.0 }
    }
}

impl bmo_xhci::XhciHal for ModuleXhciHal {
    fn alloc_dma_pages(&self, count: usize) -> Option<u64> {
        let phys = (self.hal().alloc_pages_contiguous)(count);
        if phys == 0 { None } else { Some(phys) }
    }
    fn phys_to_virt(&self, phys: u64) -> *mut u8 {
        if phys >= self.hal().HIGH_MEM_BASE {
            phys as *mut u8
        } else {
            // Below 4GB, identity-mapped by UEFI
            phys as *mut u8
        }
    }
    fn log(&self, msg: &str) { (self.hal().serial_write)(msg); }
    fn log_u64(&self, msg: &str, val: u64) { (self.hal().serial_write_u64)(val, 16); }
}

// ── Entry point ──────────────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn _module_start(hal_ptr: *const bmo_hal_defs::HalServices) -> ! {
    unsafe { HAL_PTR = hal_ptr; }

    if let Some(hal) = unsafe { HAL_PTR.as_ref() } {
        (hal.serial_write)("[mod_bmo_core] module loaded\n");

        // Init XHCI + USB HID for keyboard/mouse
        init_xhci(hal);

        // Wire USB HID poll into bmo_core's input layer
        unsafe {
            bmo_core::desktop::input::USB_HID_POLL = Some(poll_usb_hid);
        }

        // Initialize bmo_core with the kernel's HalServices
        unsafe { bmo_core::hal::init(*hal_ptr); }
        (hal.serial_write)("[mod_bmo_core] bmo_core HAL init complete\n");
        (hal.write_boot_stage)("coord_init");

        bmo_core::coord::init();

        (hal.write_crash_marker)(8);
        (hal.write_boot_stage)("welcome_dispatch");
        bmo_core::desktop::commands::enter_desktop();
    }

    loop { unsafe { core::arch::asm!("hlt"); } }
}

fn init_xhci(hal: &bmo_hal_defs::HalServices) {
    // Get XHCI MMIO from BootInfo (kernel filled it during PCIe scan)
    let xhci_mmio = unsafe {
        if (hal.boot_info).is_null() { 0 }
        else { (*(hal.boot_info)).xhci_mmio }
    };

    if xhci_mmio == 0 {
        (hal.serial_write)("[mod_bmo_core] no XHCI controller found, input via PS/2 only\n");
        return;
    }

    let backend = ModuleXhciHal { hal: HalPtr(hal as *const _) };
    let static_backend: &'static ModuleXhciHal = unsafe {
        // SAFETY: the backend lives for the module's lifetime
        core::mem::transmute::<&ModuleXhciHal, &'static ModuleXhciHal>(&backend)
    };

    bmo_xhci::init_hal(static_backend as &'static dyn bmo_xhci::XhciHal);
    bmo_xhci::set_mmio(xhci_mmio);

    (hal.serial_write)("[mod_bmo_core] XHCI init at 0x");
    (hal.serial_write_u64)(xhci_mmio, 16);
    (hal.serial_write)("\n");

    // Init USB HID
    let mut uhid = bmo_uhid::UsbHidHal::new();
    {
        use bmo_input::hal::InputHal;
        if uhid.init() {
            (hal.serial_write)("[mod_bmo_core] USB HID ready\n");
        } else {
            (hal.serial_write)("[mod_bmo_core] USB HID init failed\n");
        }
    }

    // Store UHID pointer for polling
    unsafe {
        UHID_PTR = Some(uhid);
    }
}

static mut UHID_PTR: Option<bmo_uhid::UsbHidHal> = None;

/// Public API for input layer: poll USB HID if XHCI is available.
/// Returns true if any event was available.
pub fn poll_usb_hid() -> bool {
    unsafe {
        if let Some(ref mut uhid) = UHID_PTR {
            let mut buf = [bmo_input::event::InputEvent::empty(); 32];
            use bmo_input::hal::InputHal;
            let n = uhid.poll(&mut buf);
            n > 0
        } else { false }
    }
}

// ── Panic handler ────────────────────────────────────────────────────

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    if let Some(hal) = unsafe { HAL_PTR.as_ref() } {
        (hal.serial_write)("[mod_bmo_core] PANIC: ");
        if let Some(s) = info.message().as_str() {
            (hal.serial_write)(s);
        }
        (hal.serial_write)("\n");
    }
    loop { unsafe { core::arch::asm!("hlt"); } }
}
