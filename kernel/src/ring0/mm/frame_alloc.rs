//! Physical Frame Allocator — Buddy System.
//!
//! O(log n) allocation and coalescing via power-of-2 free lists.
//! Orders 0..MAX_ORDER: 2^k × 4 KiB → 4 KiB .. 8 MiB.
//!
//! Metadata is a u8-per-physical-page array sized at init from UEFI map.
//! Free blocks store their linked-list pointers within the block itself
//! (valid since the block is unused).
//!
//! Public API is identical to the original bitmap allocator:
//! no consumers need changes.

use core::ptr;
use core::sync::atomic::{AtomicUsize, Ordering};
use fastos_boot_protocol::{MemoryEntry, MemoryType};

const PAGE_SIZE: u64 = super::PAGE_SIZE;
const BASE: u64 = 0x0100_0000; // start tracking at 16 MB
const MAX_ORDER: usize = 11;   // 2^11 × 4 KiB = 8 MiB

// Order constants stored in PAGE_ORDERS[]
const ORDER_FREE: u8 = 0;
const ORDER_RSVD: u8 = 0xFF; // reserved (never freed)

/// One byte per physical page frame:
///   0          = free (not in any free list yet)
///   1..MAX_ORDER = allocated at that order
///   0xFF       = reserved (kernel, bitmap, crash marker, etc.)
static mut PAGE_ORDERS: *mut u8 = ptr::null_mut();
static mut PAGE_COUNT: usize = 0; // tracked pages

/// Free lists for each order. Points to the physical address of the
/// first free block at that order (0 = empty).
static mut FREE_LISTS: [u64; MAX_ORDER + 1] = [0; MAX_ORDER + 1];

static mut INITIALIZED: bool = false;
static FREE_COUNT: AtomicUsize = AtomicUsize::new(0);
static mut TOTAL_RAM: u64 = 0;

// ── Helpers ─────────────────────────────────────────────────────────

/// Convert a physical address (≥ BASE) to a page index.
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
    if idx + len > PAGE_COUNT { return false; }
    for i in idx..idx + len {
        if *PAGE_ORDERS.add(i) != ORDER_FREE { return false; }
    }
    true
}

/// Mark a range of pages with a given order (called after allocating).
unsafe fn set_allocated(addr: u64, order: usize) {
    let idx = addr_to_idx(addr).unwrap();
    let len = 1usize << order;
    for i in idx..idx + len {
        *PAGE_ORDERS.add(i) = order as u8;
    }
}

/// Mark a range of pages as free.
unsafe fn set_free(addr: u64, order: usize) {
    let idx = addr_to_idx(addr).unwrap();
    let len = 1usize << order;
    for i in idx..idx + len {
        *PAGE_ORDERS.add(i) = ORDER_FREE;
    }
}

// ── Free list operations ────────────────────────────────────────────

/// Pop a block from the free list at `order`. Returns physical address
/// or 0 if empty.
unsafe fn list_pop(order: usize) -> u64 {
    let head = FREE_LISTS[order];
    if head == 0 { return 0; }
    // The free block headers are stored in the block itself:
    //   [0..7]  = next pointer (phys addr)
    //   [7..]   = (reserved for future)
    let next = *(head as *const u64);
    FREE_LISTS[order] = next;
    head
}

/// Push a block onto the free list at `order`. `addr` must be order-aligned.
unsafe fn list_push(addr: u64, order: usize) {
    *(addr as *mut u64) = FREE_LISTS[order];
    FREE_LISTS[order] = addr;
}

// ── Core buddy operations ───────────────────────────────────────────

