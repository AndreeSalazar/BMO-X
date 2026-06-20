//! Memory subsystem for FastOS.
//!
//! Modular memory management:
//!   - Page allocator (physical pages) — vive en `arch::page_alloc`
//!   - VMM (virtual memory manager — VMA management, CoW, demand paging)
//!
//! El orquestador `ring_0::init()` llama a:
//!   - `arch::page_alloc::init(boot_info)`
//!   - `memory::vmm::init()`
//!   - `memory::init()` (este módulo, no-op por ahora)

pub mod vmm;

/// Inicializa el subsistema de memoria. Llama a vmm::init() y
/// cualquier inicialización adicional de page_alloc.
pub fn init() {
    vmm::init();
}

