//! Kernel implementation of bmo_xhci::XhciHal.

use bmo_xhci::XhciHal;

pub struct KernelXhciHal;

impl XhciHal for KernelXhciHal {
    fn alloc_dma_pages(&self, count: usize) -> Option<u64> {
        unsafe { crate::mm::phys::alloc_pages_contiguous(count) }
    }

    fn phys_to_virt(&self, phys: u64) -> *mut u8 {
        crate::mm::vmm::phys_to_virt(phys) as *mut u8
    }

    fn log(&self, msg: &str) {
        crate::dev::console::serial_write(msg);
    }

    fn log_u64(&self, msg: &str, val: u64) {
        crate::dev::console::serial_write(msg);
        crate::dev::console::serial_write_u64(val, 16);
        crate::dev::console::serial_write("\n");
    }
}
