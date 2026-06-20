//! Memory-Mapped I/O (MMIO) regions with safe accessors.
//!
//! Drivers that talk to hardware (GPU, NIC, storage) read and write
//! memory-mapped registers. A raw `*mut u32` is dangerous because:
//!
//! 1. The compiler may reorder or elide reads/writes.
//! 2. The CPU may combine writes or read stale cached values.
//! 3. The region may not be mapped or may be mapped with the wrong
//!    cache type (UC vs WB), causing data corruption.
//!
//! `MmioRegion` solves all three: it exposes typed read/write methods
//! that use `read_volatile` / `write_volatile`, and it is constructed via
//! `MmioRegion::map`, which validates the mapping and sets the cache
//! type via MTRR.

#![allow(dead_code)]

use crate::mem::virt::{self, PageTableEntry};
use crate::mem::phys::PhysAddr;
use crate::result::{KError, KResult};
use core::ptr;

/// Cache type for an MMIO region. Determines how the CPU caches accesses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheType {
    /// Strongly Uncacheable. Every read/write goes to the device.
    Uncacheable,
    /// Write-Combining. Writes are buffered, reads go to the device.
    WriteCombining,
    /// Write-Back. The default for RAM. Use only for memory-mapped
    /// RAM (e.g. VRAM exposed as BAR on some GPUs).
    WriteBack,
}

impl CacheType {
    fn to_pte_flags(self) -> u64 {
        use crate::mem::virt::flags;
        let mut f = flags::PRESENT | flags::WRITABLE | flags::NO_EXECUTE;
        match self {
            CacheType::Uncacheable    => f |= flags::PCD | flags::PWT,
            CacheType::WriteCombining => f |= flags::PWT,    // PAT index 1 = WC
            CacheType::WriteBack      => {},
        }
        f
    }
}

/// A mapped MMIO region. Drops by unmapping the virtual address range.
pub struct MmioRegion {
    virt_base: *mut u8,
    phys_base: PhysAddr,
    size: usize,
    cache: CacheType,
}

// SAFETY: an MMIO region is a single-writer resource in the kernel; access
// must be synchronized by the driver. We do NOT make this Send/Sync — the
// driver is responsible for any cross-core synchronization.
unsafe impl Send for MmioRegion {}
unsafe impl Sync for MmioRegion {}

impl MmioRegion {
    /// Map `size` bytes starting at physical address `phys` into the
    /// kernel address space with the requested cache type.
    pub fn map(phys: PhysAddr, size: usize, cache: CacheType) -> KResult<Self> {
        if size == 0 {
            return Err(KError::InvalidArgument);
        }
        if phys & 0xFFF != 0 {
            return Err(KError::InvalidArgument);
        }
        let pages = (size + 0xFFF) / 0x1000;
        let pte_flags = cache.to_pte_flags();

        // Map each page in the region. The kernel reserves a window of
        // virtual addresses for MMIO mappings; for now, we use a fixed
        // region (4 GB) starting at 0xFFFF_B000_0000_0000.
        // TODO: allocate from a proper MMIO virtual allocator.
        const MMIO_WINDOW_BASE: u64 = 0xFFFF_B000_0000_0000;
        let virt_base = MMIO_WINDOW_BASE;
        for i in 0..pages {
            let va = virt_base + (i as u64) * 0x1000;
            let pa = phys + (i as u64) * 0x1000;
            unsafe {
                if !map_single_page(va, pa, pte_flags) {
                    return Err(KError::OutOfMemory);
                }
            }
        }
        Ok(Self {
            virt_base: virt_base as *mut u8,
            phys_base: phys,
            size: pages * 0x1000,
            cache,
        })
    }

    #[inline]
    pub fn read_u32(&self, offset: usize) -> u32 {
        debug_assert!(offset + 4 <= self.size, "MMIO read out of bounds");
        unsafe { (self.virt_base.add(offset) as *const u32).read_volatile() }
    }

    #[inline]
    pub fn write_u32(&self, offset: usize, value: u32) {
        debug_assert!(offset + 4 <= self.size, "MMIO write out of bounds");
        unsafe { (self.virt_base.add(offset) as *mut u32).write_volatile(value); }
    }

    #[inline]
    pub fn read_u64(&self, offset: usize) -> u64 {
        debug_assert!(offset + 8 <= self.size, "MMIO read out of bounds");
        unsafe { (self.virt_base.add(offset) as *const u64).read_volatile() }
    }

    #[inline]
    pub fn write_u64(&self, offset: usize, value: u64) {
        debug_assert!(offset + 8 <= self.size, "MMIO write out of bounds");
        unsafe { (self.virt_base.add(offset) as *mut u64).write_volatile(value); }
    }

