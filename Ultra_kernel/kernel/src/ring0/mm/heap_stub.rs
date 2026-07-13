//! Kernel heap — stub for the Ring 0 base.
//!
//! The full heap (linked-list or slab-based) will be implemented
//! after the frame allocator is verified working.

pub fn init_heap() {}

pub fn heap_total() -> usize { 0 }
pub fn heap_used()  -> usize { 0 }
