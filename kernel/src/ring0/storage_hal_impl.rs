//! Kernel implementation of bmo_ahci::StorageHal.
//!
//! Bridges the AHCI crate to kernel services: physical memory allocation,
//! MMIO access, and diagnostic logging.

use bmo_ahci::storage_hal::StorageHal;

pub struct KernelStorageHal;

impl StorageHal for KernelStorageHal {
    fn alloc_dma_pages(&self, count: usize) -> Option<u64> {
        unsafe { crate::mm::phys::alloc_pages_contiguous(count) }
    }

    fn free_dma_pages(&self, addr: u64, count: usize) {
        unsafe { crate::mm::phys::free_pages(addr, count); }
    }

    fn phys_to_virt(&self, phys: u64) -> *mut u8 {
        crate::mm::vmm::phys_to_virt(phys) as *mut u8
    }

    fn log(&self, msg: &str) {
        crate::dev::console::serial_write(msg);
    }
}
