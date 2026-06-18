//! ntdll — Object management (NT level).

#![allow(dead_code)]

/// NtDuplicateObject — duplicate a handle.
#[no_mangle]
pub extern "C" fn NtDuplicateObject(
    _source_process: u64, _source_handle: u64,
    _target_process: u64, target_handle: *mut u64,
    _desired_access: u32, _handle_attributes: u32, _options: u32,
) -> i32 {
    if !target_handle.is_null() {
        unsafe { *target_handle = 0; }
    }
    0 // STATUS_SUCCESS
}

/// NtQueryObject — query object information.
#[no_mangle]
pub extern "C" fn NtQueryObject(
    _handle: u64, _info_class: u32, _buffer: u64, _length: u32, _return_length: *mut u32,
) -> i32 {
    0xC0000002u32 as i32 // STATUS_NOT_IMPLEMENTED
}

/// NtSetInformationObject — set object information.
#[no_mangle]
pub extern "C" fn NtSetInformationObject(
    _handle: u64, _info_class: u32, _buffer: u64, _length: u32,
) -> i32 {
    0xC0000002u32 as i32
}
