//! Kernel Slab Allocator — free-list with first-fit, coalescing, and
//! proper alignment guarantees.
//!
//! Uses a 32 MB static buffer. v1.9 will switch to a dynamic heap backed
//! by the frame allocator.
//!
//! Design:
//!   - Block header: 8 bytes (next: u32 + size: u32)
//!   - First-fit search with splitting
//!   - O(1) prepend on free, O(n) coalescing with single-pass merge
//!   - Alignment: over-allocates to guarantee returned pointer meets
//!     layout.align() requirement

use core::alloc::{GlobalAlloc, Layout};

pub const HEAP_SIZE: usize = 32 * 1024 * 1024;
const BLOCK_HEADER_SIZE: usize = 8;
const MIN_BLOCK_SIZE: usize = 16;
const LIST_END: u32 = u32::MAX;

static mut HEAP_SPACE: [u8; HEAP_SIZE] = [0; HEAP_SIZE];
static mut FREE_LIST: u32 = LIST_END;
static mut INITIALIZED: bool = false;

#[derive(Clone, Copy)]
struct BlockHeader {
    next: u32,
    size: u32,
}

impl BlockHeader {
    #[inline]
    fn from_offset_mut(off: u32) -> &'static mut BlockHeader {
        unsafe { &mut *(HEAP_SPACE.as_mut_ptr().add(off as usize) as *mut BlockHeader) }
    }

    #[inline]
    fn data_ptr_from_offset(off: u32) -> *mut u8 {
        unsafe { HEAP_SPACE.as_mut_ptr().add(off as usize + BLOCK_HEADER_SIZE) }
    }

    #[inline]
    fn total_size(&self) -> usize {
        BLOCK_HEADER_SIZE + self.size as usize
    }
}

pub fn init_heap() {
    unsafe { init_free_list() }
}

unsafe fn init_free_list() {
    if INITIALIZED { return; }
    INITIALIZED = true;

    let first = BlockHeader::from_offset_mut(0);
    first.next = LIST_END;
    first.size = (HEAP_SIZE - BLOCK_HEADER_SIZE) as u32;
    FREE_LIST = 0;

    debug_assert!(first.size as usize <= HEAP_SIZE - BLOCK_HEADER_SIZE);
}

/// Find a free block large enough for `needed_size` with `needed_align`.
/// Returns a raw pointer to the block's data area, or null.
///
/// When `needed_align > BLOCK_HEADER_SIZE`, we over-allocate and return
/// an aligned sub-slice of the block. The original block is split around
/// the aligned region.
unsafe fn find_free(needed_size: usize, needed_align: usize) -> *mut u8 {
    let mut offset = FREE_LIST;
    let mut prev_offset: u32 = LIST_END;

    while offset != LIST_END {
        let block = BlockHeader::from_offset_mut(offset);
        let data_addr = BlockHeader::data_ptr_from_offset(offset) as usize;
        let aligned_addr = (data_addr + needed_align - 1) & !(needed_align - 1);
        let align_pad = aligned_addr - data_addr;
        let total_needed = align_pad + needed_size;

        if block.size as usize >= total_needed {
            let remaining = block.total_size().saturating_sub(BLOCK_HEADER_SIZE + total_needed);

            if remaining >= BLOCK_HEADER_SIZE + MIN_BLOCK_SIZE {
                let new_offset = (offset as usize + BLOCK_HEADER_SIZE + total_needed) as u32;
                let new_block = BlockHeader::from_offset_mut(new_offset);
                new_block.next = block.next;
                new_block.size = (remaining - BLOCK_HEADER_SIZE) as u32;

                block.size = total_needed as u32;

                if prev_offset == LIST_END {
                    FREE_LIST = new_offset;
                } else {
                    BlockHeader::from_offset_mut(prev_offset).next = new_offset;
                }
            } else {
                if prev_offset == LIST_END {
                    FREE_LIST = block.next;
                } else {
                    BlockHeader::from_offset_mut(prev_offset).next = block.next;
                }
            }
            return aligned_addr as *mut u8;
        }

        prev_offset = offset;
        offset = block.next;
    }

    core::ptr::null_mut()
}

/// Free a previously-allocated block. Single-pass coalescing with neighbors.
unsafe fn free_block(ptr: *mut u8) {
    if ptr.is_null() { return; }

    let heap_base = HEAP_SPACE.as_ptr() as usize;
    let ptr_addr = ptr as usize;

    if ptr_addr < heap_base + BLOCK_HEADER_SIZE || ptr_addr >= heap_base + HEAP_SIZE {
        return;
    }

    let block_offset = (ptr_addr - BLOCK_HEADER_SIZE - heap_base) as u32;

    if (block_offset as usize) % BLOCK_HEADER_SIZE != 0 {
        return;
    }

    let block = BlockHeader::from_offset_mut(block_offset);

    // Insert at head (O(1) prepend).
    block.next = FREE_LIST;
    FREE_LIST = block_offset;

    // Single-pass coalescing: scan once, merge all adjacent pairs.
    let mut off = FREE_LIST;
    let mut prev_off: u32 = LIST_END;

    while off != LIST_END {
        let b = BlockHeader::from_offset_mut(off);
        let next_off = b.next;
        let b_end = off as usize + b.total_size();

        if next_off != LIST_END && b_end == next_off as usize {
            let next = BlockHeader::from_offset_mut(next_off);
            let merged_size = b.total_size() + next.total_size() - BLOCK_HEADER_SIZE;
            b.size = merged_size as u32;
            b.next = next.next;

            if prev_off == LIST_END {
                FREE_LIST = off;
            } else {
                BlockHeader::from_offset_mut(prev_off).next = off;
            }
            continue;
        }

        prev_off = off;
        off = next_off;
    }
}

struct FreeListAllocator;

unsafe impl GlobalAlloc for FreeListAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if !INITIALIZED { init_free_list(); }
        let align = layout.align().max(BLOCK_HEADER_SIZE);
        let size = (layout.size() + align - 1) & !(align - 1);
        if size < layout.size() { return core::ptr::null_mut(); }
        find_free(size, align)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        free_block(ptr);
    }
}

#[global_allocator]
static ALLOCATOR: FreeListAllocator = FreeListAllocator;

pub fn heap_used() -> usize {
    unsafe {
        if !INITIALIZED { return 0; }
        let mut free_bytes = 0usize;
        let mut off = FREE_LIST;
        while off != LIST_END {
            let b = BlockHeader::from_offset_mut(off);
            free_bytes += b.total_size();
            off = b.next;
        }
        HEAP_SIZE - free_bytes
    }
}

pub const fn heap_total() -> usize {
    HEAP_SIZE
}

pub unsafe fn heap_alloc(size: usize, align: usize) -> *mut u8 {
    let layout = match Layout::from_size_align(size, align) {
        Ok(l) => l,
        Err(_) => return core::ptr::null_mut(),
    };
    ALLOCATOR.alloc(layout)
}

pub unsafe fn heap_free(ptr: *mut u8, size: usize, align: usize) {
    if ptr.is_null() { return; }
    if let Ok(layout) = Layout::from_size_align(size, align) {
        ALLOCATOR.dealloc(ptr, layout);
    }
}
