#![allow(dead_code)]

//! Page Frame Allocator — bitmap-based, supports contiguous allocation.
//!
//! Used by the kernel for physical pages and optional boot-reserved payloads.
//!
//! Design:
//!   - Tracks physical pages from 16 MB (`BASE_ADDR`) to 4 GB (`MAX_ADDR`).
//!   - ~1 M pages → 128 KB static bitmap (1 bit per 4 KiB page).
//!   - Bit = 1 → page is **used**; bit = 0 → page is **free**.
//!   - Bitmap starts fully "used"; `init()` marks usable regions free.
//!   - Identity mapping assumed (phys == virt for first 4 GB).

use fastos_boot_protocol::{MemoryEntry, MemoryType};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const PAGE_SIZE: usize = 4096;
const BASE_ADDR: u64 = 0x0100_0000; // 16 MB — below this is legacy / kernel
const MAX_ADDR: u64 = 0x1_0000_0000; // 4 GB
const MAX_PAGES: usize = ((MAX_ADDR - BASE_ADDR) / PAGE_SIZE as u64) as usize; // ~1 M
const BITMAP_SIZE: usize = MAX_PAGES / 8; // ~128 KB





// ---------------------------------------------------------------------------
// Static state (single-core, no preemption during init — safe with `unsafe`)
// ---------------------------------------------------------------------------

/// Every bit starts as 1 (used). `init()` clears bits for usable RAM.
static mut BITMAP: [u8; BITMAP_SIZE] = [0xFF; BITMAP_SIZE];
static mut INITIALIZED: bool = false;
static mut FREE_PAGES_COUNT: usize = 0;

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Convert a physical address to a page index relative to `BASE_ADDR`.
/// Returns `None` if the address is out of the tracked range.
#[inline]
fn addr_to_index(addr: u64) -> Option<usize> {
    if addr < BASE_ADDR || addr >= MAX_ADDR {
        return None;
    }
    Some(((addr - BASE_ADDR) / PAGE_SIZE as u64) as usize)
}

/// Convert a page index back to a physical address.
#[inline]
fn index_to_addr(idx: usize) -> u64 {
    BASE_ADDR + (idx as u64) * (PAGE_SIZE as u64)
}

/// Check whether page `idx` is used (bit == 1).
#[inline]
unsafe fn is_used(idx: usize) -> bool {
    let byte = idx / 8;
    let bit = idx % 8;
    (BITMAP[byte] & (1 << bit)) != 0
}

/// Mark page `idx` as used (set bit to 1).
#[inline]
unsafe fn mark_used(idx: usize) {
    let byte = idx / 8;
    let bit = idx % 8;
    BITMAP[byte] |= 1 << bit;
}

/// Mark page `idx` as free (clear bit to 0).
#[inline]
unsafe fn mark_free(idx: usize) {
    let byte = idx / 8;
    let bit = idx % 8;
    BITMAP[byte] &= !(1 << bit);
}

/// Returns `true` if the physical range `[start, end)` overlaps with
/// `[region_start, region_end)`.
#[inline]
fn ranges_overlap(start: u64, end: u64, region_start: u64, region_end: u64) -> bool {
    start < region_end && end > region_start
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Initialise the page-frame allocator from the boot-provided memory map.
///
/// `memory_map` — pointer to the array of `MemoryEntry` structs.
/// `count`      — number of entries in the array.
///
/// # Safety
/// Must be called exactly once, early in kernel init, before any allocation.
pub unsafe fn init(
    memory_map: &[MemoryEntry],
    count: usize,
    reserved_addr: u64,
    reserved_size: u64,
    kernel_base: u64,
    kernel_size: u64,
) {
    if INITIALIZED {
        return;
    }

    // Bitmap is already all-ones (every page marked used).
    // Walk the memory map and free pages that belong to Usable regions,
    // *except* those that overlap the kernel reservation.

    let entries = &memory_map[..count];

    for entry in entries {
        // Only consider usable RAM.
        if entry.mem_type != MemoryType::Usable {
            continue;
        }

        let region_start = entry.base;
        let region_end = entry.base + entry.size;

        // Clamp to our tracked window [BASE_ADDR, MAX_ADDR).
        let start = if region_start < BASE_ADDR {
            BASE_ADDR
        } else {
            region_start
        };
        let end = if region_end > MAX_ADDR {
            MAX_ADDR
        } else {
            region_end
        };

        if start >= end {
            continue;
        }

        // Page-align inward.
        let first_page = ((start + PAGE_SIZE as u64 - 1) / PAGE_SIZE as u64) * PAGE_SIZE as u64;
        let last_page_end = (end / PAGE_SIZE as u64) * PAGE_SIZE as u64;

        if first_page >= last_page_end {
            continue;
        }

        let mut addr = first_page;
        while addr < last_page_end {
            // Skip pages that fall inside the kernel region.
            if ranges_overlap(addr, addr + PAGE_SIZE as u64, kernel_base, kernel_base + kernel_size) {
                addr += PAGE_SIZE as u64;
                continue;
            }

            // Skip pages that belong to an optional boot-reserved payload.
            if reserved_size > 0 && ranges_overlap(addr, addr + PAGE_SIZE as u64, reserved_addr, reserved_addr + reserved_size) {
                addr += PAGE_SIZE as u64;
                continue;
            }

            if let Some(idx) = addr_to_index(addr) {
                if is_used(idx) {
                    mark_free(idx);
                    FREE_PAGES_COUNT += 1;
                }
            }
            addr += PAGE_SIZE as u64;
        }
    }

    INITIALIZED = true;
}

/// Allocate `count` physically-contiguous pages.
///
/// Returns the **physical base address** of the first page, or `None` if no
/// contiguous run of that length is available.
///
/// The returned memory is identity-mapped (phys == virt) under FastOS's
/// first-4 GB identity map, so the caller can use the address directly.
pub unsafe fn alloc_pages_contiguous(count: usize) -> Option<u64> {
    if !INITIALIZED || count == 0 {
        return None;
    }

    let mut run_start: usize = 0;
    let mut run_len: usize = 0;

    for idx in 0..MAX_PAGES {
        if is_used(idx) {
            // Reset the run.
            run_start = idx + 1;
            run_len = 0;
        } else {
            if run_len == 0 {
                run_start = idx;
            }
            run_len += 1;

            if run_len == count {
                // Found a large-enough contiguous run — mark all pages used.
                for i in run_start..run_start + count {
                    mark_used(i);
                }
                FREE_PAGES_COUNT -= count;
                return Some(index_to_addr(run_start));
            }
        }
    }

    None // Not enough contiguous free pages.
}

/// Free `count` pages starting at physical address `addr`.
///
/// # Safety
/// The caller must ensure the pages were previously allocated by this
/// allocator and are no longer in use.
pub unsafe fn free_pages(addr: u64, count: usize) {
    if !INITIALIZED || count == 0 {
        return;
    }

    let start_idx = match addr_to_index(addr) {
        Some(i) => i,
        None => return,
    };

    let end_idx = core::cmp::min(start_idx + count, MAX_PAGES);

    for idx in start_idx..end_idx {
        if is_used(idx) {
            mark_free(idx);
            FREE_PAGES_COUNT += 1;
        }
    }
}

/// Return the current number of free pages tracked by the allocator.
pub unsafe fn free_count() -> usize {
    FREE_PAGES_COUNT
}

/// Return the total number of pages the allocator can track.
pub const fn total_pages() -> usize {
    MAX_PAGES
}

/// Return the page size in bytes.
pub const fn page_size() -> usize {
    PAGE_SIZE
}
