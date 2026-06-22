//! `bmo_abi::memory` — Interfaz de allocator y modelo de memoria.
//!
//! Define el trait `BmoAllocator` que **cualquier** allocator (del
//! programa, no del kernel) puede implementar para pedir memoria al
//! kernel vía syscalls.
//!
//! ## Syscalls
//!
//! - `NR_MEM_ALLOC` (0x190) → `bmo_mem_alloc(size, align) -> u64`
//! - `NR_MEM_FREE`  (0x191) → `bmo_mem_free(ptr)`
//! - `NR_MEM_MAP`   (0x192) → `bmo_mem_map(fd, len, prot, flags) -> u64`
//! - `NR_MEM_UNMAP` (0x193) → `bmo_mem_unmap(ptr, len)`
//!
//! ## Garantías
//!
//! - `bmo_mem_alloc(size, 16)` retorna un puntero alineado a 16 bytes.
//! - `bmo_mem_free(NULL)` es no-op.
//! - Memoria de datos es `NOEXEC` por defecto.
//! - Memoria de stack es `RW` y crece hacia abajo.

#![allow(dead_code)]

// ─── Allocation flags ──────────────────────────────────────────────

/// Permisos para una región de memoria.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BmoMemProt {
    /// Sin permisos.
    None  = 0,
    /// Lectura.
    Read  = 1,
    /// Escritura.
    Write = 2,
    /// Ejecución.
    Exec  = 4,
    /// Lectura + escritura.
    RW    = 3,
    /// Lectura + ejecución.
    RX    = 5,
    /// Lectura + escritura + ejecución.
    RWX   = 7,
}

impl BmoMemProt {
    pub const R:    Self = Self::Read;
    pub const W:    Self = Self::Write;
    pub const X:    Self = Self::Exec;
}

/// Flags para `bmo_mem_alloc`.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BmoAllocFlag {
    /// Zero-initialize la memoria.
    Zero    = 1,
    /// Memoria es ejecutable.
    Exec    = 2,
    /// Memoria es lockable (no se puede paginar).
    Locked  = 4,
    /// Memoria es compartida entre procesos.
    Shared  = 8,
    /// Memoria es de 32-bit (low mem).
    LowMem  = 16,
    /// Memoria es de 64-bit (high mem).
    HighMem = 32,
}

// ─── Allocator interface ──────────────────────────────────────────

/// Trait que un allocator del programa puede implementar.
///
/// El allocator pide memoria al kernel mediante los syscalls
/// `NR_MEM_ALLOC`. Esto NO es el allocator del kernel (eso vive en
/// `crate::ring0::mem`), sino un allocator de **userland** que delega
/// al kernel.
pub trait BmoAllocator {
    /// Pide `size` bytes con al menos `align` bytes de alineación.
    /// Retorna `None` si el kernel no puede satisfacer el pedido.
    fn alloc(&mut self, size: usize, align: usize) -> Option<*mut u8>;

    /// Libera un bloque previamente pedido con `alloc`.
    /// `ptr` debe provenir de `alloc`; cualquier otro valor es UB.
    fn free(&mut self, ptr: *mut u8, size: usize);

    /// Tamaño total reservado por este allocator (para estadísticas).
    fn total_allocated(&self) -> usize { 0 }

    /// Pico de uso (high water mark).
    fn peak_allocated(&self) -> usize { 0 }
}

// ─── Default implementation ───────────────────────────────────────

/// Allocator trivial que delega 1:1 al kernel.
pub struct BmoKernelAllocator;

impl BmoAllocator for BmoKernelAllocator {
    fn alloc(&mut self, _size: usize, _align: usize) -> Option<*mut u8> {
        // Implementación real en un crate aparte que use este trait.
        // Aquí solo declaramos la interfaz.
        None
    }
    fn free(&mut self, _ptr: *mut u8, _size: usize) {}
}
