//! Heap allocator — free-list with first-fit for FastOS kernel.
//!
//! Provides alloc + dealloc. 16 MB heap, split into free-list blocks.
//! Each allocation uses an 8-byte header: [next_free: u32 | size: u32].
//! Free blocks are coalesced on dealloc when adjacent.

use core::alloc::{GlobalAlloc, Layout};
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

const HEAP_SIZE: usize = 16 * 1024 * 1024; // 16 MB
const BLOCK_HEADER_SIZE: usize = 8; // next_free: u32 + size: u32
const MIN_BLOCK_SIZE: usize = 16; // minimum usable size after header

static mut HEAP_SPACE: [u8; HEAP_SIZE] = [0; HEAP_SIZE];
static mut FREE_LIST: usize = 0; // head of free-list (offset into HEAP_SPACE)
static mut ALLOC_INIT: bool = false;
static ALLOCATOR_INIT: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy)]
struct BlockHeader {
    next: u32, // offset to next free block (0 = end)
    size: u32, // usable size of this block (excluding header)
}

impl BlockHeader {
    fn from_ptr(ptr: *mut u8) -> &'static mut BlockHeader {
        unsafe { &mut *(ptr as *mut BlockHeader) }
    }

    fn data_ptr(&self) -> *mut u8 {
        unsafe { (self as *const Self as *mut u8).add(BLOCK_HEADER_SIZE) }
    }

    fn total_size(&self) -> usize {
        BLOCK_HEADER_SIZE + self.size as usize
    }

    fn end_offset(&self) -> usize {
        let heap_base = unsafe { HEAP_SPACE.as_ptr() as usize };
        (self as *const Self as usize) - heap_base + self.total_size()
    }
}

/// Initialize the free-list with one big block spanning the entire heap.
/// Safe to call multiple times (idempotent).
pub fn init_heap() {
    unsafe { init_free_list() }
}

/// Initialize the free-list with one big block spanning the entire heap.
unsafe fn init_free_list() {
    if ALLOC_INIT { return; }
    ALLOC_INIT = true;

    // First block: starts at offset 0, spans full heap
    let first = BlockHeader::from_ptr(HEAP_SPACE.as_mut_ptr());
    first.next = 0; // end of list
    first.size = (HEAP_SIZE - BLOCK_HEADER_SIZE) as u32;
    FREE_LIST = 0;
}

/// Find a free block large enough for `needed_size`. Uses first-fit.
/// Returns a raw pointer to the block header, or null.
unsafe fn find_free(needed_size: usize) -> *mut u8 {
    let mut offset = FREE_LIST;
    let mut prev_offset: usize = 0;

    while offset != 0 {
        let block = BlockHeader::from_ptr(HEAP_SPACE.as_mut_ptr().add(offset));
        if block.size as usize >= needed_size {
            // Found a fit — split if large enough for two blocks
            let total_needed = BLOCK_HEADER_SIZE + needed_size;
            let remaining = block.total_size() - total_needed;

            if remaining >= BLOCK_HEADER_SIZE + MIN_BLOCK_SIZE {
                // Split: create a new free block after the allocated one
                let new_block_ptr = HEAP_SPACE.as_mut_ptr().add(offset + total_needed);
                let new_block = BlockHeader::from_ptr(new_block_ptr);
                new_block.next = block.next;
                new_block.size = (remaining - BLOCK_HEADER_SIZE) as u32;

                // Update the allocated block
                block.next = (new_block_ptr as usize - HEAP_SPACE.as_ptr() as usize) as u32;
                block.size = needed_size as u32;

                // Update prev pointer
                if prev_offset == 0 {
                    FREE_LIST = offset + total_needed;
                } else {
                    let prev = BlockHeader::from_ptr(HEAP_SPACE.as_mut_ptr().add(prev_offset));
                    prev.next = offset as u32 + total_needed as u32;
                }
            } else {
                // No split — allocate entire block
                if prev_offset == 0 {
                    FREE_LIST = block.next as usize;
                } else {
                    let prev = BlockHeader::from_ptr(HEAP_SPACE.as_mut_ptr().add(prev_offset));
                    prev.next = block.next;
                }
            }
            return block.data_ptr();
        }
        prev_offset = offset;
        offset = block.next as usize;
    }
    core::ptr::null_mut()
}

/// Free a block back into the free-list. Coalesces adjacent free blocks.
unsafe fn free_block(ptr: *mut u8) {
    if ptr.is_null() { return; }

    let block_ptr = ptr.sub(BLOCK_HEADER_SIZE);
    let block_offset = block_ptr as usize - HEAP_SPACE.as_ptr() as usize;
    let block = BlockHeader::from_ptr(block_ptr);

    // Insert at head of free-list (simple O(1) prepend)
                block.next = FREE_LIST as u32;
    FREE_LIST = block_offset;

    // Try to coalesce with adjacent blocks
    // Pass 1: merge with next block if it's physically adjacent
    let mut merged = true;
    while merged {
        merged = false;
        let mut off = FREE_LIST;
        let mut prev_off: usize = 0;
        while off != 0 {
            let b = BlockHeader::from_ptr(HEAP_SPACE.as_mut_ptr().add(off));
            let next_off = b.next as usize;
            let b_end = off + b.total_size();

            if next_off != 0 && b_end == next_off {
                // b and next block are adjacent — merge
                let next = BlockHeader::from_ptr(HEAP_SPACE.as_mut_ptr().add(next_off));
                let merged_size = b.total_size() + next.total_size() - BLOCK_HEADER_SIZE;
                b.size = merged_size as u32;
                b.next = next.next;

                // Update prev pointer
                if prev_off == 0 {
                    FREE_LIST = off;
                } else {
                    let prev = BlockHeader::from_ptr(HEAP_SPACE.as_mut_ptr().add(prev_off));
                    prev.next = off as u32;
                }
                merged = true;
                break; // restart coalescing
            }
            prev_off = off;
            off = next_off;
        }
    }
}

struct FreeListAllocator;

unsafe impl GlobalAlloc for FreeListAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if !ALLOC_INIT { init_free_list(); }

        let size = layout.size();
        let align = layout.align();

        // Align up to block header alignment (8 bytes)
        let needed = (size + 7) & !7;

        find_free(needed)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        free_block(ptr);
    }
}

#[global_allocator]
static ALLOCATOR: FreeListAllocator = FreeListAllocator;

/// Bytes of heap currently in use (allocated + fragmentation overhead).
pub fn heap_used() -> usize {
    if !unsafe { ALLOC_INIT } { return 0; }

    // Walk free-list and count free bytes
    let mut free_bytes = 0usize;
    unsafe {
        let mut offset = FREE_LIST;
        while offset != 0 {
            let block = BlockHeader::from_ptr(HEAP_SPACE.as_mut_ptr().add(offset));
            free_bytes += block.total_size();
            offset = block.next as usize;
        }
    }
    HEAP_SIZE - free_bytes
}

pub const fn heap_total() -> usize {
    HEAP_SIZE
}

#[no_mangle]
pub extern "C" fn __rust_no_alloc_shim_is_unstable() {}
