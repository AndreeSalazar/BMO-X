//! Backends for the freelist allocator.
//!
//! Currently provides `SyscallBackend` which uses `bmo_mem_alloc`/`bmo_mem_free`.
//! Future: `DirectBackend` for Ring 0 (calls kernel heap directly).

use super::freelist::MemBackend;
use bmo_abi::syscalls::{self, syscall1, syscall2};

/// A memory backend that requests chunks via `bmo_mem_alloc` / `bmo_mem_free`.
#[derive(Debug, Clone, Copy)]
pub struct SyscallBackend;

impl SyscallBackend {
    pub const fn new() -> Self {
        Self
    }
}

impl MemBackend for SyscallBackend {
    unsafe fn alloc_chunk(&self, min_size: usize) -> *mut u8 {
        let result = syscall1(syscalls::NR_MEM_ALLOC, min_size as u64);
        let ptr = result.code() as *mut u8;
        if ptr.is_null() || (ptr as usize) < 0x1000 {
            core::ptr::null_mut()
        } else {
            ptr
        }
    }

    unsafe fn free_chunk(&self, ptr: *mut u8, size: usize) {
        let _ = syscall2(syscalls::NR_MEM_FREE, ptr as u64, size as u64);
    }
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
