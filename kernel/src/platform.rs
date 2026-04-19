//! FastOS Platform — nv_hal::Platform implementation for Ring 0.
//!
//! Bridges the kernel's hardware access to the GPU driver stack.
//! All operations run in Ring 0, identity-mapped first 4GB.

use nv_hal::{Platform, PciAddress, DmaBuffer, MmioRegion};

pub struct FastOsPlatform;

impl FastOsPlatform {
    pub const fn new() -> Self {
        Self
    }
}

#[inline]
fn outl(port: u16, val: u32) {
    unsafe { core::arch::asm!("out dx, eax", in("dx") port, in("eax") val); }
}

#[inline]
fn inl(port: u16) -> u32 {
    let v: u32;
    unsafe { core::arch::asm!("in eax, dx", in("dx") port, out("eax") v); }
    v
}

impl Platform for FastOsPlatform {
    fn pci_config_read32(&self, addr: PciAddress, offset: u8) -> u32 {
        let cfg = addr.config_addr(offset);
        outl(0x0CF8, cfg);
        inl(0x0CFC)
    }

    fn pci_config_write32(&self, addr: PciAddress, offset: u8, value: u32) {
        let cfg = addr.config_addr(offset);
        outl(0x0CF8, cfg);
        outl(0x0CFC, value);
    }

    fn map_mmio(&self, phys_addr: u64, _size: usize) -> *mut u8 {
        // Identity-mapped by bootloader (first 4GB via 2MB pages).
        // Physical address = virtual address.
        phys_addr as *mut u8
    }

    fn unmap_mmio(&self, _virt_addr: *mut u8, _size: usize) {
        // Identity mapping — nothing to unmap.
    }

    fn alloc_dma(&self, size: usize) -> Option<DmaBuffer> {
        // Simple bump allocator from a fixed region above kernel.
        // Kernel ends at ~1MB + 128KB = 0x120000.
        // Use 2MB-4MB range for DMA buffers (identity mapped).
        static mut DMA_NEXT: u64 = 0x0020_0000; // 2MB

        unsafe {
            let aligned = (DMA_NEXT + 0xFFF) & !0xFFF; // 4KB align
            let end = aligned + size as u64;
            if end > 0x0040_0000 { // 4MB limit
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
        // Use rdtsc-based busy wait.
        // Ryzen 5 5600X base: 3.7GHz, TSC ticks ~= CPU cycles.
        // ~3700 ticks per microsecond.
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
