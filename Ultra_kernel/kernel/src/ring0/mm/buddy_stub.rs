//! Buddy allocator — stub for the Ring 0 base.
//!
//! The frame allocator in `mm::phys` is sufficient for the boot path.
//! A full buddy allocator with order-0..10 free lists will be added
//! when we need variable-size allocations.

pub fn init() {}
