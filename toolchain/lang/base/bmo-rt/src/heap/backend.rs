//! Backends for the freelist allocator.
//!
//! `SyscallBackend` pide los trozos a `KIND_MEMORIA` (`crate::syscall::memoria_pedir`).

use super::freelist::MemBackend;

/// Trozos de `KIND_MEMORIA`: se piden y NO se devuelven, porque el kernel no
/// tiene forma de recibirlos. El asignador de encima es quien reparte.
#[derive(Debug, Clone, Copy)]
pub struct SyscallBackend;

impl SyscallBackend {
    pub const fn new() -> Self {
        Self
    }
}

impl MemBackend for SyscallBackend {
    unsafe fn alloc_chunk(&self, min_size: usize) -> *mut u8 {
        crate::syscall::memoria_pedir(min_size as u64)
    }

    /// Nada: un bloque de `KIND_MEMORIA` no se devuelve. Antes esto llamaba a
    /// `bmo_mem_free` (0x191), que el kernel contestaba con "no existe".
    unsafe fn free_chunk(&self, _ptr: *mut u8, _size: usize) {}
}

#[cfg(test)]
pub mod test_backend {
    use core::ptr;
    use super::super::freelist::MemBackend;
    use std::alloc::{alloc, dealloc, Layout};

    #[derive(Debug, Clone, Copy)]
    pub struct TestBackend;

    impl MemBackend for TestBackend {
        unsafe fn alloc_chunk(&self, min_size: usize) -> *mut u8 {
            let layout = Layout::from_size_align(min_size, 8).expect("overflow");
            let ptr = alloc(layout);
            if ptr.is_null() { ptr::null_mut() } else { ptr }
        }

        unsafe fn free_chunk(&self, ptr: *mut u8, _size: usize) {
            let _ = dealloc(ptr, Layout::from_size_align(1, 8).unwrap());
        }
    }
}
