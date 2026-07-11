//! Buddy System â€” Backing allocator implementation.
//!
//! O(log n) allocation and coalescing via power-of-2 free lists.
//! Orders 0..MAX_ORDER: 2^k Ã— 4 KiB â†’ 4 KiB .. 8 MiB.
//!
//! Metadata is a u8-per-physical-page array sized at init from UEFI map.
//! Free blocks store their linked-list pointers within the block itself.

use core::ptr;
use core::sync::atomic::{AtomicUsize, Ordering};
use bmo_boot_protocol::{MemoryEntry, MemoryType};
use super::PAGE_SIZE;
use super::MAX_ORDER;
use super::BackingAllocator;

const BASE: u64 = 0x0100_0000;
const ORDER_FREE: u8 = 0;
const ORDER_RSVD: u8 = 0xFF;

static mut PAGE_ORDERS: *mut u8 = ptr::null_mut();
static mut PAGE_COUNT: usize = 0;
static mut FREE_LISTS: [u64; MAX_ORDER + 1] = [0; MAX_ORDER + 1];
static INITIALIZED: AtomicUsize = AtomicUsize::new(0);
static FREE_COUNT: AtomicUsize = AtomicUsize::new(0);
static mut TOTAL_RAM: u64 = 0;

/// Convert a physical address (â‰¥ BASE) to a page index.
#[inline]
fn addr_to_idx(addr: u64) -> Option<usize> {
    if addr < BASE { return None; }
    let idx = ((addr - BASE) / PAGE_SIZE) as usize;
    if idx >= unsafe { PAGE_COUNT } { return None; }
    Some(idx)
}

/// Convert a page index to a physical address.
#[inline]
fn idx_to_addr(idx: usize) -> u64 {
    BASE + (idx as u64) * PAGE_SIZE
}

/// Check if a page is currently tracked as allocated.
#[allow(dead_code)]
#[inline]
unsafe fn is_used(addr: u64) -> bool {
    match addr_to_idx(addr) {
        Some(idx) => *PAGE_ORDERS.add(idx) != ORDER_FREE,
        None => true,
    }
}

/// Return the order (k) such that 2^k pages starting at `addr` are all
/// free AND aligned-to-order, or None if not all free/not aligned.
unsafe fn coalescable(addr: u64, order: usize) -> bool {
    let idx = match addr_to_idx(addr) {
        Some(i) => i,
        None => return false,
    };
    let len = 1usize << order;
    if idx & (len - 1) != 0 { return false; }
    if idx + len > unsafe { PAGE_COUNT } { return false; }
    for i in idx..idx + len {
        if *PAGE_ORDERS.add(i) != ORDER_FREE { return false; }
    }
    true
}

/// Mark a range of pages with a given order (called after allocating).
/// Returns false if addr is outside tracked range (callers should handle).
unsafe fn set_allocated(addr: u64, order: usize) -> bool {
    let idx = match addr_to_idx(addr) {
        Some(i) => i,
        None => return false,
    };
    let len = 1usize << order;
    for i in idx..idx + len {
        *PAGE_ORDERS.add(i) = order as u8;
    }
    true
}

/// Mark a range of pages as free.
/// Returns false if addr is outside tracked range.
unsafe fn set_free(addr: u64, order: usize) -> bool {
    let idx = match addr_to_idx(addr) {
        Some(i) => i,
        None => return false,
    };
    let len = 1usize << order;
    for i in idx..idx + len {
        *PAGE_ORDERS.add(i) = ORDER_FREE;
    }
    true
}

/// Pop a block from the free list at `order`. Returns physical address or 0.
unsafe fn list_pop(order: usize) -> u64 {
    let head = FREE_LISTS[order];
    if head == 0 { return 0; }
    let next = *(head as *const u64);
    FREE_LISTS[order] = next;
    head
}

/// Push a block onto the free list at `order`. `addr` must be order-aligned.
unsafe fn list_push(addr: u64, order: usize) {
    *(addr as *mut u64) = FREE_LISTS[order];
    FREE_LISTS[order] = addr;
}

/// Allocate 2^order contiguous physical pages from the buddy system.
///
/// # Safety
/// - `order` must be <= MAX_ORDER (11)
/// - Returned address is a physical address in the tracked range
/// - The returned pages are not aliased until buddy_free() is called
/// - Each page is PAGE_SIZE (4096) bytes
unsafe fn buddy_alloc(order: usize) -> Option<u64> {
    if order > MAX_ORDER { return None; }
    let mut o = order;
    while o <= MAX_ORDER && FREE_LISTS[o] == 0 {
        o += 1;
    }
    if o > MAX_ORDER { return None; }
    let block = list_pop(o);
    if block == 0 { return None; }
    while o > order {
        o -= 1;
        let half_size = 1u64 << o;
        let upper = block + half_size * PAGE_SIZE;
        list_push(upper, o);
    }
    if !set_allocated(block, order) { return None; }
    FREE_COUNT.fetch_sub(1usize << order, Ordering::Relaxed);
    Some(block)
}

