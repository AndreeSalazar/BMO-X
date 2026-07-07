//! Heap: free-list allocator with a syscall backend.
//!
//! Provides `malloc`, `free`, `realloc`, `calloc` convenience functions
//! that use a global `FreelistAllocator<SyscallBackend>` instance.
//!
//! The global allocator doubles as `#[global_allocator]` for Rust's `alloc` crate.

pub mod backend;
pub mod freelist;

use backend::SyscallBackend;
use freelist::FreelistAllocator;
use crate::init;
use core::alloc::{GlobalAlloc, Layout};

/// The global heap allocator instance for BMO.
/// Only registered as the global allocator when not testing (test runner runs on the host OS).
#[cfg_attr(not(test), global_allocator)]
pub static HEAP: FreelistAllocator<SyscallBackend> = FreelistAllocator::new_with(SyscallBackend::new());

/// Allocate `size` bytes and return a pointer (8-byte aligned).
/// Returns null on failure.
pub fn malloc(size: usize) -> *mut u8 {
    init::ensure_init();
    HEAP.allocate(size)
}

/// Free a block previously returned by `malloc`/`realloc`/`calloc`.
pub fn free(ptr: *mut u8) {
    if ptr.is_null() { return; }
    init::ensure_init();
    // We don't know the original layout, but allocate always uses 8-byte alignment.
    // Layout::from_size_align_unchecked is safe because all blocks are aligned to 8.
    let layout = unsafe { Layout::from_size_align_unchecked(1, 8) };
    HEAP.deallocate(ptr, layout);
}

/// Allocate zero-initialized memory for `nmemb` elements of `size` bytes each.
pub fn calloc(nmemb: usize, size: usize) -> *mut u8 {
    let total = nmemb.wrapping_mul(size);
    if total == 0 { return core::ptr::null_mut(); }
    let ptr = malloc(total);
    if ptr.is_null() { return core::ptr::null_mut(); }
    unsafe { core::ptr::write_bytes(ptr, 0, total); }
    ptr
}

/// Resize an allocation to `new_size` bytes, preserving content.
pub fn realloc(ptr: *mut u8, new_size: usize) -> *mut u8 {
    if ptr.is_null() { return malloc(new_size); }
    if new_size == 0 { free(ptr); return core::ptr::null_mut(); }
    init::ensure_init();
    let layout = unsafe { Layout::from_size_align_unchecked(1, 8) };
    unsafe { HEAP.realloc(ptr, layout, new_size) }
}

#[cfg(test)]
mod tests {
    use core::alloc::{GlobalAlloc, Layout};
    use super::freelist::{FreelistAllocator, ARENA_SIZE};
    use super::backend::test_backend::TestBackend;
    use core::ptr;

    fn test_allocator() -> FreelistAllocator<TestBackend> {
        FreelistAllocator::new_with(TestBackend)
    }

    #[test]
    fn test_malloc_free() {
        let alloc = test_allocator();
        let ptr = alloc.allocate(64);
        assert!(!ptr.is_null());
        alloc.deallocate(ptr, Layout::from_size_align(1, 8).unwrap());
    }

    #[test]
    fn test_malloc_zero() {
        let alloc = test_allocator();
        let ptr = alloc.allocate(0);
        assert!(ptr.is_null());
    }

    #[test]
    fn test_realloc_grow() {
        let alloc = test_allocator();
        unsafe {
            let ptr = alloc.allocate(16);
            assert!(!ptr.is_null());
            ptr::write(ptr, 42u8);
            let new_ptr = alloc.realloc(
                ptr,
                Layout::from_size_align(1, 8).unwrap(),
                64,
            );
            assert!(!new_ptr.is_null());
            assert_eq!(ptr::read(new_ptr), 42u8);
            alloc.deallocate(new_ptr, Layout::from_size_align(1, 8).unwrap());
        }
    }

    #[test]
    fn test_many_small_allocs() {
        let alloc = test_allocator();
        let mut ptrs = [core::ptr::null_mut(); 100];
        unsafe {
            for i in 0..100 {
                ptrs[i] = alloc.allocate(8);
                assert!(!ptrs[i].is_null());
                ptr::write(ptrs[i], i as u8);
            }
            for i in 0..100 {
                assert_eq!(ptr::read(ptrs[i]), i as u8);
            }
            for i in 0..100 {
                alloc.deallocate(ptrs[i], Layout::from_size_align(1, 8).unwrap());
                ptrs[i] = core::ptr::null_mut();
            }
        }
    }

    #[test]
    fn test_large_alloc() {
        let alloc = test_allocator();
        let ptr = alloc.allocate(ARENA_SIZE / 2);
        assert!(!ptr.is_null());
        alloc.deallocate(ptr, Layout::from_size_align(1, 8).unwrap());
    }

    #[test]
    fn test_calloc() {
        let alloc = test_allocator();
        unsafe {
            let ptr = alloc.allocate(1024);
            assert!(!ptr.is_null());
            // Write non-zero
            ptr::write_bytes(ptr, 0xff, 1024);
            alloc.deallocate(ptr, Layout::from_size_align(1, 8).unwrap());

            // Allocate again (should get a zeroed or reused block)
            let ptr2 = alloc.allocate(1024);
            assert!(!ptr2.is_null());
            alloc.deallocate(ptr2, Layout::from_size_align(1, 8).unwrap());
        }
    }
}
