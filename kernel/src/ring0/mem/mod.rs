//! Memory API (Ring 0 HAL).
//!
//! Subsistema de memoria:
//!   - `heap`  — Bump heap allocator (BumpHeap)
//!   - `phys`  — Physical frame allocator (4 KB frames)
//!   - `virt`  — Page table walker (huge pages, CoW)
//!   - `space` — Virtual memory manager (VMAs, demand paging, address spaces)
//!
//! El orquestador `coordinator::init()` llama a `crate::mem::init()`
//! (que llama a `space::init()`) y los drivers individuales (gop, etc.)
//! llaman a `phys::alloc_pages()` directamente.
//!
//! API pública:
//!   - `phys::alloc_pages(n)` / `free_pages(p, n)`
//!   - `virt::map_page(...)`
//!   - `space::get_or_create(pid)`, `space::create_process_space(pid)`

#![allow(dead_code)]

pub mod heap;
pub mod phys;
pub mod virt;
pub mod space;

/// Inicializa el subsistema de memoria. Llamar desde `coordinator::init()`.
pub fn init() {
    space::init();
}
