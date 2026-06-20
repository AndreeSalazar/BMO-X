//! ÑEXO std::mem — Gestión de memoria.

#![allow(dead_code)]

extern crate alloc;
use alloc::alloc::{alloc as kalloc, Layout};

/// Reservar memoria del pool.
pub fn alloc(size: usize) -> Option<*mut u8> {
    let layout = Layout::from_size_align(size, 16).ok()?;
    let ptr = unsafe { kalloc(layout) };
    if ptr.is_null() { None } else { Some(ptr) }
}

/// Liberar memoria (placeholder — bump allocator no libera).
pub fn free(_ptr: *mut u8) {}

/// Copiar memoria.
pub fn copy(dst: *mut u8, src: *const u8, len: usize) {
    unsafe { let mut i = 0; while i < len { *dst.add(i) = *src.add(i); i += 1; } }
}

/// Rellenar memoria con valor.
pub fn fill(dst: *mut u8, val: u8, len: usize) {
    unsafe { let mut i = 0; while i < len { *dst.add(i) = val; i += 1; } }
}

/// Comparar memoria.
pub fn equal(a: *const u8, b: *const u8, len: usize) -> bool {
    unsafe { let mut i = 0; while i < len { if *a.add(i) != *b.add(i) { return false; } i += 1; } true }
}

/// Bytes usados del heap.
pub fn heap_used() -> usize { crate::allocator::heap_used() }

/// Tamaño total del heap.
pub fn heap_total() -> usize { crate::allocator::heap_total() }
