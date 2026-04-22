//! FastOS Platform — nv_hal::Platform implementation for Ring 0.
//!
//! Bridges the kernel's hardware access to the GPU driver stack.
//! All operations run in Ring 0, identity-mapped by UEFI page tables.
//! PCI config space access uses ECAM MMIO (no legacy I/O ports).

use nv_hal::{Platform, PciAddress, DmaBuffer};

pub struct FastOsPlatform;

impl FastOsPlatform {
    pub const fn new() -> Self {
        Self
    }
}

impl Platform for FastOsPlatform {
    fn pci_config_read32(&self, addr: PciAddress, offset: u8) -> u32 {
        crate::drivers::pci::pci_read32(
            addr.bus, addr.device, addr.function, offset as u16,
        )
    }

    fn pci_config_write32(&self, addr: PciAddress, offset: u8, value: u32) {
        crate::drivers::pci::pci_write32(
            addr.bus, addr.device, addr.function, offset as u16, value,
        )
    }

    fn map_mmio(&self, phys_addr: u64, _size: usize) -> *mut u8 {
        // Identity-mapped by UEFI page tables (firmware leaves them active).
        phys_addr as *mut u8
    }

    fn unmap_mmio(&self, _virt_addr: *mut u8, _size: usize) {
        // Identity mapping — nothing to unmap.
    }

    fn alloc_dma(&self, size: usize) -> Option<DmaBuffer> {
        // Use the UEFI memory map to find usable regions.
        // For now: bump allocator from a safe region above kernel.
        // TODO: Use page_alloc with the UEFI memory map.
        static mut DMA_NEXT: u64 = 0x0040_0000; // 4MB

        unsafe {
            let aligned = (DMA_NEXT + 0xFFF) & !0xFFF; // 4KB align
            let end = aligned + size as u64;
            if end > 0x0080_0000 { // 8MB limit
                return None;
            }
            DMA_NEXT = end;

            // Zero the buffer
            core::ptr::write_bytes(aligned as *mut u8, 0, size);

            Some(DmaBuffer {
                virt: aligned as *mut u8,
                phys: aligned, // identity mapped
                size,
            })
        }
    }

    fn free_dma(&self, _buf: DmaBuffer) {
        // Bump allocator — no individual free.
    }

    fn stall_us(&self, us: u32) {
        // rdtsc-based busy wait.
        // Ryzen 5 5600X base: 3.7GHz, TSC ticks ≈ CPU cycles.
        let ticks_per_us: u64 = 3700;
        let target = ticks_per_us * us as u64;
        let start = rdtsc();
        while rdtsc().wrapping_sub(start) < target {
            core::hint::spin_loop();
        }
    }
}

#[inline]
fn rdtsc() -> u64 {
    let (lo, hi): (u32, u32);
    unsafe { core::arch::asm!("rdtsc", out("eax") lo, out("edx") hi); }
    ((hi as u64) << 32) | lo as u64
}
