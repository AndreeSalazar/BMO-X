//! kernel32.dll — File I/O.
//!
//! Maps: CreateFile, ReadFile, WriteFile, CloseHandle, FindFirstFile, etc.

#![allow(dead_code)]

/// CreateFileA — open or create a file (ASCII).
#[no_mangle]
pub extern "C" fn CreateFileA(
    name: u64, access: u32, share: u32, _attrs: u64,
    disposition: u32, flags: u32, _template: u64,
) -> u64 {
    let _ = (access, share, disposition, flags);
    // TODO: translate to BMO FileOpen
    crate::diag::info_u64("wcompat::k32", "CreateFileA stub", name);
    !0u64 // INVALID_HANDLE_VALUE
}

/// ReadFile — read from a file handle.
#[no_mangle]
pub extern "C" fn ReadFile(
    handle: u64, buf: u64, to_read: u32, bytes_read: *mut u32, _overlapped: u64,
) -> u64 {
    let _ = (handle, buf, to_read, bytes_read);
    // TODO: map to BMO FileRead
    0 // FALSE = failure
}

/// WriteFile — write to a file handle.
#[no_mangle]
pub extern "C" fn WriteFile(
    handle: u64, buf: u64, to_write: u32, bytes_written: *mut u32, _overlapped: u64,
) -> u64 {
    let _ = (handle, buf, to_write, bytes_written);
    // TODO: map to BMO FileWrite
    0
}

/// CloseHandle — close a handle.
#[no_mangle]
pub extern "C" fn CloseHandle(handle: u64) -> u64 {
    let _ = handle;
    // TODO: map to BMO FileClose
    1
}

/// GetFileSize — get file size.
#[no_mangle]
pub extern "C" fn GetFileSize(handle: u64, _high: *mut u32) -> u32 {
    let _ = handle;
    // TODO: map to BMO FileSize
    0
}

/// SetFilePointer — move file pointer.
#[no_mangle]
pub extern "C" fn SetFilePointer(handle: u64, distance: i32, _high: *mut i32, method: u32) -> u32 {
    let _ = (handle, distance, method);
    // TODO: map to BMO FileSeek
    !0u32 // INVALID_SET_FILE_POINTER
}
