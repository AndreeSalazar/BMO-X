//! Kernel heap allocator — free-list with first-fit and coalescing.
//!
//! v1.8.9: bug fix crítico. El sentinel "fin de lista = 0" colisionaba con
//! el offset del primer bloque (que vive en offset 0). Resultado:
//! `find_free` saltaba el bloque 0 → primer `Vec::new()`/`Box::new()`
//! retornaba null → panic silencioso en fase 2 ó 3.
//!
//! v1.8.9: usar `u32::MAX` como sentinel de fin. Ahora offset 0 es un
//! offset válido para el primer bloque.
//!
//! v1.8.9: añadir alineación correcta (no solo a 8 bytes — respetar
//! `layout.align()`), coalescing más robusto, y assert de sanity en
//! `init_heap`.

#![allow(dead_code)]

use core::alloc::{GlobalAlloc, Layout};
use core::sync::atomic::AtomicBool;

// ── Layout constants ─────────────────────────────────────────────────

/// Tamaño total del heap estático. 1 MB reduce el BSS del kernel a
/// ~1 MB en vez de los 16 MB originales. v1.9 lo cambiará por heap
/// dinámico backed por el page allocator.
pub const HEAP_SIZE: usize = 1024 * 1024;

/// Tamaño del header de cada bloque (next: u32 + size: u32).
const BLOCK_HEADER_SIZE: usize = 8;

/// Tamaño mínimo utilizable en un bloque libre (post-header) para que
/// valga la pena mantenerlo tras un split.
const MIN_BLOCK_SIZE: usize = 16;

/// Sentinel de "fin de lista". Usamos `u32::MAX` para no colisionar
/// con offset 0 (donde vive el primer bloque).
const LIST_END: u32 = u32::MAX;

// ── Static state ─────────────────────────────────────────────────────

static mut HEAP_SPACE: [u8; HEAP_SIZE] = [0; HEAP_SIZE];
static mut FREE_LIST: u32 = LIST_END;
static mut ALLOC_INIT: bool = false;
static ALLOCATOR_INIT: AtomicBool = AtomicBool::new(false);

// ── Block header ─────────────────────────────────────────────────────

#[derive(Clone, Copy)]
struct BlockHeader {
    /// Offset al siguiente bloque libre, o `LIST_END` si es el último.
    next: u32,
    /// Tamaño utilizable del bloque (excluye el header de 8 bytes).
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

// ── Public init API ──────────────────────────────────────────────────

/// Initialize the free-list with one big block spanning the entire heap.
/// Idempotent: safe to call multiple times.
pub fn init_heap() {
    unsafe { init_free_list() }
}

unsafe fn init_free_list() {
    if ALLOC_INIT { return; }
    ALLOC_INIT = true;

    // Un solo bloque en offset 0 que ocupa todo el heap.
    let first = BlockHeader::from_offset_mut(0);
    first.next = LIST_END;
    first.size = (HEAP_SIZE - BLOCK_HEADER_SIZE) as u32;
    FREE_LIST = 0;

    // Sanity check: el primer bloque debe tener sentido.
    debug_assert!(first.size as usize <= HEAP_SIZE - BLOCK_HEADER_SIZE);
}

// ── Free-list walk ───────────────────────────────────────────────────

/// Find a free block large enough for `needed_size`. First-fit.
/// Returns a raw pointer to the block's data area, or null.
unsafe fn find_free(needed_size: usize) -> *mut u8 {
    let mut offset = FREE_LIST;
    let mut prev_offset: u32 = LIST_END;

    while offset != LIST_END {
        let block = BlockHeader::from_offset_mut(offset);

        if block.size as usize >= needed_size {
            // Found a fit. Try to split if there's enough room left.
            let total_needed = BLOCK_HEADER_SIZE + needed_size;
            let remaining = block.total_size().saturating_sub(total_needed);

            if remaining >= BLOCK_HEADER_SIZE + MIN_BLOCK_SIZE {
                // Split: nuevo bloque libre después del que vamos a ocupar.
                let new_offset = offset + total_needed as u32;
                let new_block = BlockHeader::from_offset_mut(new_offset);
                new_block.next = block.next;
                new_block.size = (remaining - BLOCK_HEADER_SIZE) as u32;

                block.size = needed_size as u32;

                // Re-link: prev → new_offset, o FREE_LIST si era el head.
                if prev_offset == LIST_END {
                    FREE_LIST = new_offset;
                } else {
                    BlockHeader::from_offset_mut(prev_offset).next = new_offset;
                }
            } else {
                // No split — consume the entire block.
                if prev_offset == LIST_END {
                    FREE_LIST = block.next;
                } else {
                    BlockHeader::from_offset_mut(prev_offset).next = block.next;
                }
            }
            return BlockHeader::data_ptr_from_offset(offset);
        }

        prev_offset = offset;
        offset = block.next;
    }

    core::ptr::null_mut()
}

/// Free a previously-allocated block. Coalesces with adjacent free blocks.
unsafe fn free_block(ptr: *mut u8) {
    if ptr.is_null() { return; }

    let block_ptr = ptr.sub(BLOCK_HEADER_SIZE);
    let block_offset = (block_ptr as usize - HEAP_SPACE.as_ptr() as usize) as u32;
    if block_offset as usize >= HEAP_SIZE {
        // Wild pointer — ignore. En release, no panic.
        return;
    }

    // Insert at head (O(1) prepend).
    let block = BlockHeader::from_offset_mut(block_offset);
    block.next = FREE_LIST;
    FREE_LIST = block_offset;

    // Coalesce iteratively until no more merges happen.
    loop {
        let mut merged = false;
        let mut off = FREE_LIST;
        let mut prev_off: u32 = LIST_END;

        while off != LIST_END {
            let b = BlockHeader::from_offset_mut(off);
            let next_off = b.next;
            let b_end = off as usize + b.total_size();

            if next_off != LIST_END && b_end == next_off as usize {
                // b y next son adyacentes — fusionar.
                let next = BlockHeader::from_offset_mut(next_off);
                let merged_size = b.total_size() + next.total_size() - BLOCK_HEADER_SIZE;
                b.size = merged_size as u32;
                b.next = next.next;

                if prev_off == LIST_END {
                    FREE_LIST = off;
                } else {
                    BlockHeader::from_offset_mut(prev_off).next = off;
                }
                merged = true;
                break;
            }

            prev_off = off;
            off = next_off;
        }

        if !merged { break; }
    }
}

// ── GlobalAlloc impl ─────────────────────────────────────────────────

struct FreeListAllocator;

unsafe impl GlobalAlloc for FreeListAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if !ALLOC_INIT { init_free_list(); }

