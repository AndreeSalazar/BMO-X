//! Kernel Slab Allocator — object caching with per-size caches.
//!
//! Pre-defined size classes for common allocations (16 B .. 4 KiB).
//! Each cache manages a linked list of 4 KiB slabs from the buddy
//! allocator. Each slab is divided into N equally-sized objects with
//! a free-object bitmap.
//!
//! Three slab states per cache: empty, partial, full.
//! Allocation O(1): pop from partial (or empty if partial empty).
//! Free O(1): return object to its slab's bitmap, promote slab.
//!
//! For sizes > 4 KiB or non-standard alignments: falls back to
//! direct buddy allocator pages (4 KiB multiples).

use core::alloc::{GlobalAlloc, Layout};
use core::ptr;
use core::sync::atomic::{AtomicUsize, Ordering};

const SLAB_SIZE: usize = 4096; // each slab = 1 physical page
const CACHE_COUNT: usize = 16;

// Power-of-2-ish size classes from 16 to 4096
const CACHE_SIZES: [usize; CACHE_COUNT] = [
    16, 24, 32, 48, 64, 96, 128, 192, 256, 384, 512, 768, 1024, 1536, 2048, 3072,
];

// ── Slab metadata (embedded at the start of each 4 KiB slab page) ───
//
// Uses a free-list of indices stored IN free objects themselves
// (each free object's first 4 bytes hold the next-free index).
// This requires obj_size ≥ 4 (our smallest cache is 16 B, fine).

#[repr(C)]
struct SlabHead {
    next: *mut SlabHead, // next slab in the cache's list
    prev: *mut SlabHead, // previous slab
    cache_idx: u8,       // index into CACHE_SIZES
    obj_size: u16,       // size of each object in this slab
    free_count: u16,     // how many objects are free
    first_free: u32,     // index of first free object, or u32::MAX if none
    _pad: [u8; 3],       // pad to 32 bytes for alignment
}

const HEADER_SIZE: usize = 32;
const FREE_END: u32 = u32::MAX;

impl SlabHead {
    fn obj_count(&self) -> usize {
        (SLAB_SIZE - HEADER_SIZE) / self.obj_size as usize
    }

    fn obj_ptr(&self, index: usize) -> *mut u8 {
        unsafe {
            let base = (self as *const Self as usize + HEADER_SIZE) as *mut u8;
            base.add(index * self.obj_size as usize)
        }
    }

    fn pop_free(&mut self) -> Option<u32> {
        if self.first_free == FREE_END { return None; }
        let idx = self.first_free;
        // Read the next-free index from the free object itself (first 4 bytes).
        let next = unsafe { *(self.obj_ptr(idx as usize) as *const u32) };
        self.first_free = if next == 0 { FREE_END } else { next };
        self.free_count -= 1;
        Some(idx)
    }

    fn push_free(&mut self, idx: u32) {
        // Store the current first_free as the next-free in this object.
        unsafe { *(self.obj_ptr(idx as usize) as *mut u32) = self.first_free; }
        self.first_free = idx;
        self.free_count += 1;
    }

    fn build_free_list(&mut self) {
        let count = self.obj_count();
        self.free_count = count as u16;
        self.first_free = 0;
        for i in 0..count {
            let next = if i + 1 < count { (i + 1) as u32 } else { FREE_END };
            unsafe { *(self.obj_ptr(i) as *mut u32) = next; }
        }
    }
}

// ── Per-cache structure ─────────────────────────────────────────────

fn size_for_align(size: usize, align: usize) -> usize {
    size.max(align)
}

#[derive(Copy, Clone)]
struct SlabCache {
    obj_size: usize,
    partial: *mut SlabHead,
    empty: *mut SlabHead,
    full: *mut SlabHead,
}

static mut CACHES: [SlabCache; CACHE_COUNT + 1] = [SlabCache {
    obj_size: 0,
    partial: ptr::null_mut(),
    empty: ptr::null_mut(),
    full: ptr::null_mut(),
}; CACHE_COUNT + 1];
// The +1 cache is for "large allocations" (> 4 KiB) handled by buddy.

static mut INITIALIZED: bool = false;
static mut HEAP_TOTAL: usize = 0;
static IN_USE: AtomicUsize = AtomicUsize::new(0);

// ── Slab operations ─────────────────────────────────────────────────

