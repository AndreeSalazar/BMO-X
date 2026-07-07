use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::ptr::{self, NonNull};
use bmo_abi::fundamentals::sync::BmoSpinLock;

const MIN_BLOCK: usize = 32;

pub(crate) const ARENA_SIZE: usize = 1024 * 1024;

const ARENA_MAGIC: u64 = 0x5254_4E41_B4D0_424D;

#[repr(transparent)]
pub struct BlockHeader(usize);

impl BlockHeader {
    const FREE: usize = 1;

    pub const fn new(size: usize, free: bool) -> Self {
        Self(size | if free { Self::FREE } else { 0 })
    }

    pub fn size(&self) -> usize {
        self.0 & !Self::FREE
    }

    pub fn is_free(&self) -> bool {
        self.0 & Self::FREE != 0
    }

    pub fn set_free(&mut self, free: bool) {
        if free { self.0 |= Self::FREE; } else { self.0 &= !Self::FREE; }
    }
}

pub const HEADER_SIZE: usize = core::mem::size_of::<BlockHeader>();

#[allow(dead_code)]
struct ArenaHeader {
    magic: u64,
    size: usize,
    next: Option<NonNull<ArenaHeader>>,
}

pub trait MemBackend {
    unsafe fn alloc_chunk(&self, min_size: usize) -> *mut u8;
    unsafe fn free_chunk(&self, ptr: *mut u8, size: usize);
}

struct HeapInner {
    free_head: *mut u8,
    arenas: Option<NonNull<ArenaHeader>>,
}

pub struct FreelistAllocator<B: MemBackend> {
    inner: UnsafeCell<HeapInner>,
    backend: B,
    lock: BmoSpinLock,
}

unsafe impl<B: MemBackend> Send for FreelistAllocator<B> {}
unsafe impl<B: MemBackend> Sync for FreelistAllocator<B> {}

impl<B: MemBackend> FreelistAllocator<B> {
    pub const fn new_with(backend: B) -> Self {
        Self {
            inner: UnsafeCell::new(HeapInner {
                free_head: ptr::null_mut(),
                arenas: None,
            }),
            backend,
            lock: BmoSpinLock::new(),
        }
    }

    pub fn allocate(&self, size: usize) -> *mut u8 {
        if size == 0 { return ptr::null_mut(); }
        let aligned = size.wrapping_add(7) & !7;
        let needed = HEADER_SIZE + aligned;
        if needed >= ARENA_SIZE / 2 {
            return unsafe { self.allocate_large(needed) };
        }
        self.lock.lock();
        let result = unsafe { self.allocate_from_freelist(needed) };
        self.lock.unlock();
        result
    }

    pub fn deallocate(&self, ptr: *mut u8, _layout: Layout) {
        if ptr.is_null() { return; }
        self.lock.lock();
        unsafe { self.deallocate_inner(ptr) };
        self.lock.unlock();
    }
}

impl<B: MemBackend> FreelistAllocator<B> {
    fn inner(&self) -> &mut HeapInner {
        unsafe { &mut *self.inner.get() }
    }

    unsafe fn allocate_from_freelist(&self, needed: usize) -> *mut u8 {
        let needed = needed.max(MIN_BLOCK);
        let inner = self.inner();
        let mut prev: *mut u8 = ptr::null_mut();
        let mut curr = inner.free_head;

        while !curr.is_null() {
            let hdr = &*curr.cast::<BlockHeader>();
            let block_size = hdr.size();
            if block_size >= needed {
                let next_free = self.read_next_free(curr);
                if prev.is_null() {
                    inner.free_head = next_free;
                } else {
                    self.write_next_free(prev, next_free);
                }

                let remaining = block_size - needed;
                if remaining >= MIN_BLOCK {
                    let alloc_hdr = curr;
                    let remainder_hdr = curr.add(needed);
                    alloc_hdr.cast::<BlockHeader>().write(BlockHeader::new(needed, false));
                    remainder_hdr.cast::<BlockHeader>().write(BlockHeader::new(remaining, true));
                    self.write_next_free(remainder_hdr, next_free);
                    inner.free_head = remainder_hdr;
                } else {
                    curr.cast::<BlockHeader>().write(BlockHeader::new(block_size, false));
                }
                return curr.add(HEADER_SIZE);
            }
            prev = curr;
            curr = self.read_next_free(curr);
        }

        let arena = self.add_arena(ARENA_SIZE);
        if arena.is_null() { return ptr::null_mut(); }

        let hdr = &*arena.cast::<ArenaHeader>();
        let arena_size = hdr.size;
        let block_start = arena.add(core::mem::size_of::<ArenaHeader>());
        let avail = (arena as usize + arena_size) - block_start as usize;
        if avail < needed { return ptr::null_mut(); }

        let remaining = avail - needed;
        if remaining >= MIN_BLOCK {
            block_start.cast::<BlockHeader>().write(BlockHeader::new(needed, false));
            let remainder_hdr = block_start.add(needed);
            remainder_hdr.cast::<BlockHeader>().write(BlockHeader::new(remaining, true));
            self.write_next_free(remainder_hdr, inner.free_head);
            inner.free_head = remainder_hdr;
            block_start.add(HEADER_SIZE)
        } else {
            block_start.cast::<BlockHeader>().write(BlockHeader::new(avail, false));
            block_start.add(HEADER_SIZE)
        }
    }

