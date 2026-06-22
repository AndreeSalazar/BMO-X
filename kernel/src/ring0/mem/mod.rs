//! Memory API (Ring 0 HAL).
//!
//! Subsistema de memoria:
//!   - `heap`  — Bump heap allocator (BumpHeap)
//!   - `phys`  — Physical frame allocator (4 KB frames)
//!   - `virt`  — Page table walker (huge pages, CoW) — incluye `AddressSpace`,
//!                `Vma`, `VmaKind` (los VMA reales del sistema).
//!   - `space` — DEPRECATED v1.8.7: tipos VM duplicados con `virt`. Mantenido
//!                temporalmente por seguridad; eliminar cuando se confirme que
//!                nadie lo usa externamente.
//!
//! El orquestador `coordinator::main` invoca `phys::init`, `heap::init_heap`,
//! `virt::*` directamente. NO hay un `mem::init()` central (eliminado v1.8.7
//! porque era un wrapper trivial de `space::init()` que nadie llamaba).
//!
//! API pública:
//!   - `phys::alloc_pages(n)` / `free_pages(p, n)`
//!   - `virt::map_page(...)`, `virt::AddressSpace`, `virt::Vma`
//!   - `heap::init_heap`, `heap::heap_alloc`

#![allow(dead_code)]

pub mod heap;
pub mod phys;
pub mod virt;
#[allow(dead_code)]
pub mod space;