/// Create a new slab for the given cache index, backing it with one
/// page from the buddy allocator. Returns pointer to the SlabHead.
unsafe fn slab_create(cache_idx: usize) -> Option<*mut SlabHead> {
    let phys = crate::mm::phys::alloc_pages_contiguous(1)?;
    let virt = crate::mm::virt::phys_to_virt(phys);
    core::ptr::write_bytes(virt as *mut u8, 0, SLAB_SIZE);

    let obj_size = CACHE_SIZES[cache_idx];
    let head = &mut *(virt as *mut SlabHead);
    head.cache_idx = cache_idx as u8;
    head.obj_size = obj_size as u16;
    head.first_free = FREE_END;
    head.free_count = 0;
    head.build_free_list();

    HEAP_TOTAL += SLAB_SIZE;
    Some(head)
}

/// Destroy a slab: return its page to the buddy allocator.
unsafe fn slab_destroy(head: *mut SlabHead) {
    let virt = head as u64;
    let phys = crate::mm::virt::virt_to_phys(virt);
    crate::mm::phys::free_pages(phys, 1);
    HEAP_TOTAL -= SLAB_SIZE;
}

/// Allocate an object from a cache. Returns pointer or null if OOM.
unsafe fn cache_alloc(cache_idx: usize) -> *mut u8 {
    let cache = &mut CACHES[cache_idx];

    // 1. Try partial slab.
    if !cache.partial.is_null() {
        let slab = &mut *cache.partial;
        if let Some(idx) = slab.pop_free() {
            let ptr = slab.obj_ptr(idx as usize);
            if slab.free_count == 0 {
                let list = &mut cache.full as *mut *mut SlabHead;
                unlink_slab(cache, slab);
                link_slab(slab, &mut *list);
            }
            return ptr;
        }
        let list = &mut cache.full as *mut *mut SlabHead;
        unlink_slab(cache, slab);
        link_slab(slab, &mut *list);
    }

    // 2. Try empty slab.
    if !cache.empty.is_null() {
        let slab = &mut *cache.empty;
        let list_p = &mut cache.partial as *mut *mut SlabHead;
        unlink_slab(cache, slab);
        link_slab(slab, &mut *list_p);
        if let Some(idx) = slab.pop_free() {
            let ptr = slab.obj_ptr(idx as usize);
            if slab.free_count == 0 {
                let list_f = &mut cache.full as *mut *mut SlabHead;
                unlink_slab(cache, slab);
                link_slab(slab, &mut *list_f);
            }
            return ptr;
        }
    }

    // 3. Create a new slab.
    let new = match slab_create(cache_idx) {
        Some(s) => s,
        None => return ptr::null_mut(),
    };
    let list_e = &mut cache.empty as *mut *mut SlabHead;
    link_slab(&mut *new, &mut *list_e);
    cache_alloc(cache_idx)
}

/// Free an object. Determine its slab by rounding the pointer down
/// to the 4 KiB page boundary (each slab occupies exactly one page).
unsafe fn cache_free(ptr: *mut u8, cache_idx: usize) {
    let page = (ptr as usize) & !(SLAB_SIZE - 1);
    let slab = &mut *(page as *mut SlabHead);
    let offset = ptr as usize - page - HEADER_SIZE;
    let obj_idx = (offset / slab.obj_size as usize) as u32;
    slab.push_free(obj_idx);

    let cache = &mut CACHES[cache_idx];
    if slab.free_count == 1 && slab.obj_count() > 1 {
        let list_p = &mut cache.partial as *mut *mut SlabHead;
        unlink_slab(cache, slab);
        link_slab(slab, &mut *list_p);
    } else if slab.free_count as usize == slab.obj_count() {
        let list_e = &mut cache.empty as *mut *mut SlabHead;
        unlink_slab(cache, slab);
        link_slab(slab, &mut *list_e);
    }
}

unsafe fn unlink_slab(cache: &mut SlabCache, slab: &mut SlabHead) {
    let next = slab.next;
    let prev = slab.prev;
    if !prev.is_null() { (*prev).next = next; }
    else if core::ptr::addr_eq(cache.partial, slab as *mut _) { cache.partial = next; }
    else if core::ptr::addr_eq(cache.empty, slab as *mut _) { cache.empty = next; }
    else if core::ptr::addr_eq(cache.full, slab as *mut _) { cache.full = next; }
    if !next.is_null() { (*next).prev = prev; }
    slab.next = ptr::null_mut();
    slab.prev = ptr::null_mut();
}

unsafe fn link_slab(slab: &mut SlabHead, list: &mut *mut SlabHead) {
    slab.next = *list;
    slab.prev = ptr::null_mut();
    if !(*list).is_null() { (**list).prev = slab; }
    *list = slab;
}

// ── Public API ──────────────────────────────────────────────────────

