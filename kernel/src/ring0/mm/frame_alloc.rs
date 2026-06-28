//! Physical Frame Allocator — bitmap-based with next-free hint.
//!
//! Auto-detects RAM size from UEFI memory map.
//! Tracks 4 KiB physical frames from 16 MB up to detected limit.
//!
//! The bitmap is dynamically sized based on detected RAM:
//!   - 16 GB  → 512 KB bitmap
//!   - 64 GB  → 2 MB bitmap
//!   - 256 GB → 8 MB bitmap
//!   - 1 TB   → 32 MB bitmap
//!   - 4 TB   → 128 MB bitmap
//!
//! No static waste — bitmap only uses what's needed.
//! Scales to any RAM size the UEFI firmware reports.

use core::sync::atomic::{AtomicUsize, Ordering};
use fastos_boot_protocol::{MemoryEntry, MemoryType};

const PAGE_SIZE: u64 = super::PAGE_SIZE;
const BASE: u64 = 0x0100_0000;

/// Maximum address we can track. Set dynamically at init from memory map.
static mut MAX_ADDR: u64 = 0;

/// Maximum pages based on MAX_ADDR. Set dynamically at init.
static mut MAX_PAGES: usize = 0;

/// Pointer to the dynamically-sized bitmap.
/// Allocated from the frame allocator after detecting RAM size.
static mut BITMAP: *mut u8 = core::ptr::null_mut();

/// Number of bytes allocated for the bitmap.
static mut BITMAP_BYTES: usize = 0;

static mut INITIALIZED: bool = false;
static FREE_PAGES: AtomicUsize = AtomicUsize::new(0);
static mut NEXT_FREE_HINT: usize = 0;

/// Total RAM detected by UEFI (bytes). For reporting only.
static mut TOTAL_RAM: u64 = 0;

#[inline]
fn addr_to_index(addr: u64) -> Option<usize> {
    if addr < BASE || addr >= unsafe { MAX_ADDR } { return None; }
    Some(((addr - BASE) / PAGE_SIZE) as usize)
}

#[inline]
fn index_to_addr(idx: usize) -> u64 {
    BASE + (idx as u64) * PAGE_SIZE
}

#[inline]
unsafe fn is_used(idx: usize) -> bool {
    (*BITMAP.add(idx / 8) & (1 << (idx % 8))) != 0
}

#[inline]
unsafe fn mark_used(idx: usize) {
    *BITMAP.add(idx / 8) |= 1 << (idx % 8);
}

#[inline]
unsafe fn mark_free(idx: usize) {
    *BITMAP.add(idx / 8) &= !(1 << (idx % 8));
}

#[inline]
fn ranges_overlap(a_start: u64, a_end: u64, b_start: u64, b_end: u64) -> bool {
    a_start < b_end && a_end > b_start
}

struct RamInfo {
    total_usable: u64,
    max_usable: u64,
}

fn detect_ram(memory_map: &[MemoryEntry]) -> RamInfo {
    let mut total: u64 = 0;
    let mut max_usable: u64 = 0;

    for entry in memory_map {
        if entry.mem_type == MemoryType::Usable {
            total += entry.size;
            let end = entry.base + entry.size;
            if end > max_usable {
                max_usable = end;
            }
        }
    }

    RamInfo { total_usable: total, max_usable }
}

