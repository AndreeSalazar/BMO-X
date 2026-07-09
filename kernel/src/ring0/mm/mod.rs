//! Memory Management (Ring 0 HAL).
//!
//! Subsistema de administraciÃ³n de memoria:
//!   - `frame_alloc` â€” Physical Frame Allocator (bitmap, 4 KiB frames)
//!   - `slab`        â€” Kernel Slab Allocator (free-list, first-fit, coalescing)
//!   - `vmm`         â€” Virtual Memory Manager (page tables, VMA, demand paging, CoW)
//!
//! Arquitectura DDR4:
//!   - Frame allocator: bitmap 128 KB, tracking 16 MB â†’ 4 GB
//!   - Slab allocator: 32 MB estÃ¡tico, crecerÃ¡ en v1.9
//!   - VMM: 4-level x86-64 page tables, huge pages (2 MiB / 1 GiB)
//!
//! InicializaciÃ³n (coordinator::main):
//!   1. `frame_alloc::init()` â€” parsear UEFI memory map
//!   2. `slab::init_heap()`   â€” inicializar slab allocator
//!   3. `vmm::*`              â€” page table operations


pub const PAGE_SIZE: u64 = 4096;
pub(crate) const MAX_ORDER: usize = 11;

use bmo_boot_protocol::MemoryEntry;

/// Abstract interface for the physical page backing allocator.
pub trait BackingAllocator: Sync {
    unsafe fn init(&self, memory_map: &[MemoryEntry], count: usize,
                   reserved_addr: u64, reserved_size: u64,
                   kernel_base: u64, kernel_size: u64);
    unsafe fn free_high_memory(&self, memory_map: &[MemoryEntry], count: usize);
    unsafe fn alloc_order(&self, order: usize) -> Option<u64>;
    unsafe fn free_order(&self, addr: u64, order: usize);
    fn free_count(&self) -> usize;
    fn total_ram(&self) -> u64;
    fn tracked_pages(&self) -> usize;
}

#[cfg(not(feature = "alloc-llfree"))]
pub(crate) mod buddy;
#[cfg(feature = "alloc-llfree")]
pub(crate) mod llfree;

pub mod frame_alloc;
pub mod slab;
pub mod vmm;
pub mod vdso;

pub use frame_alloc as phys;
pub use slab as heap;
pub use vmm as virt;
