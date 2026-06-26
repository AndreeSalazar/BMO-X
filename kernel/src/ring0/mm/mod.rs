//! Memory Management (Ring 0 HAL).
//!
//! Subsistema de administración de memoria:
//!   - `frame_alloc` — Physical Frame Allocator (bitmap, 4 KiB frames)
//!   - `slab`        — Kernel Slab Allocator (free-list, first-fit, coalescing)
//!   - `vmm`         — Virtual Memory Manager (page tables, VMA, demand paging, CoW)
//!
//! Arquitectura DDR4:
//!   - Frame allocator: bitmap 128 KB, tracking 16 MB → 4 GB
//!   - Slab allocator: 32 MB estático, crecerá en v1.9
//!   - VMM: 4-level x86-64 page tables, huge pages (2 MiB / 1 GiB)
//!
//! Inicialización (coordinator::main):
//!   1. `frame_alloc::init()` — parsear UEFI memory map
//!   2. `slab::init_heap()`   — inicializar slab allocator
//!   3. `vmm::*`              — page table operations
//!
//! API pública:
//!   - `frame_alloc::alloc_pages(n)` / `free_pages(p, n)`
//!   - `vmm::map_page(...)`, `vmm::AddressSpace`, `vmm::Vma`
//!   - `slab::init_heap`, `slab::heap_alloc`

#![allow(dead_code)]

pub mod frame_alloc;
pub mod slab;
pub mod vmm;

pub use frame_alloc as phys;
pub use slab as heap;
pub use vmm as virt;