pub unsafe fn init(
    memory_map: &[MemoryEntry],
    count: usize,
    reserved_addr: u64,
    reserved_size: u64,
    kernel_base: u64,
    kernel_size: u64,
) {
    if INITIALIZED { return; }

    let entries = &memory_map[..count];

    let ram = detect_ram(entries);
    TOTAL_RAM = ram.total_usable;

    // Track ALL detected RAM — no identity-mapping limit.
    MAX_ADDR = ram.max_usable;
    MAX_PAGES = ((MAX_ADDR - BASE) / PAGE_SIZE) as usize;

    // Dynamically size the bitmap based on detected RAM.
    // This wastes nothing: 16 GB → 512 KB, 1 TB → 32 MB, 4 TB → 128 MB.
    let bitmap_bytes = (MAX_PAGES + 7) / 8;
    let bitmap_pages = (bitmap_bytes + PAGE_SIZE as usize - 1) / PAGE_SIZE as usize;

    let usable_mb = ram.total_usable / (1024 * 1024);
    let tracked_mb = (MAX_ADDR - BASE) / (1024 * 1024);

    crate::dev::console::serial_write("[frame_alloc] RAM detected=");
    crate::dev::console::serial_write_u64(usable_mb, 10);
    crate::dev::console::serial_write(" MB, tracking=");
    crate::dev::console::serial_write_u64(tracked_mb, 10);
    crate::dev::console::serial_write(" MB (");
    crate::dev::console::serial_write_u64(MAX_PAGES as u64, 10);
    crate::dev::console::serial_write(" pages, bitmap=");
    crate::dev::console::serial_write_u64(bitmap_bytes as u64 / 1024, 10);
    crate::dev::console::serial_write(" KB)\n");

    // Bootstrap: find a free region in the UEFI map for the bitmap.
    // We scan the map to find a contiguous region of free pages large
    // enough to hold the bitmap. This happens BEFORE the bitmap exists.
    let mut bitmap_phys: u64 = 0;
    for entry in entries {
        if entry.mem_type != MemoryType::Usable { continue; }
        let region_start = entry.base.max(BASE);
        let region_end = entry.base + entry.size;
        let region_pages = ((region_end - region_start) / PAGE_SIZE) as usize;
        if region_pages >= bitmap_pages {
            // Found a region. Use the first `bitmap_pages` pages for the bitmap.
            bitmap_phys = ((region_start + PAGE_SIZE - 1) / PAGE_SIZE) * PAGE_SIZE;
            break;
        }
    }

    if bitmap_phys == 0 {
        crate::dev::console::serial_write("[frame_alloc] FATAL: cannot allocate bitmap (");
        crate::dev::console::serial_write_u64(bitmap_pages as u64, 10);
        crate::dev::console::serial_write(" pages needed)\n");
        return;
    }

    // The bitmap is identity-mapped at low addresses (< 4 GB) because
    // we search the UEFI map which is always in low memory.
    BITMAP = bitmap_phys as *mut u8;
    BITMAP_BYTES = bitmap_bytes;

    crate::dev::console::serial_write("[frame_alloc] bitmap at phys=0x");
    crate::serial::hex(bitmap_phys);
    crate::dev::console::serial_write(" (");
    crate::dev::console::serial_write_u64(bitmap_pages as u64, 10);
    crate::dev::console::serial_write(" pages)\n");

    // Initialize bitmap: mark all pages as used (pessimistic)
    core::ptr::write_bytes(BITMAP, 0xFF, bitmap_bytes);

    // Free usable pages from memory map
    for entry in entries {
        if entry.mem_type != MemoryType::Usable { continue; }

        let region_start = entry.base;
        let region_end = entry.base + entry.size;
        let start = region_start.max(BASE);
        let end = region_end.min(MAX_ADDR);
        if start >= end { continue; }

        let first_page = ((start + PAGE_SIZE - 1) / PAGE_SIZE) * PAGE_SIZE;
        let last_page_end = (end / PAGE_SIZE) * PAGE_SIZE;
        if first_page >= last_page_end { continue; }

        let mut addr = first_page;
        while addr < last_page_end {
            if ranges_overlap(addr, addr + PAGE_SIZE, kernel_base, kernel_base + kernel_size) {
                addr += PAGE_SIZE;
                continue;
            }
            if reserved_size > 0 && ranges_overlap(addr, addr + PAGE_SIZE, reserved_addr, reserved_addr + reserved_size) {
                addr += PAGE_SIZE;
                continue;
            }
            // Don't mark the bitmap's own pages as free
            if ranges_overlap(addr, addr + PAGE_SIZE, bitmap_phys, bitmap_phys + bitmap_bytes as u64) {
                addr += PAGE_SIZE;
                continue;
            }
            // Keep the crash marker page reserved. The bootloader reads
            // physical 0x90000 on the next boot to write crash.log on the SSD.
            // If the allocator hands this page out, later zeroing/copying can
            // erase the only useful post-reset breadcrumb.
            if ranges_overlap(addr, addr + PAGE_SIZE, 0x9_0000, 0x9_1000) {
                addr += PAGE_SIZE;
                continue;
            }
            if let Some(idx) = addr_to_index(addr) {
                if is_used(idx) {
                    mark_free(idx);
                    FREE_PAGES.fetch_add(1, Ordering::Relaxed);
                }
            }
            addr += PAGE_SIZE;
        }
    }

    INITIALIZED = true;
    let free = FREE_PAGES.load(Ordering::Relaxed);
    let free_mb = (free as u64 * PAGE_SIZE) / (1024 * 1024);
    crate::dev::console::serial_write("[frame_alloc] free=");
    crate::dev::console::serial_write_u64(free as u64, 10);
    crate::dev::console::serial_write(" (");
    crate::dev::console::serial_write_u64(free_mb, 10);
    crate::dev::console::serial_write(" MB)\n");
}

