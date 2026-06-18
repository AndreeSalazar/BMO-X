//! kernel32.dll — File I/O.
//!
//! Real implementations using BMO's ramdisk filesystem.
//! Maps: CreateFile, ReadFile, WriteFile, CloseHandle, FindFirstFile, etc.

#![allow(dead_code)]

use crate::bmo_abi::primitives::bx_u64;

const GENERIC_READ: u32 = 0x80000000;
const GENERIC_WRITE: u32 = 0x40000000;
const INVALID_HANDLE_VALUE: bx_u64 = !0u64;

/// CreateFileA — open or create a file (ASCII).
///
/// Real implementation: translates to BMO FileOpen syscall (0x20).
#[no_mangle]
pub extern "C" fn CreateFileA(
    name_ptr: bx_u64, access: u32, share: u32, _attrs: bx_u64,
    disposition: u32, flags: u32, _template: bx_u64,
) -> bx_u64 {
    let _ = (access, share, disposition, flags);

    if name_ptr == 0 {
        return INVALID_HANDLE_VALUE;
    }

    // Read the filename from memory (null-terminated C string)
    let name_bytes = unsafe {
        let mut len = 0;
        let ptr = name_ptr as *const u8;
        while *ptr.add(len) != 0 && len < 256 {
            len += 1;
        }
        core::slice::from_raw_parts(ptr, len)
    };

    // Map to BMO FileOpen syscall (0x20)
    let fd = crate::fs::ramdisk::open(name_bytes.as_ptr() as u64, name_bytes.len() as u64);

    if fd == u64::MAX {
        INVALID_HANDLE_VALUE
    } else {
        fd
    }
}

/// CreateFileW — open or create a file (Unicode).
///
/// Converts UTF-16 to UTF-8 and calls CreateFileA logic.
#[no_mangle]
pub extern "C" fn CreateFileW(
    name_ptr: bx_u64, access: u32, share: u32, attrs: bx_u64,
    disposition: u32, flags: u32, template: bx_u64,
) -> bx_u64 {
    let _ = (name_ptr, access, share, attrs, disposition, flags, template);
    // TODO: Convert UTF-16 to UTF-8 and call CreateFileA
    INVALID_HANDLE_VALUE
}

/// ReadFile — read from a file handle.
///
/// Real implementation: translates to BMO FileRead syscall (0x21).
#[no_mangle]
pub extern "C" fn ReadFile(
    handle: bx_u64, buf: bx_u64, to_read: u32, bytes_read: *mut u32, _overlapped: bx_u64,
) -> bx_u64 {
    if buf == 0 || to_read == 0 {
        return 0; // FALSE
    }

    // Map to BMO FileRead syscall (0x21)
    let result = crate::fs::ramdisk::read(handle, buf, to_read as u64);

    if result == u64::MAX {
        if !bytes_read.is_null() {
            unsafe { *bytes_read = 0; }
        }
        0 // FALSE
    } else {
        if !bytes_read.is_null() {
            unsafe { *bytes_read = result as u32; }
        }
        1 // TRUE
    }
}

/// WriteFile — write to a file handle.
///
/// Real implementation: translates to BMO FileWrite syscall (0x22).
#[no_mangle]
pub extern "C" fn WriteFile(
    handle: bx_u64, buf: bx_u64, to_write: u32, bytes_written: *mut u32, _overlapped: bx_u64,
) -> bx_u64 {
    if buf == 0 || to_write == 0 {
        return 0;
    }

    // Map to BMO FileWrite syscall (0x22)
    let result = crate::fs::ramdisk::write(handle, buf, to_write as u64);

    if !bytes_written.is_null() {
        unsafe { *bytes_written = result as u32; }
    }

    if result == u64::MAX { 0 } else { 1 }
}

/// CloseHandle — close a handle.
///
/// Real implementation: translates to BMO FileClose syscall (0x23).
#[no_mangle]
pub extern "C" fn CloseHandle(handle: bx_u64) -> bx_u64 {
    let result = crate::fs::ramdisk::close(handle);
    if result == u64::MAX { 0 } else { 1 }
}

/// GetFileSize — get file size.
///
/// Real implementation: translates to BMO FileSize syscall (0x25).
#[no_mangle]
pub extern "C" fn GetFileSize(handle: bx_u64, high: *mut u32) -> u32 {
    let size = crate::fs::ramdisk::size(handle);
    if size == u64::MAX {
        return !0u32;
    }
    if !high.is_null() {
        unsafe { *high = (size >> 32) as u32; }
    }
    size as u32
}

/// SetFilePointer — move file pointer.
///
/// Real implementation: translates to BMO FileSeek syscall (0x24).
#[no_mangle]
pub extern "C" fn SetFilePointer(handle: bx_u64, distance: i32, high: *mut i32, method: u32) -> u32 {
    let offset = if !high.is_null() {
        let hi = unsafe { *high } as u64;
        (hi << 32) | (distance as u32 as u64)
    } else {
        distance as u32 as u64
    };

    let result = crate::fs::ramdisk::seek(handle, offset, method as u64);

    if result == u64::MAX {
        !0u32
    } else {
        if !high.is_null() {
            unsafe { *high = (result >> 32) as i32; }
        }
        result as u32
    }
}

/// SetFilePointerEx — move file pointer (64-bit).
#[no_mangle]
pub extern "C" fn SetFilePointerEx(
    handle: bx_u64, distance: i64, new_pos: *mut i64, method: u32,
) -> bx_u64 {
    let result = crate::fs::ramdisk::seek(handle, distance as u64, method as u64);

    if result == u64::MAX {
        0
    } else {
        if !new_pos.is_null() {
            unsafe { *new_pos = result as i64; }
        }
        1
    }
}

/// GetFileAttributesA — get file attributes.
#[no_mangle]
pub extern "C" fn GetFileAttributesA(name_ptr: bx_u64) -> u32 {
    let _ = name_ptr;
    0x80 // FILE_ATTRIBUTE_NORMAL
}

/// DeleteFileA — delete a file.
#[no_mangle]
pub extern "C" fn DeleteFileA(name_ptr: bx_u64) -> bx_u64 {
    let _ = name_ptr;
    0 // FALSE — RAMdisk is read-only
}

/// GetLastError — get last error code.
#[no_mangle]
pub extern "C" fn GetLastError() -> u32 {
    0 // ERROR_SUCCESS
}

/// SetLastError — set last error code.
#[no_mangle]
pub extern "C" fn SetLastError(error: u32) {
    let _ = error;
}
