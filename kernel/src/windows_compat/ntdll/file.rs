//! ntdll — File I/O (NT level).

#![allow(dead_code)]

pub use super::syscalls::{NtCreateFile, NtReadFile, NtWriteFile, NtClose};

/// ZwCreateFile alias.
#[no_mangle]
pub extern "C" fn ZwCreateFile(
    file_handle: *mut u64,
    desired_access: u32,
    object_attributes: *const super::ObjectAttributes,
    io_status_block: *mut super::IoStatusBlock,
    allocation_size: *const super::LargeInteger,
    file_attributes: u32,
    share_access: u32,
    create_disposition: u32,
    create_options: u32,
    ea_buffer: u64,
    ea_length: u32,
) -> i32 {
    NtCreateFile(file_handle, desired_access, object_attributes, io_status_block,
                 allocation_size, file_attributes, share_access, create_disposition,
                 create_options, ea_buffer, ea_length)
}

/// ZwReadFile alias.
#[no_mangle]
pub extern "C" fn ZwReadFile(
    file_handle: u64, event: u64, apc_routine: u64, apc_context: u64,
    io_status_block: *mut super::IoStatusBlock, buffer: u64, length: u32,
    byte_offset: *const super::LargeInteger, key: *mut u32,
) -> i32 {
    NtReadFile(file_handle, event, apc_routine, apc_context, io_status_block,
               buffer, length, byte_offset, key)
}

/// ZwWriteFile alias.
#[no_mangle]
pub extern "C" fn ZwWriteFile(
    file_handle: u64, event: u64, apc_routine: u64, apc_context: u64,
    io_status_block: *mut super::IoStatusBlock, buffer: u64, length: u32,
    byte_offset: *const super::LargeInteger, key: *mut u32,
) -> i32 {
    NtWriteFile(file_handle, event, apc_routine, apc_context, io_status_block,
                buffer, length, byte_offset, key)
}

/// ZwClose alias.
#[no_mangle]
pub extern "C" fn ZwClose(handle: u64) -> i32 {
    NtClose(handle)
}