/// Free a block of 2^order pages starting at `addr`, coalescing with buddy.
///
/// # Safety
/// - `addr` must have been returned by buddy_alloc() with the same `order`
/// - `addr` must be non-zero and order-aligned
/// - Double-free is undefined behavior (may corrupt free lists)
/// - After this call, the pages are available for reallocation
unsafe fn buddy_free(addr: u64, order: usize) {
    if order > MAX_ORDER || addr == 0 { return; }
    let mut o = order;
    let mut block = addr;
    set_free(block, o);
    while o < MAX_ORDER {
        let block_idx = match addr_to_idx(block) {
            Some(i) => i,
            None => break, // address outside tracked range
        };
        let buddy_idx = block_idx ^ (1usize << o);
        let buddy_addr = idx_to_addr(buddy_idx);
        if buddy_addr + (1u64 << o) * PAGE_SIZE > BASE + (PAGE_COUNT as u64) * PAGE_SIZE {
            break;
        }
        if !coalescable(buddy_addr, o) { break; }
        list_remove(buddy_addr, o);
        set_free(buddy_addr, o);
        if buddy_addr < block { block = buddy_addr; }
        o += 1;
    }
    set_free(block, o);
    list_push(block, o);
    FREE_COUNT.fetch_add(1usize << o, Ordering::Relaxed);
}

unsafe fn list_remove(target: u64, order: usize) {
    let head = FREE_LISTS[order];
    if head == 0 { return; }
    if head == target {
        FREE_LISTS[order] = *(head as *const u64);
        return;
    }
    let mut prev = head;
    loop {
        let curr = *(prev as *const u64);
        if curr == 0 { break; }
        if curr == target {
            *(prev as *mut u64) = *(curr as *const u64);
            break;
        }
        prev = curr;
    }
}

fn ranges_overlap(a_start: u64, a_end: u64, b_start: u64, b_end: u64) -> bool {
    a_start < b_end && a_end > b_start
}

unsafe fn buddy_free_page(addr: u64) {
    let mut o = 0;
    let mut block = addr;
    set_free(block, o);
    while o < MAX_ORDER {
        let block_idx = match addr_to_idx(block) {
            Some(i) => i,
            None => break,
        };
        let buddy_idx = block_idx ^ (1usize << o);
        let buddy_addr = idx_to_addr(buddy_idx);
        if buddy_addr + (1u64 << o) * PAGE_SIZE > BASE + (PAGE_COUNT as u64) * PAGE_SIZE {
            break;
        }
        if !coalescable(buddy_addr, o) { break; }
        list_remove(buddy_addr, o);
        set_free(buddy_addr, o);
        if buddy_addr < block { block = buddy_addr; }
        o += 1;
    }
    set_free(block, o);
    list_push(block, o);
    FREE_COUNT.fetch_add(1usize << o, Ordering::Relaxed);
}

/// Buddy system backing allocator (unit struct, statics hold state).
pub struct BuddyAllocator;

unsafe impl Sync for BuddyAllocator {}