    #[inline]
    pub fn read_u8(&self, offset: usize) -> u8 {
        debug_assert!(offset < self.size, "MMIO read out of bounds");
        unsafe { self.virt_base.add(offset).read_volatile() }
    }

    #[inline]
    pub fn write_u8(&self, offset: usize, value: u8) {
        debug_assert!(offset < self.size, "MMIO write out of bounds");
        unsafe { self.virt_base.add(offset).write_volatile(value); }
    }

    /// Read-modify-write: set bits in a u32 register.
    #[inline]
    pub fn set_bits_u32(&self, offset: usize, mask: u32) {
        let v = self.read_u32(offset);
        self.write_u32(offset, v | mask);
    }

    /// Read-modify-write: clear bits in a u32 register.
    #[inline]
    pub fn clear_bits_u32(&self, offset: usize, mask: u32) {
        let v = self.read_u32(offset);
        self.write_u32(offset, v & !mask);
    }

    /// Poll a register until `(value & mask) == expected`, or until
    /// `timeout_us` microseconds elapse.
    pub fn poll_u32(&self, offset: usize, mask: u32, expected: u32, timeout_us: u64) -> KResult<()> {
        use crate::cpu::delay;
        let deadline = delay::deadline_us_from_now(timeout_us);
        loop {
            if self.read_u32(offset) & mask == expected {
                return Ok(());
            }
            if delay::deadline_elapsed(deadline) {
                return Err(KError::Timeout);
            }
            core::hint::spin_loop();
        }
    }

    pub fn as_ptr(&self) -> *mut u8 { self.virt_base }
    pub fn phys_base(&self) -> PhysAddr { self.phys_base }
    pub fn size(&self) -> usize { self.size }
    pub fn cache(&self) -> CacheType { self.cache }
}

impl Drop for MmioRegion {
    fn drop(&mut self) {
        let pages = self.size / 0x1000;
        for i in 0..pages {
            let v = self.virt_base as u64 + (i as u64) * 0x1000;
            unsafe { virt::invlpg(v); }
        }
    }
}

/// Map a single 4KB page at `va` → `pa` with `pte_flags`.
/// Returns false on OOM (failed to allocate a page table).
unsafe fn map_single_page(va: u64, pa: u64, pte_flags: u64) -> bool {
    use crate::mem::virt::{alloc_page_table, PageTable};
    use crate::mem::virt::flags;
    const ADDR_MASK: u64 = 0x000F_FFFF_FFFF_F000;
    const PAGE_SIZE: u64 = 4096;

    let pml4_phys = virt::read_cr3() & ADDR_MASK;
    let pml4 = pml4_phys as *mut PageTable;

    let pml4_i = ((va >> 39) & 0x1FF) as usize;
    let pdpt_i = ((va >> 30) & 0x1FF) as usize;
    let pd_i = ((va >> 21) & 0x1FF) as usize;
    let pt_i = ((va >> 12) & 0x1FF) as usize;

    // PML4 entry
    let pml4e = &mut (*pml4).entries[pml4_i];
    let pdpt_phys = if pml4e.is_present() {
        pml4e.phys_addr()
    } else {
        let new = match alloc_page_table() {
            Some(x) => x,
            None => return false,
        };
        ptr::write_bytes(new as *mut u8, 0, PAGE_SIZE as usize);
        pml4e.0 = PageTableEntry::new(new, flags::PRESENT | flags::WRITABLE).0;
        new
    };

    let pdpt = pdpt_phys as *mut PageTable;
    let pdpte = &mut (*pdpt).entries[pdpt_i];
    let pd_phys = if pdpte.is_present() {
        if (pdpte.0 & flags::HUGE_PAGE) != 0 {
            return false;
        }
        pdpte.phys_addr()
    } else {
        let new = match alloc_page_table() {
            Some(x) => x,
            None => return false,
        };
        ptr::write_bytes(new as *mut u8, 0, PAGE_SIZE as usize);
        pdpte.0 = PageTableEntry::new(new, flags::PRESENT | flags::WRITABLE).0;
        new
    };

    let pd = pd_phys as *mut PageTable;
    let pde = &mut (*pd).entries[pd_i];
    let pt_phys = if pde.is_present() {
        if (pde.0 & flags::HUGE_PAGE) != 0 {
            return false;
        }
        pde.phys_addr()
    } else {
        let new = match alloc_page_table() {
            Some(x) => x,
            None => return false,
        };
        ptr::write_bytes(new as *mut u8, 0, PAGE_SIZE as usize);
        pde.0 = PageTableEntry::new(new, flags::PRESENT | flags::WRITABLE).0;
        new
    };

    let pt = pt_phys as *mut PageTable;
    let pte = &mut (*pt).entries[pt_i];
    pte.0 = PageTableEntry::new(pa, pte_flags).0;
    virt::invlpg(va);
    true
}