        // Tamaño alineado al máximo entre 8 bytes y `layout.align()`.
        // Esto es crítico: el page-table allocator pide align=4096, y
        // sin esto devolvemos memoria desalineada.
        let align = layout.align().max(8);
        let size = (layout.size() + align - 1) & !(align - 1);
        if size < layout.size() { return core::ptr::null_mut(); } // overflow

        find_free(size)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        free_block(ptr);
    }
}

#[global_allocator]
static ALLOCATOR: FreeListAllocator = FreeListAllocator;

// ── Public introspection ─────────────────────────────────────────────

/// Bytes of heap currently in use (allocated + fragmentation overhead).
/// Returns 0 if the heap hasn't been initialized.
pub fn heap_used() -> usize {
    unsafe {
        if !ALLOC_INIT { return 0; }

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

// ── Public raw alloc API ─────────────────────────────────────────────

/// Allocate raw bytes with explicit alignment, returning a pointer or null.
/// Used by the page-table allocator and other code that needs a 4 KB-aligned
/// chunk from the heap without depending on `core::alloc::Layout`.
pub unsafe fn heap_alloc(size: usize, align: usize) -> *mut u8 {
    let layout = match core::alloc::Layout::from_size_align(size, align) {
        Ok(l) => l,
        Err(_) => return core::ptr::null_mut(),
    };
    let ptr = ALLOCATOR.alloc(layout);
    if !ptr.is_null() {
        // One-shot diagnostic on first successful alloc.
        static mut PRINTED: bool = false;
        if !PRINTED {
            PRINTED = true;
            let base = HEAP_SPACE.as_ptr() as u64;
            crate::dev::console::serial_write("[heap] HEAP_SPACE base=0x");
            crate::boot::serial::hex(base);
            crate::dev::console::serial_write(" ret=0x");
            crate::boot::serial::hex(ptr as u64);
            crate::dev::console::serial_write(" size=");
            crate::dev::console::serial_write_u64(HEAP_SIZE as u64, 10);
            crate::dev::console::serial_write("\n");
        }
    }
    ptr
}

/// Free memory previously returned by `heap_alloc`.
pub unsafe fn heap_free(ptr: *mut u8, size: usize, align: usize) {
    if ptr.is_null() { return; }
    if let Ok(layout) = core::alloc::Layout::from_size_align(size, align) {
        ALLOCATOR.dealloc(ptr, layout);
    }
}