impl BackingAllocator for BuddyAllocator {
    unsafe fn init(&self, memory_map: &[MemoryEntry], count: usize,
                   reserved_addr: u64, reserved_size: u64,
                   kernel_base: u64, kernel_size: u64) {
        if INITIALIZED.load(Ordering::Relaxed) != 0 { return; }
        INITIALIZED.store(1, Ordering::Relaxed);

        let entries = &memory_map[..count];
        let mut total_usable: u64 = 0;
        let mut max_usable: u64 = 0;
        for e in entries {
            if e.mem_type == MemoryType::Usable {
                total_usable += e.size;
                let end = e.base + e.size;
                if end > max_usable { max_usable = end; }
            }
        }
        TOTAL_RAM = total_usable;

        PAGE_COUNT = ((max_usable - BASE) / PAGE_SIZE) as usize;
        let meta_bytes = PAGE_COUNT;
        let meta_pages = (meta_bytes + PAGE_SIZE as usize - 1) / PAGE_SIZE as usize;

        const HUGE_2MB: u64 = 2 * 1024 * 1024;
        let mut meta_phys: u64 = 0;
        for e in entries {
            if e.mem_type != MemoryType::Usable { continue; }
            let region_start = (e.base.max(BASE) + HUGE_2MB - 1) & !(HUGE_2MB - 1);
            let region_end = (e.base + e.size) & !(HUGE_2MB - 1);
            if region_start >= region_end { continue; }
            let avail = ((region_end - region_start) / PAGE_SIZE) as usize;
            if avail >= meta_pages {
                meta_phys = region_start;
                break;
            }
        }
        if meta_phys == 0 {
            crate::dev::console::serial_write("[buddy] FATAL: cannot allocate metadata\n");
            return;
        }

        PAGE_ORDERS = meta_phys as *mut u8;
        core::ptr::write_bytes(PAGE_ORDERS, ORDER_RSVD, meta_bytes);

        for i in 0..meta_pages {
            let p = meta_phys + (i as u64) * PAGE_SIZE;
            if let Some(idx) = addr_to_idx(p) {
                *PAGE_ORDERS.add(idx) = ORDER_RSVD;
            }
        }
        if let Some(idx) = addr_to_idx(0x9_0000) {
            *PAGE_ORDERS.add(idx) = ORDER_RSVD;
        }

        for e in entries {
            if e.mem_type != MemoryType::Usable { continue; }
            let region_start = e.base.max(BASE);
            // Cap at 2 GB (0x8000_0000) for the initial low-memory bootstrap phase
            let region_end = (e.base + e.size).min(0x8000_0000);
            if region_start >= region_end { continue; }
            
            // Align start and end to 2 MiB to match high-mem page mapping
            let start_page = (region_start + HUGE_2MB - 1) & !(HUGE_2MB - 1);
            let end_page = region_end & !(HUGE_2MB - 1);
            if start_page >= end_page { continue; }

            let mut addr = start_page;
            while addr < end_page {
                let mut skip = false;
                if ranges_overlap(addr, addr + PAGE_SIZE, kernel_base, kernel_base + kernel_size) { skip = true; }
                if reserved_size > 0 && ranges_overlap(addr, addr + PAGE_SIZE, reserved_addr, reserved_addr + reserved_size) { skip = true; }
                if ranges_overlap(addr, addr + PAGE_SIZE, meta_phys, meta_phys + meta_bytes as u64) { skip = true; }
                if ranges_overlap(addr, addr + PAGE_SIZE, 0x9_0000, 0x9_1000) { skip = true; }
                if skip { addr += PAGE_SIZE; continue; }
                buddy_free_page(addr);
                addr += PAGE_SIZE;
            }
        }

        let free = FREE_COUNT.load(Ordering::Relaxed);
        let free_mb = (free as u64 * PAGE_SIZE) / (1024 * 1024);
        crate::dev::console::serial_write("[buddy] init: ");
        crate::dev::console::serial_write_u64(free as u64, 10);
        crate::dev::console::serial_write(" free pages (");
        crate::dev::console::serial_write_u64(free_mb, 10);
        crate::dev::console::serial_write(" MB), metadata=");
        crate::dev::console::serial_write_u64(meta_pages as u64, 10);
        crate::dev::console::serial_write(" pages\n");
    }

    unsafe fn free_high_memory(&self, memory_map: &[MemoryEntry], count: usize) {
        let entries = &memory_map[..count];
        const HUGE_2MB: u64 = 2 * 1024 * 1024;
        for e in entries {
            if e.mem_type != MemoryType::Usable { continue; }
            let region_start = e.base;
            let region_end = e.base + e.size;
            // Only free pages that are ABOVE 2 GB (since below 2 GB were already freed)
            if region_end <= 0x8000_0000 { continue; }
            let start = region_start.max(0x8000_0000);
            
            // Align start and end to 2 MiB to match high-mem page mapping
            let start_page = (start + HUGE_2MB - 1) & !(HUGE_2MB - 1);
            let end_page = region_end & !(HUGE_2MB - 1);
            if start_page >= end_page { continue; }

            let mut addr = start_page;
            while addr < end_page {
                buddy_free_page(addr);
                addr += PAGE_SIZE;
            }
        }
        
        let free = FREE_COUNT.load(Ordering::Relaxed);
        let free_mb = (free as u64 * PAGE_SIZE) / (1024 * 1024);
        crate::dev::console::serial_write("[buddy] post-high-mem free: ");
        crate::dev::console::serial_write_u64(free as u64, 10);
        crate::dev::console::serial_write(" free pages (");
        crate::dev::console::serial_write_u64(free_mb, 10);
        crate::dev::console::serial_write(" MB)\n");
    }

    unsafe fn alloc_order(&self, order: usize) -> Option<u64> {
        buddy_alloc(order)
    }

    unsafe fn free_order(&self, addr: u64, order: usize) {
        buddy_free(addr, order);
    }

    fn free_count(&self) -> usize {
        FREE_COUNT.load(Ordering::Relaxed)
    }

    fn total_ram(&self) -> u64 {
        unsafe { TOTAL_RAM }
    }

    fn tracked_pages(&self) -> usize {
        unsafe { PAGE_COUNT }
    }
}
