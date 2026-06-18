//! kernel32.dll — Memory management.
//!
//! Maps: VirtualAlloc, VirtualFree, HeapCreate/Alloc/Free.

#![allow(dead_code)]

/// VirtualAlloc — allocate virtual memory.
#[no_mangle]
pub extern "C" fn VirtualAlloc(addr: u64, size: u64, alloc_type: u32, protect: u32) -> u64 {
    let _ = (addr, size, alloc_type, protect);
    // TODO: map to BMO mmap syscall
    crate::diag::info_u64("wcompat::k32", "VirtualAlloc stub", size);
    0
}

/// VirtualFree — free virtual memory.
#[no_mangle]
pub extern "C" fn VirtualFree(addr: u64, size: u64, free_type: u32) -> u64 {
    let _ = (addr, size, free_type);
    // TODO: map to BMO munmap syscall
    0
}

/// VirtualProtect — change memory protection.
#[no_mangle]
pub extern "C" fn VirtualProtect(addr: u64, size: u64, new_prot: u32, old_prot: *mut u32) -> u64 {
    let _ = (addr, size, new_prot, old_prot);
    1 // TRUE = success
}
