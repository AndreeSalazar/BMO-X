//! `allocator` — BmoAllocator trait, interfaz de asignación del BMO ABI.
//!
//! Permite pluguear diferentes backends de memoria:
//! - Slab allocator (Ring 0, fast)
//! - Buddy system (páginas físicas)
//! - Pool allocator (objetos fijos)
//! - Heap de usuario (Ring 3)

use crate::bmo_abi::primitives::{bx_u64, bx_usize};
use crate::bmo_abi::fundamentals::status::BmoStatus;

/// Resultado de una asignación.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BmoAllocResult {
    pub status: BmoStatus,
    pub ptr: *mut u8,
}
const _: () = assert!(core::mem::size_of::<BmoAllocResult>() == 24);

/// Trait de asignación de memoria.
///
/// Reemplaza `malloc`/`free` de C. Sin TLS, sin errno, todo por valor.
pub trait BmoAllocator {
    /// Allocate `size` bytes with given alignment.
    fn alloc(&mut self, size: bx_usize, align: bx_usize) -> BmoAllocResult;

    /// Allocate zero-initialized memory.
    fn alloc_zeroed(&mut self, size: bx_usize, align: bx_usize) -> BmoAllocResult;

    /// Reallocate memory to a new size.
    fn realloc(&mut self, ptr: *mut u8, old_size: bx_usize, new_size: bx_usize, align: bx_usize) -> BmoAllocResult;

    /// Free allocated memory.
    fn free(&mut self, ptr: *mut u8, size: bx_usize, align: bx_usize) -> BmoStatus;

    /// Return total allocated bytes (for accounting).
    fn allocated(&self) -> bx_u64 { 0 }
}

// ─── BmoAllocator for `alloc::alloc::Global` ──────────────────────

/// Wrapper que implementa `BmoAllocator` sobre el global allocator de Rust.
pub struct BmoGlobalAllocator;

impl BmoAllocator for BmoGlobalAllocator {
    fn alloc(&mut self, size: bx_usize, align: bx_usize) -> BmoAllocResult {
        let layout = match core::alloc::Layout::from_size_align(size as usize, align as usize) {
            Ok(l) => l,
            Err(_) => return BmoAllocResult { status: BmoStatus::err(crate::bmo_abi::error_code::INVALID_ARGUMENT), ptr: core::ptr::null_mut() },
        };
        let ptr = unsafe { alloc::alloc::alloc(layout) };
        if ptr.is_null() {
            BmoAllocResult { status: BmoStatus::err(crate::bmo_abi::error_code::OUT_OF_MEMORY), ptr: core::ptr::null_mut() }
        } else {
            BmoAllocResult { status: BmoStatus::OK, ptr }
        }
    }

    fn alloc_zeroed(&mut self, size: bx_usize, align: bx_usize) -> BmoAllocResult {
        let layout = match core::alloc::Layout::from_size_align(size as usize, align as usize) {
            Ok(l) => l,
            Err(_) => return BmoAllocResult { status: BmoStatus::err(crate::bmo_abi::error_code::INVALID_ARGUMENT), ptr: core::ptr::null_mut() },
        };
        let ptr = unsafe { alloc::alloc::alloc_zeroed(layout) };
        if ptr.is_null() {
            BmoAllocResult { status: BmoStatus::err(crate::bmo_abi::error_code::OUT_OF_MEMORY), ptr: core::ptr::null_mut() }
        } else {
            BmoAllocResult { status: BmoStatus::OK, ptr }
        }
    }

    fn realloc(&mut self, ptr: *mut u8, old_size: bx_usize, new_size: bx_usize, align: bx_usize) -> BmoAllocResult {
        let layout = match core::alloc::Layout::from_size_align(old_size as usize, align as usize) {
            Ok(l) => l,
            Err(_) => return BmoAllocResult { status: BmoStatus::err(crate::bmo_abi::error_code::INVALID_ARGUMENT), ptr: core::ptr::null_mut() },
        };
        let new_ptr = unsafe { alloc::alloc::realloc(ptr, layout, new_size as usize) };
        if new_ptr.is_null() {
            BmoAllocResult { status: BmoStatus::err(crate::bmo_abi::error_code::OUT_OF_MEMORY), ptr: core::ptr::null_mut() }
        } else {
            BmoAllocResult { status: BmoStatus::OK, ptr: new_ptr }
        }
    }

    fn free(&mut self, ptr: *mut u8, size: bx_usize, align: bx_usize) -> BmoStatus {
        if ptr.is_null() { return BmoStatus::OK; }
        let layout = match core::alloc::Layout::from_size_align(size as usize, align as usize) {
            Ok(l) => l,
            Err(_) => return BmoStatus::err(crate::bmo_abi::error_code::INVALID_ARGUMENT),
        };
        unsafe { alloc::alloc::dealloc(ptr, layout); }
        BmoStatus::OK
    }
}
