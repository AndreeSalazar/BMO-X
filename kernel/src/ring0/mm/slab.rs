//! Kernel Slab Allocator — free-list with first-fit, coalescing,
//! proper alignment, and dynamic growth from the frame allocator.
//!
//! No static buffer — heap grows by allocating physical pages on demand.
//! Initial chunk: 4 MB. Grows in 1 MB increments when allocation fails.

use core::alloc::{GlobalAlloc, Layout};

const INITIAL_CHUNK_SIZE: usize = 4 * 1024 * 1024;
const GROW_CHUNK_SIZE: usize = 1 * 1024 * 1024;
const BLOCK_HEADER_SIZE: usize = 8;
const MIN_BLOCK_SIZE: usize = 16;
const LIST_END: u32 = u32::MAX;

struct HeapChunk {
    base: *mut u8,
    size: usize,
}

static mut CHUNK_LIST: Option<HeapChunk> = None;
static mut FREE_LIST: u32 = LIST_END;
static mut INITIALIZED: bool = false;
static mut TOTAL_HEAP_SIZE: usize = 0;

#[derive(Clone, Copy)]
struct BlockHeader {
    next: u32,
    size: u32,
}

impl BlockHeader {
    #[inline]
    unsafe fn from_ptr(ptr: *mut u8) -> &'static mut BlockHeader {
        &mut *(ptr.sub(BLOCK_HEADER_SIZE) as *mut BlockHeader)
    }

    #[inline]
    fn data_ptr(&self) -> *mut u8 {
        let hdr = self as *const BlockHeader as usize;
        (hdr + BLOCK_HEADER_SIZE) as *mut u8
    }

    #[inline]
    fn total_size(&self) -> usize {
        BLOCK_HEADER_SIZE + self.size as usize
    }

    #[inline]
    fn chunk_end(&self) -> usize {
        self as *const BlockHeader as usize + self.total_size()
    }
}

unsafe fn add_chunk(size: usize) -> Option<*mut u8> {
    let pages = (size + super::PAGE_SIZE as usize - 1) / super::PAGE_SIZE as usize;
    let phys = crate::mm::phys::alloc_pages_contiguous(pages)?;
    let base = phys as *mut u8;
    let chunk_bytes = pages * super::PAGE_SIZE as usize;

    let first = base as *mut BlockHeader;
    (*first).next = LIST_END;
    (*first).size = (chunk_bytes - BLOCK_HEADER_SIZE) as u32;

    CHUNK_LIST = Some(HeapChunk { base, size: chunk_bytes });
    TOTAL_HEAP_SIZE += chunk_bytes;

    // Add this block to the free list
    (*first).next = FREE_LIST;
    FREE_LIST = base as u32;

    crate::dev::console::serial_write("[slab] grow +");
    crate::dev::console::serial_write_u64((chunk_bytes / (1024 * 1024)) as u64, 10);
    crate::dev::console::serial_write(" MB (total=");
    crate::dev::console::serial_write_u64((TOTAL_HEAP_SIZE / (1024 * 1024)) as u64, 10);
    crate::dev::console::serial_write(" MB)\n");

    Some(base)
}

pub fn init_heap() {
    unsafe {
        if INITIALIZED { return; }
        INITIALIZED = true;
        add_chunk(INITIAL_CHUNK_SIZE);
    }
}

