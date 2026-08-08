//! StorageHal -- trait for kernel services needed by storage drivers.
//!
//! Implemented by the kernel and injected via global function pointers.

use core::sync::atomic::{AtomicBool, Ordering};

/// Trait implemented by the kernel for storage driver services.
pub trait StorageHal {
    /// Allocate contiguous physical pages for DMA buffers.
    fn alloc_dma_pages(&self, count: usize) -> Option<u64>;
    /// Free DMA pages.
    fn free_dma_pages(&self, addr: u64, count: usize);
    /// Convert physical address to virtual (for DMA buffer access).
    fn phys_to_virt(&self, phys: u64) -> *mut u8;
    /// Log a message for diagnostics.
    fn log(&self, msg: &str);
    /// Log a message followed by a value in HEX. Los registros de un
    /// controlador se leen en hexadecimal; obligar a cada driver a fabricar
    /// la cadena a mano es como se acaba sin imprimir el numero que hacia
    /// falta.
    fn log_hex(&self, msg: &str, value: u64);
    /// Espera de tiempo REAL en milisegundos.
    ///
    /// Los tiempos del SATA son fisicos: un COMRESET dura milisegundos, y
    /// negociar el enlace o arrancar un disco, decenas o cientos. Contar
    /// vueltas de bucle mide la velocidad del CPU, no el tiempo -- y por eso
    /// un mismo numero funciona en una maquina y falla en otra.
    fn delay_ms(&self, ms: u64);
}

/// Static storage for the HAL singleton (set by kernel at boot).
static mut STORAGE_HAL: Option<&'static dyn StorageHal> = None;
static INIT: AtomicBool = AtomicBool::new(false);

/// Called by kernel to wire the HAL before using storage drivers.
pub fn init_hal(hal: &'static dyn StorageHal) {
    if INIT.swap(true, Ordering::SeqCst) { return; }
    unsafe { STORAGE_HAL = Some(hal); }
}

/// Get reference to the storage HAL. Panics if not initialized.
pub fn hal() -> &'static dyn StorageHal {
    unsafe { STORAGE_HAL.expect("StorageHal not initialized") }
}