    unsafe fn deallocate_inner(&self, ptr: *mut u8) {
        let hdr_ptr = ptr.sub(HEADER_SIZE);
        let hdr = &mut *hdr_ptr.cast::<BlockHeader>();
        debug_assert!(!hdr.is_free(), "double free");
        hdr.set_free(true);
        let block_size = hdr.size();
        let next_block = hdr_ptr.add(block_size);

        let inner = self.inner();
        let mut curr = inner.free_head;
        let mut prev: *mut u8 = ptr::null_mut();

        while !curr.is_null() {
            if curr == next_block {
                let next_hdr = &*next_block.cast::<BlockHeader>();
                let next_size = next_hdr.size();
                let next_next = self.read_next_free(next_block);
                if prev.is_null() {
                    inner.free_head = next_next;
                } else {
                    self.write_next_free(prev, next_next);
                }
                let new_size = block_size + next_size;
                hdr_ptr.cast::<BlockHeader>().write(BlockHeader::new(new_size, true));
                break;
            }
            prev = curr;
            curr = self.read_next_free(curr);
        }

        self.write_next_free(hdr_ptr, inner.free_head);
        inner.free_head = hdr_ptr;
    }

    unsafe fn allocate_large(&self, needed: usize) -> *mut u8 {
        let total = needed.max(MIN_BLOCK);
        let raw = self.backend.alloc_chunk(total);
        if raw.is_null() { return ptr::null_mut(); }
        raw.cast::<BlockHeader>().write(BlockHeader::new(total, false));
        raw.add(HEADER_SIZE)
    }

    unsafe fn add_arena(&self, size: usize) -> *mut u8 {
        let total_size = size + core::mem::size_of::<ArenaHeader>();
        let raw = self.backend.alloc_chunk(total_size);
        if raw.is_null() { return ptr::null_mut(); }
        let arena = raw.cast::<ArenaHeader>();
        let inner = self.inner();
        arena.write(ArenaHeader {
            magic: ARENA_MAGIC,
            size: total_size,
            next: inner.arenas,
        });
        inner.arenas = Some(NonNull::new_unchecked(arena));
        raw
    }

    fn read_next_free(&self, hdr: *mut u8) -> *mut u8 {
        unsafe { hdr.add(HEADER_SIZE).cast::<*mut u8>().read() }
    }

    fn write_next_free(&self, hdr: *mut u8, next: *mut u8) {
        unsafe { hdr.add(HEADER_SIZE).cast::<*mut u8>().write(next); }
    }
}

unsafe impl<B: MemBackend> GlobalAlloc for FreelistAllocator<B> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.allocate(layout.size())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.deallocate(ptr, layout)
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if ptr.is_null() { return self.alloc(Layout::from_size_align_unchecked(new_size, layout.align())); }
        if new_size == 0 { self.dealloc(ptr, layout); return ptr::null_mut(); }
        let hdr_ptr = ptr.sub(HEADER_SIZE);
        let hdr = &*hdr_ptr.cast::<BlockHeader>();
        let old_size = hdr.size() - HEADER_SIZE;
        if new_size <= old_size { return ptr; }
        let new_ptr = self.alloc(Layout::from_size_align_unchecked(new_size, layout.align()));
        if new_ptr.is_null() { return ptr::null_mut(); }
        let copy_size = old_size.min(new_size);
        ptr::copy_nonoverlapping(ptr, new_ptr, copy_size);
        self.dealloc(ptr, layout);
        new_ptr
    }
}
