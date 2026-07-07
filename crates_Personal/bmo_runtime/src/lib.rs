//! `bmo_runtime` — the BMO runtime: heap allocator + FFI exports.
//!
//! Provides `malloc`/`free`/`realloc`/`calloc` with a user-space free-list
//! allocator that requests large arenas from the kernel via `bmo_mem_alloc`.
//! Can be used from Rust (via the `alloc` crate with `#[global_allocator]`)
//! or from C/COBOL (via `extern "C"` FFI exports).

#![no_std]

#[cfg(test)]
extern crate std;

pub mod heap;
pub mod ffi;

mod init;

/// Heap statistics for diagnostics.
#[derive(Debug, Clone, Copy, Default)]
pub struct HeapStats {
    pub arena_count: usize,
    pub total_allocated: usize,
    pub total_free: usize,
    pub free_blocks: usize,
}

/// Re-export convenience functions.
pub use heap::malloc;
pub use heap::free;
pub use heap::realloc;
pub use heap::calloc;