unsafe fn find_free(needed_size: usize, needed_align: usize) -> *mut u8 {
    let mut offset = FREE_LIST;
    let mut prev_offset: u32 = LIST_END;

    while offset != LIST_END {
        let block = &mut *(offset as usize as *mut BlockHeader);
        let data_addr = (offset as usize) + BLOCK_HEADER_SIZE;
        let aligned_addr = (data_addr + needed_align - 1) & !(needed_align - 1);
        let align_pad = aligned_addr - data_addr;
        let total_needed = align_pad + needed_size;

        if block.size as usize >= total_needed {
            let remaining = block.total_size().saturating_sub(BLOCK_HEADER_SIZE + total_needed);

            if remaining >= BLOCK_HEADER_SIZE + MIN_BLOCK_SIZE {
                let new_offset = (offset as usize + BLOCK_HEADER_SIZE + total_needed) as u32;
                let new_block = &mut *(new_offset as usize as *mut BlockHeader);
                new_block.next = block.next;
                new_block.size = (remaining - BLOCK_HEADER_SIZE) as u32;
                block.size = total_needed as u32;

                if prev_offset == LIST_END {
                    FREE_LIST = new_offset;
                } else {
                    (*(prev_offset as usize as *mut BlockHeader)).next = new_offset;
                }
            } else {
                if prev_offset == LIST_END {
                    FREE_LIST = block.next;
                } else {
                    (*(prev_offset as usize as *mut BlockHeader)).next = block.next;
                }
            }
            return aligned_addr as *mut u8;
        }

        prev_offset = offset;
        offset = block.next;
    }

    core::ptr::null_mut()
}

unsafe fn free_block(ptr: *mut u8) {
    if ptr.is_null() { return; }

    let heap_base = match CHUNK_LIST {
        Some(ref chunk) => chunk.base as usize,
        None => return,
    };
    let heap_end = heap_base + TOTAL_HEAP_SIZE;
    let ptr_addr = ptr as usize;

    if ptr_addr < heap_base + BLOCK_HEADER_SIZE || ptr_addr >= heap_end { return; }
    if (ptr_addr - BLOCK_HEADER_SIZE) % BLOCK_HEADER_SIZE != 0 { return; }

    let block = &mut *((ptr_addr - BLOCK_HEADER_SIZE) as *mut BlockHeader);
    let block_offset = (ptr_addr - BLOCK_HEADER_SIZE) as u32;

    block.next = FREE_LIST;
    FREE_LIST = block_offset;

    let mut off = FREE_LIST;
    let mut prev_off: u32 = LIST_END;

    while off != LIST_END {
        let b = &mut *(off as usize as *mut BlockHeader);
        let next_off = b.next;
        let b_end = off as usize + b.total_size();

        if next_off != LIST_END && b_end == next_off as usize {
            let next = &mut *(next_off as usize as *mut BlockHeader);
            let merged_size = b.total_size() + next.total_size() - BLOCK_HEADER_SIZE;
            b.size = merged_size as u32;
            b.next = next.next;

            if prev_off == LIST_END {
                FREE_LIST = off;
            } else {
                (*(prev_off as usize as *mut BlockHeader)).next = off;
            }
            continue;
        }

        prev_off = off;
        off = next_off;
    }
}

struct DynamicAllocator;

unsafe impl GlobalAlloc for DynamicAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if !INITIALIZED { init_heap(); }

        let align = layout.align().max(BLOCK_HEADER_SIZE);
        let size = (layout.size() + align - 1) & !(align - 1);
        if size < layout.size() { return core::ptr::null_mut(); }

        let ptr = find_free(size, align);
        if !ptr.is_null() { return ptr; }

        // Heap full — grow and retry
        let grow_size = size.max(GROW_CHUNK_SIZE);
        if add_chunk(grow_size).is_none() {
            return core::ptr::null_mut();
        }
        find_free(size, align)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        free_block(ptr);
    }
}

#[global_allocator]
static ALLOCATOR: DynamicAllocator = DynamicAllocator;

pub fn heap_used() -> usize {
    unsafe {
        if !INITIALIZED { return 0; }
        let mut free_bytes = 0usize;
        let mut off = FREE_LIST;
        while off != LIST_END {
            let b = &mut *(off as usize as *mut BlockHeader);
            free_bytes += b.total_size();
            off = b.next;
        }
        TOTAL_HEAP_SIZE - free_bytes
    }
}

pub fn heap_total() -> usize {
    unsafe { TOTAL_HEAP_SIZE }
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
