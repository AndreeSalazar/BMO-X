//! ntdll — Memory management (NT level).
//!
//! Thin wrappers around the BMO page allocator.

#![allow(dead_code)]

pub use crate::bmo_abi::interop::win32::ntdll_syscalls::{NtAllocateVirtualMemory, NtFreeVirtualMemory, NtProtectVirtualMemory};

/// NtAllocateVirtualMemory with MEM_COMMIT semantics.
#[no_mangle]
pub extern "C" fn ZwAllocateVirtualMemory(
    process_handle: u64,
    base_address: *mut u64,
    zero_bits: u64,
    region_size: *mut u64,
    allocation_type: u32,
    protect: u32,
) -> i32 {
    NtAllocateVirtualMemory(process_handle, base_address, zero_bits, region_size, allocation_type, protect)
}

/// NtFreeVirtualMemory alias.
#[no_mangle]
pub extern "C" fn ZwFreeVirtualMemory(
    process_handle: u64,
    base_address: *mut u64,
    region_size: *mut u64,
    free_type: u32,
) -> i32 {
    NtFreeVirtualMemory(process_handle, base_address, region_size, free_type)
}