/// Allocate 2^order contiguous physical pages. Returns base address.
/// Splits a larger block if no block is available at the exact order.
unsafe fn buddy_alloc(order: usize) -> Option<u64> {
    if order > MAX_ORDER { return None; }

    // Check if we have a block at this order, else try one higher.
    let mut o = order;
    while o <= MAX_ORDER && FREE_LISTS[o] == 0 {
        o += 1;
    }
    if o > MAX_ORDER { return None; } // OOM

    // Pop the block from the higher free list.
    let block = list_pop(o);
    if block == 0 { return None; }

    // Split: recursively push the upper half at each lower order.
    while o > order {
        o -= 1;
        let half_size = 1u64 << o; // in pages
        let upper = block + half_size * PAGE_SIZE;
        list_push(upper, o);
    }

    set_allocated(block, order);
    FREE_COUNT.fetch_sub(1usize << order, Ordering::Relaxed);
    cabina_daemon::telemetry::memory::inc_allocs();
    cabina_daemon::telemetry::memory::set_free_pages(FREE_COUNT.load(Ordering::Relaxed) as u64);
    Some(block)
}

/// Free a block of 2^order pages starting at `addr`. Coalesces with
/// the buddy if it's also free (same order, aligned).
unsafe fn buddy_free(addr: u64, order: usize) {
    if order > MAX_ORDER { return; }
    if addr == 0 { return; }

    let mut o = order;
    let mut block = addr;
    set_free(block, o);

    // Coalesce upward while the buddy is also free.
    while o < MAX_ORDER {
        let block_idx = addr_to_idx(block).unwrap();
        let buddy_idx = block_idx ^ (1usize << o);
        let buddy_addr = idx_to_addr(buddy_idx);
        if buddy_addr + (1u64 << o) * PAGE_SIZE > BASE + (PAGE_COUNT as u64) * PAGE_SIZE {
            break;
        }
        if !coalescable(buddy_addr, o) { break; }
        // Remove buddy from its free list.
        list_remove(buddy_addr, o);
        set_free(buddy_addr, o);
        // Merge: take the lower-aligned address.
        if buddy_addr < block { block = buddy_addr; }
        o += 1;
    }

    set_free(block, o);
    list_push(block, o);
    FREE_COUNT.fetch_add(1usize << o, Ordering::Relaxed);
}

/// Remove a specific block from a free list (during coalescing).
/// Scans the list to find and unlink `target`.
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