pub unsafe fn alloc_pages_contiguous(count: usize) -> Option<u64> {
    if !INITIALIZED || count == 0 { return None; }

    let max = MAX_PAGES;
    let hint = NEXT_FREE_HINT;
    let mut run_len: usize = 0;
    let mut run_start: usize = 0;

    for idx in hint..max {
        if is_used(idx) {
            run_start = idx + 1;
            run_len = 0;
        } else {
            if run_len == 0 { run_start = idx; }
            run_len += 1;
            if run_len == count {
                for i in run_start..run_start + count { mark_used(i); }
                FREE_PAGES.fetch_sub(count, Ordering::Relaxed);
                NEXT_FREE_HINT = run_start + count;
                cabina_daemon::telemetry::memory::inc_allocs();
                cabina_daemon::telemetry::memory::set_free_pages(FREE_PAGES.load(Ordering::Relaxed) as u64);
                return Some(index_to_addr(run_start));
            }
        }
    }

    for idx in 0..hint {
        if is_used(idx) {
            run_start = idx + 1;
            run_len = 0;
        } else {
            if run_len == 0 { run_start = idx; }
            run_len += 1;
            if run_len == count {
                for i in run_start..run_start + count { mark_used(i); }
                FREE_PAGES.fetch_sub(count, Ordering::Relaxed);
                NEXT_FREE_HINT = run_start + count;
                cabina_daemon::telemetry::memory::inc_allocs();
                cabina_daemon::telemetry::memory::set_free_pages(FREE_PAGES.load(Ordering::Relaxed) as u64);
                return Some(index_to_addr(run_start));
            }
        }
    }

    None
}

pub unsafe fn free_pages(addr: u64, count: usize) {
    if !INITIALIZED || count == 0 { return; }

    debug_assert!(addr % PAGE_SIZE == 0, "free_pages: unaligned addr {:#x}", addr);

    let start_idx = match addr_to_index(addr) {
        Some(i) => i,
        None => return,
    };
    let end_idx = core::cmp::min(start_idx + count, MAX_PAGES);

    for idx in start_idx..end_idx {
        debug_assert!(is_used(idx), "free_pages: double-free at idx {}", idx);
        if is_used(idx) {
            mark_free(idx);
            FREE_PAGES.fetch_add(1, Ordering::Relaxed);
        }
    }
    cabina_daemon::telemetry::memory::inc_frees();
    cabina_daemon::telemetry::memory::set_free_pages(FREE_PAGES.load(Ordering::Relaxed) as u64);
}

pub fn free_count() -> usize {
    FREE_PAGES.load(Ordering::Relaxed)
}

pub fn total_ram() -> u64 {
    unsafe { TOTAL_RAM }
}

pub fn tracked_pages() -> usize {
    unsafe { MAX_PAGES }
}

pub const fn page_size() -> usize {
    PAGE_SIZE as usize
}
