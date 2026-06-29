//! LLFree adapter — Lock-free backing allocator via llfree crate.
//!
//! Wraps `llfree::LLFree` into the `BackingAllocator` trait.
//! Not active by default; enable with feature `alloc-llfree`.
//!
//! LLFree is a two-level allocator:
//!   - Lower level: bitfield-based per-page metadata (4 KiB .. 4 MiB)
//!   - Upper level: tree-based locality classes with per-CPU reservations
//!
//! Reference: Wrenger et al., USENIX ATC '23

use core::sync::atomic::AtomicUsize;
use fastos_boot_protocol::{MemoryEntry, MemoryType};
use super::PAGE_SIZE;
use super::MAX_ORDER;
use super::BackingAllocator;

use llfree::LLFree;
use llfree::Alloc;
use llfree::FrameId;
use llfree::Init;
use llfree::Classing;
use llfree::MetaData;
use llfree::Request;

const BASE: u64 = 0x0100_0000;

/// LLFree backing allocator.
/// Metadata buffers are allocated from the first usable UEFI region.
pub struct LlfreeAllocator;

unsafe impl Sync for LlfreeAllocator {}

impl BackingAllocator for LlfreeAllocator {
    unsafe fn init(&self, memory_map: &[MemoryEntry], count: usize,
                   _reserved_addr: u64, _reserved_size: u64,
                   _kernel_base: u64, _kernel_size: u64) {
        let entries = &memory_map[..count];

        // Count total frames.
        let mut max_usable: u64 = 0;
        for e in entries {
            if e.mem_type == MemoryType::Usable {
                let end = e.base + e.size;
                if end > max_usable { max_usable = end; }
            }
        }
        let total_frames = ((max_usable - BASE) / PAGE_SIZE) as usize;

        // Build classing with 1 core (BSP; SMP later adds more).
        let (classing, _request) = Classing::simple(1);

        // Calculate metadata size.
        let ms = LLFree::metadata_size(&classing, total_frames);

        // Allocate metadata buffers from the first large enough usable region.
        // TODO: proper metadata placement, similar to buddy's bootstrap logic.

        let _meta = MetaData {
            local: &mut [],   // TODO: allocate from UEFI region
            trees: &mut [],
            lower: &mut [],
        };

        // Initialize the allocator marking all frames as free.
        // let _alloc = LLFree::new(total_frames, Init::FreeAll, &classing, _meta);
        // crate::dev::console::serial_write("[llfree] init complete\n");
    }

    unsafe fn alloc_order(&self, order: usize) -> Option<u64> {
        // TODO: route through LLFree::get() with proper Request
        // For now: stub falls back to nothing
        let _ = order;
        None
    }

    unsafe fn free_order(&self, _addr: u64, _order: usize) {
        // TODO: route through LLFree::put()
    }

    fn free_count(&self) -> usize {
        0 // TODO: query LLFree
    }

    fn total_ram(&self) -> u64 {
        0 // TODO: track from init
    }

    fn tracked_pages(&self) -> usize {
        0
    }
}