// ── Public API ──────────────────────────────────────────────────────

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

    // Detect total usable RAM.
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
    let meta_bytes = PAGE_COUNT; // 1 byte per page
    let meta_pages = (meta_bytes + PAGE_SIZE as usize - 1) / PAGE_SIZE as usize;

    // Bootstrap: allocate metadata from first large enough usable region.
    let mut meta_phys: u64 = 0;
    for e in entries {
        if e.mem_type != MemoryType::Usable { continue; }
        let region_start = e.base.max(BASE);
        let region_end = e.base + e.size;
        let avail = ((region_end - region_start) / PAGE_SIZE) as usize;
        if avail >= meta_pages {
            meta_phys = (region_start + PAGE_SIZE - 1) / PAGE_SIZE * PAGE_SIZE;
            break;
        }
    }
    if meta_phys == 0 {
        crate::dev::console::serial_write("[frame_alloc] FATAL: cannot allocate metadata\n");
        return;
    }

    PAGE_ORDERS = meta_phys as *mut u8;
    core::ptr::write_bytes(PAGE_ORDERS, ORDER_RSVD, meta_bytes);

    // Reserve metadata pages themselves.
    for i in 0..meta_pages {
        let p = meta_phys + (i as u64) * PAGE_SIZE;
        if let Some(idx) = addr_to_idx(p) {
            *PAGE_ORDERS.add(idx) = ORDER_RSVD;
        }
    }

    // Reserve crash marker page.
    if let Some(idx) = addr_to_idx(0x9_0000) {
        *PAGE_ORDERS.add(idx) = ORDER_RSVD;
    }

    // Mark all pages as free initially, then override reserved regions.
    // We iterate the UEFI map and free each usable region into the buddy.
    for e in entries {
        if e.mem_type != MemoryType::Usable { continue; }
        let region_start = e.base.max(BASE);
        let region_end = (e.base + e.size).min(BASE + (PAGE_COUNT as u64) * PAGE_SIZE);
        if region_start >= region_end { continue; }

        let start_page = (region_start + PAGE_SIZE - 1) / PAGE_SIZE * PAGE_SIZE;
        let end_page = region_end / PAGE_SIZE * PAGE_SIZE;

        let mut addr = start_page;
        while addr < end_page {
            // Skip kernel image, reserved area, metadata, crash marker.
            let mut skip = false;
            if ranges_overlap(addr, addr + PAGE_SIZE, kernel_base, kernel_base + kernel_size) { skip = true; }
            if reserved_size > 0 && ranges_overlap(addr, addr + PAGE_SIZE, reserved_addr, reserved_addr + reserved_size) { skip = true; }
            if ranges_overlap(addr, addr + PAGE_SIZE, meta_phys, meta_phys + meta_bytes as u64) { skip = true; }
            if ranges_overlap(addr, addr + PAGE_SIZE, 0x9_0000, 0x9_1000) { skip = true; }
            if skip {
                addr += PAGE_SIZE;
                continue;
            }
            // Free this single page into the buddy system.
            // To efficiently add many pages, we build up to order MAX_ORDER.
            buddy_free_page(addr);
            addr += PAGE_SIZE;
        }
    }

    INITIALIZED = true;
    let free = FREE_COUNT.load(Ordering::Relaxed);
    let free_mb = (free as u64 * PAGE_SIZE) / (1024 * 1024);
    crate::dev::console::serial_write("[frame_alloc] buddy init: ");
    crate::dev::console::serial_write_u64(free as u64, 10);
    crate::dev::console::serial_write(" free pages (");
    crate::dev::console::serial_write_u64(free_mb, 10);
    crate::dev::console::serial_write(" MB), metadata=");
    crate::dev::console::serial_write_u64(meta_pages as u64, 10);
    crate::dev::console::serial_write(" pages\n");
}

/// Free a single page, merging it into the buddy free lists by
/// building up to the highest possible order.
unsafe fn buddy_free_page(addr: u64) {
    let mut o = 0;
    let mut block = addr;
    set_free(block, o);

    while o < MAX_ORDER {
        let block_idx = addr_to_idx(block).unwrap();
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

fn ranges_overlap(a_start: u64, a_end: u64, b_start: u64, b_end: u64) -> bool {
    a_start < b_end && a_end > b_start
}

/// Allocate `count` contiguous pages. Rounds up to next power of 2.
pub unsafe fn alloc_pages_contiguous(count: usize) -> Option<u64> {
    if !INITIALIZED || count == 0 { return None; }
    let order = order_for(count);
    buddy_alloc(order)
}

/// Free `count` contiguous pages starting at `addr`.
/// The pages must have been allocated via `alloc_pages_contiguous`.
pub unsafe fn free_pages(addr: u64, count: usize) {
    if !INITIALIZED || count == 0 || addr == 0 { return; }
    debug_assert!(addr % PAGE_SIZE == 0, "free_pages: unaligned addr");
    let order = order_for(count);
    buddy_free(addr, order);
    cabina_daemon::telemetry::memory::inc_frees();
    cabina_daemon::telemetry::memory::set_free_pages(FREE_COUNT.load(Ordering::Relaxed) as u64);
}

pub fn free_count() -> usize {
    FREE_COUNT.load(Ordering::Relaxed)
}

pub fn total_ram() -> u64 {
    unsafe { TOTAL_RAM }
}

pub fn tracked_pages() -> usize {
    unsafe { PAGE_COUNT }
}

pub const fn page_size() -> usize {
    PAGE_SIZE as usize
}

fn order_for(count: usize) -> usize {
    if count == 0 { return 0; }
    let mut o = 0usize;
    let mut size = 1usize;
    while size < count && o < MAX_ORDER {
        size <<= 1;
        o += 1;
    }
    o
}
