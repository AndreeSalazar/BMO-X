//! Memory subsystem for FastOS.
//!
//! Modular memory management:
//!   - Page allocator (physical pages)
//!   - VMM (virtual memory manager — VMA management, CoW, demand paging)

pub mod vmm;
