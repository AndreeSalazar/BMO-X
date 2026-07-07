//! Physical Frame Allocator â€” Generic dispatcher + Per-CPU Pagesets.
//!
//! Architecture:
//!
//!   alloc_pages_contiguous() / free_pages()
//!         â”‚
//!         â–¼
//!   Per-CPU pagesets (cache layer, orders 0..4)
//!         â”‚
//!         â–¼
//!   BackingAllocator trait (buddy :: llfree)
//!
//! The backing allocator is selected at compile time via Cargo features:
//!   - `alloc-buddy`  (default) â€” buddy system with coalescing
//!   - `alloc-llfree`           â€” lock-free LLFree allocator
//!
//! Each CPU has a local cache to avoid contending on the backing allocator.
//! Orders > PER_CPU_MAX_ORDER bypass the cache and go directly to backing.

use core::sync::atomic::{AtomicUsize, Ordering};
use bmo_boot_protocol::MemoryEntry;

const PAGE_SIZE: u64 = super::PAGE_SIZE;
const MAX_ORDER: usize = super::MAX_ORDER;
const PER_CPU_MAX_ORDER: usize = 4;
const BATCH_SIZE: usize = 16;
const CACHE_SLOTS: usize = PER_CPU_MAX_ORDER + 1;
const MAX_CPUS: usize = 64;

// â”€â”€ Select backing allocator â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

use super::BackingAllocator;

#[cfg(not(feature = "alloc-llfree"))]
use super::buddy::BuddyAllocator as Backing;
#[cfg(feature = "alloc-llfree")]
use super::llfree::LlfreeAllocator as Backing;

static BACKING: Backing = Backing;
static BACKING_INIT: AtomicUsize = AtomicUsize::new(0);

// â”€â”€ Per-CPU pagesets â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[derive(Copy, Clone)]
struct PerCpuCache {
    pages: [[u64; BATCH_SIZE]; CACHE_SLOTS],
    count: [usize; CACHE_SLOTS],
}

static mut PER_CPU: [PerCpuCache; MAX_CPUS] = [PerCpuCache {
    pages: [[0; BATCH_SIZE]; CACHE_SLOTS],
    count: [0; CACHE_SLOTS],
}; MAX_CPUS];

static CURRENT_CPU: AtomicUsize = AtomicUsize::new(0);

fn cpu_id() -> usize {
    CURRENT_CPU.load(Ordering::Relaxed)
}

pub fn set_cpu_id(id: usize) {
    CURRENT_CPU.store(id.min(MAX_CPUS - 1), Ordering::Relaxed);
}

// â”€â”€ Public API â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

/// Initialize the frame allocator from the UEFI memory map.
///
/// # Safety
/// Must be called once, on BSP, before any page allocation/free.
pub unsafe fn init(
    memory_map: &[MemoryEntry],
    count: usize,
    reserved_addr: u64,
    reserved_size: u64,
    kernel_base: u64,
    kernel_size: u64,
) {
    if BACKING_INIT.load(Ordering::Relaxed) != 0 { return; }
    BACKING.init(memory_map, count, reserved_addr, reserved_size, kernel_base, kernel_size);
    BACKING_INIT.store(1, Ordering::Relaxed);

    crate::dev::console::serial_write("[frame_alloc] backing: ");
    #[cfg(not(feature = "alloc-llfree"))]
    crate::dev::console::serial_write("buddy");
    #[cfg(feature = "alloc-llfree")]
    crate::dev::console::serial_write("llfree");
    crate::dev::console::serial_write(", per-CPU caches: ");
    crate::dev::console::serial_write_u64(MAX_CPUS as u64, 10);
    crate::dev::console::serial_write(" CPUs Ã— ");
    crate::dev::console::serial_write_u64(CACHE_SLOTS as u64, 10);
    crate::dev::console::serial_write(" orders\n");
}

/// Free physical memory above 2 GB into the allocator (called after page tables are set up).
pub unsafe fn free_high_memory(memory_map: &[MemoryEntry], count: usize) {
    BACKING.free_high_memory(memory_map, count);
}

/// Allocate `count` contiguous pages. Rounds up to next power of 2.
/// Small orders use the per-CPU cache; larger go directly to backing.
pub unsafe fn alloc_pages_contiguous(count: usize) -> Option<u64> {
    if count == 0 { return None; }
    let order = order_for(count);
    if order > PER_CPU_MAX_ORDER {
        return BACKING.alloc_order(order);
    }

    let cpu = cpu_id();
    let cache = &mut PER_CPU[cpu];

    if cache.count[order] > 0 {
        cache.count[order] -= 1;
        let page = cache.pages[order][cache.count[order]];
        cabina_daemon::telemetry::memory::inc_allocs();
        return Some(page);
    }

    let batch_order = order_for(BATCH_SIZE << order);
    let block = BACKING.alloc_order(batch_order)?;
    let mut addr = block;
    for i in 0..BATCH_SIZE {
        cache.pages[order][i] = addr;
        addr += (1u64 << order) * PAGE_SIZE;
    }
    cache.count[order] = BATCH_SIZE;

    cache.count[order] -= 1;
    let page = cache.pages[order][cache.count[order]];
    cabina_daemon::telemetry::memory::inc_allocs();
    Some(page)
}

/// Free `count` contiguous pages starting at `addr`.
pub unsafe fn free_pages(addr: u64, count: usize) {
    if count == 0 || addr == 0 { return; }
    debug_assert!(addr % PAGE_SIZE == 0, "free_pages: unaligned addr");
    let order = order_for(count);

    if order > PER_CPU_MAX_ORDER {
        BACKING.free_order(addr, order);
        cabina_daemon::telemetry::memory::inc_frees();
        return;
    }

    let cpu = cpu_id();
    let cache = &mut PER_CPU[cpu];

    if cache.count[order] < BATCH_SIZE {
        cache.pages[order][cache.count[order]] = addr;
        cache.count[order] += 1;
        cabina_daemon::telemetry::memory::inc_frees();
        return;
    }

    let flush = BATCH_SIZE / 2;
    for i in 0..flush {
        let p = cache.pages[order][i];
        if p != 0 {
            BACKING.free_order(p, order);
        }
    }
    for i in flush..BATCH_SIZE {
        cache.pages[order][i - flush] = cache.pages[order][i];
    }
    cache.count[order] = BATCH_SIZE - flush;

    cache.pages[order][cache.count[order]] = addr;
    cache.count[order] += 1;
    cabina_daemon::telemetry::memory::inc_frees();
}

pub fn free_count() -> usize {
    BACKING.free_count()
}

pub fn total_ram() -> u64 {
    BACKING.total_ram()
}

pub fn tracked_pages() -> usize {
    BACKING.tracked_pages()
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
