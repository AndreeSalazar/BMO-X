//! Memory API (Ring 0 HAL).
//!
//! Subsistema de memoria:
//!   - `heap`        — Bump heap allocator (BumpHeap)
//!   - `page_alloc`  — Bitmap allocator (páginas físicas 16MB-4GB)
//!   - `paging`      — Page table walker (huge pages, CoW)
//!   - `vmm`         — Virtual memory manager (VMAs, demand paging)
//!
//! El orquestador `coordinator::init()` llama a `crate::memory::init()`
//! (que llama a `vmm::init()`) y los drivers individuales (gop, etc.)
//! llaman a `page_alloc::alloc_pages()` directamente.
//!
//! API pública:
//!   - `page_alloc::alloc_pages(n)` / `free_pages(p, n)`
//!   - `paging::map_page(...)` (cuando se implemente)
//!   - `vmm::get_or_create(pid)`, `vmm::create_process_space(pid)`

#![allow(dead_code)]

pub mod heap;
pub mod page_alloc;
pub mod paging;
pub mod vmm;

/// Inicializa el subsistema de memoria. Llamar desde `coordinator::init()`.
pub fn init() {
    vmm::init();
}
