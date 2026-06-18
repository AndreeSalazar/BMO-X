//! kernel32.dll — Memory management.
//!
//! Real implementations using BMO's page allocator and heap.
//! Maps: VirtualAlloc, VirtualFree, HeapCreate/Alloc/Free.

#![allow(dead_code)]

use crate::bmo_abi::primitives::bx_u64;

const MEM_COMMIT: u32 = 0x00001000;
const MEM_RESERVE: u32 = 0x00002000;
const MEM_RELEASE: u32 = 0x00008000;
const MEM_DECOMMIT: u32 = 0x00004000;

/// VirtualAlloc — allocate virtual memory.
///
/// Real implementation: uses BMO heap allocator.
/// Maps to BMO syscall 0x10 (Mmap) conceptually.
#[no_mangle]
pub extern "C" fn VirtualAlloc(addr: bx_u64, size: bx_u64, alloc_type: u32, protect: u32) -> bx_u64 {
    let _ = (addr, protect);

    if size == 0 || size > (1 << 30) {
        return 0;
    }

    let align = 4096;
    let aligned_size = ((size as usize) + align - 1) & !(align - 1);

    let layout = match core::alloc::Layout::from_size_align(aligned_size, align) {
        Ok(l) => l,
        Err(_) => return 0,
    };

    let ptr = unsafe { alloc::alloc::alloc_zeroed(layout) };
    if ptr.is_null() {
        return 0;
    }

    ptr as bx_u64
}

/// VirtualFree — free virtual memory.
///
/// Real implementation: uses BMO heap deallocator.
/// Maps to BMO syscall 0x11 (Munmap) conceptually.
#[no_mangle]
pub extern "C" fn VirtualFree(addr: bx_u64, size: bx_u64, free_type: u32) -> bx_u64 {
    if addr == 0 {
        return 0;
    }

    if free_type & MEM_RELEASE != 0 {
        let align = 4096;
        let aligned_size = ((size as usize) + align - 1) & !(align - 1);
        if let Ok(layout) = core::alloc::Layout::from_size_align(aligned_size, align) {
            unsafe {
                alloc::alloc::dealloc(addr as *mut u8, layout);
            }
        }
    }

    1 // TRUE
}

/// VirtualProtect — change memory protection.
///
/// BMO doesn't have fine-grained memory protection yet.
/// Returns TRUE to indicate success (no-op).
#[no_mangle]
pub extern "C" fn VirtualProtect(addr: bx_u64, size: bx_u64, new_prot: u32, old_prot: *mut u32) -> bx_u64 {
    let _ = (addr, size, new_prot);
    if !old_prot.is_null() {
        unsafe { *old_prot = 0x04; } // PAGE_READWRITE
    }
    1 // TRUE
}

/// HeapCreate — create a private heap.
///
/// For simplicity, returns a pseudo-handle. All allocations go through
/// the global BMO heap.
#[no_mangle]
pub extern "C" fn HeapCreate(options: u32, initial_size: bx_u64, maximum_size: bx_u64) -> bx_u64 {
    let _ = (options, initial_size, maximum_size);
    // Return a pseudo-handle (1 = default process heap)
    1
}

/// HeapAlloc — allocate from a heap.
#[no_mangle]
pub extern "C" fn HeapAlloc(heap: bx_u64, flags: u32, size: bx_u64) -> bx_u64 {
    let _ = (heap, flags);

    if size == 0 {
        return 0;
    }

    let align = 16;
    let aligned_size = ((size as usize) + align - 1) & !(align - 1);

    let layout = match core::alloc::Layout::from_size_align(aligned_size, align) {
        Ok(l) => l,
        Err(_) => return 0,
    };

    let ptr = unsafe { alloc::alloc::alloc_zeroed(layout) };
    if ptr.is_null() {
        return 0;
    }

    ptr as bx_u64
}

/// HeapFree — free from a heap.
#[no_mangle]
pub extern "C" fn HeapFree(heap: bx_u64, flags: u32, ptr: bx_u64) -> bx_u64 {
    let _ = (heap, flags);

    if ptr == 0 {
        return 1;
    }

    // We don't know the exact size, so we use a conservative approach
    // In a real implementation, we'd track allocation sizes
    // For now, just return success
    1
}

/// GetProcessHeap — get the default process heap.
#[no_mangle]
pub extern "C" fn GetProcessHeap() -> bx_u64 {
    1 // pseudo-handle for default heap
}

/// GlobalAlloc — allocate global memory.
#[no_mangle]
pub extern "C" fn GlobalAlloc(flags: u32, size: bx_u64) -> bx_u64 {
    HeapAlloc(1, 0, size)
}

/// GlobalFree — free global memory.
#[no_mangle]
pub extern "C" fn GlobalFree(handle: bx_u64) -> bx_u64 {
    HeapFree(1, 0, handle);
    0
}

/// LocalAlloc — allocate local memory.
#[no_mangle]
pub extern "C" fn LocalAlloc(flags: u32, size: bx_u64) -> bx_u64 {
    HeapAlloc(1, 0, size)
}

/// LocalFree — free local memory.
#[no_mangle]
pub extern "C" fn LocalFree(handle: bx_u64) -> bx_u64 {
    HeapFree(1, 0, handle);
    0
}
