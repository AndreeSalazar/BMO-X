//! Physical Frame Allocator — bitmap-based with next-free hint.
//!
//! Tracks 4 KiB physical frames from 16 MB to 4 GB.
//! ~1M pages → 128 KB static bitmap (1 bit per frame).
//! Bit = 1 → used; bit = 0 → free.
//! Identity mapping assumed (phys == virt for first 4 GB).

use core::sync::atomic::{AtomicUsize, Ordering};
use fastos_boot_protocol::{MemoryEntry, MemoryType};

const PAGE_SIZE: u64 = super::PAGE_SIZE;
const BASE: u64 = 0x0100_0000;
const MAX_ADDR: u64 = 0x1_0000_0000;
const MAX_PAGES: usize = ((MAX_ADDR - BASE) / PAGE_SIZE) as usize;
const BITMAP_SIZE: usize = MAX_PAGES / 8;

static mut BITMAP: [u8; BITMAP_SIZE] = [0; BITMAP_SIZE];
static mut INITIALIZED: bool = false;
static FREE_PAGES: AtomicUsize = AtomicUsize::new(0);
static mut NEXT_FREE_HINT: usize = 0;

#[inline]
fn addr_to_index(addr: u64) -> Option<usize> {
    if addr < BASE || addr >= MAX_ADDR { return None; }
    Some(((addr - BASE) / PAGE_SIZE) as usize)
}

#[inline]
fn index_to_addr(idx: usize) -> u64 {
    BASE + (idx as u64) * PAGE_SIZE
}

#[inline]
unsafe fn is_used(idx: usize) -> bool {
    (BITMAP[idx / 8] & (1 << (idx % 8))) != 0
}

#[inline]
unsafe fn mark_used(idx: usize) {
    BITMAP[idx / 8] |= 1 << (idx % 8);
}

#[inline]
unsafe fn mark_free(idx: usize) {
    BITMAP[idx / 8] &= !(1 << (idx % 8));
}

#[inline]
fn ranges_overlap(a_start: u64, a_end: u64, b_start: u64, b_end: u64) -> bool {
    a_start < b_end && a_end > b_start
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

    core::ptr::write_bytes(core::ptr::addr_of_mut!(BITMAP) as *mut u8, 0xFF, BITMAP_SIZE);

    let entries = &memory_map[..count];

    let mut usable_bytes: u64 = 0;
    for entry in entries {
        if entry.mem_type == MemoryType::Usable {
            usable_bytes += entry.size;
        }
    }

    crate::dev::console::serial_write("[frame_alloc] entries=");
    crate::dev::console::serial_write_u64(count as u64, 10);
    crate::dev::console::serial_write(" usable=");
    crate::dev::console::serial_write_u64(usable_bytes / (1024 * 1024), 10);
    crate::dev::console::serial_write(" MB\n");

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
    crate::dev::console::serial_write("[frame_alloc] free=");
    crate::dev::console::serial_write_u64(free as u64, 10);
    crate::dev::console::serial_write(" (");
    crate::dev::console::serial_write_u64((free * 4096 / (1024 * 1024)) as u64, 10);
    crate::dev::console::serial_write(" MB)\n");
}

pub unsafe fn alloc_pages_contiguous(count: usize) -> Option<u64> {
    if !INITIALIZED || count == 0 { return None; }

    let hint = NEXT_FREE_HINT;
    let mut run_len: usize = 0;
    let mut run_start: usize = 0;

    for idx in hint..MAX_PAGES {
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
}

pub fn free_count() -> usize {
    FREE_PAGES.load(Ordering::Relaxed)
}

pub const fn total_pages() -> usize {
    MAX_PAGES
}

pub const fn page_size() -> usize {
    PAGE_SIZE as usize
}
