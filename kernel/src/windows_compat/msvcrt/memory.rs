//! msvcrt.dll — C memory allocation (malloc, free, realloc).

#![allow(dead_code)]

extern crate alloc;

/// malloc — allocate memory.
#[no_mangle]
pub extern "C" fn malloc(size: u64) -> u64 {
    if size == 0 { return 0; }
    unsafe {
        let layout = core::alloc::Layout::from_size_align(size as usize, 8).unwrap_or_else(|_| {
            core::alloc::Layout::from_size_align(1, 1).unwrap()
        });
        alloc::alloc::alloc(layout) as u64
    }
}

/// free — free memory.
#[no_mangle]
pub extern "C" fn free(ptr: u64) {
    if ptr == 0 { return; }
    // TODO: track allocation sizes for proper dealloc
    // For now, this is a no-op (memory leak)
    let _ = ptr;
}

/// realloc — reallocate memory.
#[no_mangle]
pub extern "C" fn realloc(ptr: u64, new_size: u64) -> u64 {
    if ptr == 0 { return malloc(new_size); }
    if new_size == 0 {
        free(ptr);
        return 0;
    }
    // TODO: proper realloc implementation
    let new_ptr = malloc(new_size);
    if new_ptr != 0 {
        unsafe {
            core::ptr::copy_nonoverlapping(ptr as *const u8, new_ptr as *mut u8, new_size as usize);
        }
    }
    new_ptr
}

/// calloc — allocate zero-initialized memory.
#[no_mangle]
pub extern "C" fn calloc(count: u64, size: u64) -> u64 {
    let total = count * size;
    let ptr = malloc(total);
    if ptr != 0 {
        unsafe {
            core::ptr::write_bytes(ptr as *mut u8, 0, total as usize);
        }
    }
    ptr
}