/// Find the cache index for a given allocation size.
/// Returns None for large allocations (> 4 KiB).
fn cache_for(size: usize) -> Option<usize> {
    if size > CACHE_SIZES[CACHE_COUNT - 1] || size == 0 { return None; }
    for i in 0..CACHE_COUNT {
        if size <= CACHE_SIZES[i] { return Some(i); }
    }
    None
}

pub fn init_heap() {
    unsafe {
        if INITIALIZED { return; }
        for i in 0..CACHE_COUNT {
            CACHES[i].obj_size = CACHE_SIZES[i];
        }
        INITIALIZED = true;
    }
}

/// Allocate `size` bytes with `align` alignment.
/// Small allocations go through slab caches; large ones through buddy.
pub unsafe fn heap_alloc(size: usize, align: usize) -> *mut u8 {
    if size == 0 { return ptr::null_mut(); }
    if let Some(ci) = cache_for(size) {
        if align > CACHE_SIZES[ci].min(64) {
            // Alignment larger than our slab object can provide → buddy fallback
            return buddy_alloc(size, align);
        }
        IN_USE.fetch_add(size, Ordering::Relaxed);
        cabina_daemon::telemetry::memory::inc_allocs();
        cache_alloc(ci)
    } else {
        cabina_daemon::telemetry::memory::inc_allocs();
        buddy_alloc(size, align)
    }
}

/// Free memory allocated by `heap_alloc`.
pub unsafe fn heap_free(ptr: *mut u8, size: usize, align: usize) {
    if ptr.is_null() || size == 0 { return; }
    if let Some(ci) = cache_for(size) {
        if align > CACHE_SIZES[ci].min(64) {
            buddy_free(ptr, size);
            return;
        }
        IN_USE.fetch_sub(size, Ordering::Relaxed);
        cabina_daemon::telemetry::memory::inc_frees();
        cache_free(ptr, ci);
    } else {
        cabina_daemon::telemetry::memory::inc_frees();
        buddy_free(ptr, size);
    }
}

/// Buddy fallback for large allocations or special alignment.
unsafe fn buddy_alloc(size: usize, align: usize) -> *mut u8 {
    let pages = (size + SLAB_SIZE - 1) / SLAB_SIZE;
    let phys = match crate::mm::phys::alloc_pages_contiguous(pages) {
        Some(p) => p,
        None => return ptr::null_mut(),
    };
    let virt = crate::mm::virt::phys_to_virt(phys);
    // Align within the allocated pages if needed.
    let off = (virt as usize) & (align - 1);
    let adjusted = if off == 0 { virt } else { virt + (align - off) as u64 };
    if adjusted + size as u64 > virt + (pages * SLAB_SIZE) as u64 {
        crate::mm::phys::free_pages(phys, pages);
        let phys2 = match crate::mm::phys::alloc_pages_contiguous(pages + 1) {
            Some(p) => p,
            None => return ptr::null_mut(),
        };
        let virt2 = crate::mm::virt::phys_to_virt(phys2);
        let off2 = (virt2 as usize) & (align - 1);
        let adj2 = if off2 == 0 { virt2 } else { virt2 + (align - off2) as u64 };
        HEAP_TOTAL += (pages + 1) * SLAB_SIZE;
        IN_USE.fetch_add(size, Ordering::Relaxed);
        return adj2 as *mut u8;
    }
    HEAP_TOTAL += pages * SLAB_SIZE;
    IN_USE.fetch_add(size, Ordering::Relaxed);
    adjusted as *mut u8
}

unsafe fn buddy_free(ptr: *mut u8, size: usize) {
    let pages = (size + SLAB_SIZE - 1) / SLAB_SIZE;
    // Round down to page boundary
    let virt = (ptr as u64) & !(SLAB_SIZE as u64 - 1);
    let phys = crate::mm::virt::virt_to_phys(virt);
    crate::mm::phys::free_pages(phys, pages);
    HEAP_TOTAL -= pages * SLAB_SIZE;
    IN_USE.fetch_sub(size, Ordering::Relaxed);
}

// ── GlobalAlloc impl (Rust alloc) ───────────────────────────────────

struct SlabAllocator;

unsafe impl GlobalAlloc for SlabAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if !INITIALIZED { init_heap(); }
        heap_alloc(layout.size(), layout.align())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        heap_free(ptr, layout.size(), layout.align());
    }
}

#[global_allocator]
static ALLOCATOR: SlabAllocator = SlabAllocator;

pub fn heap_used() -> usize {
    IN_USE.load(Ordering::Relaxed)
}

pub fn heap_total() -> usize {
    unsafe { HEAP_TOTAL }
}
