//! ntdll — RTL (Run-Time Library) functions.
//!
//! These are low-level utility functions used by all Windows code.

#![allow(dead_code)]

/// RtlAddFunctionTable — register unwind info for SEH.
#[no_mangle]
pub extern "C" fn RtlAddFunctionTable(
    _function_table: u64, _entry_count: u32, _base_address: u64,
) -> i32 {
    1 // TRUE
}

/// RtlDeleteFunctionTable — unregister unwind info.
#[no_mangle]
pub extern "C" fn RtlDeleteFunctionTable(_function_table: u64) -> i32 {
    1
}

/// RtlVirtualUnwind — unwind a stack frame.
#[no_mangle]
pub extern "C" fn RtlVirtualUnwind(
    _handler_type: u32, _base: u64, _pc: u64, _runtime_function: u64,
    _context: u64, _handler_data: u64, _establisher_frame: u64,
    _context_record: u64,
) -> u64 {
    0
}

/// RtlInitAnsiString — initialize ANSI_STRING from C string.
#[no_mangle]
pub extern "C" fn RtlInitAnsiString(dest: *mut AnsiString, source: *const u8) {
    if dest.is_null() { return; }
    if source.is_null() {
        unsafe {
            (*dest).length = 0;
            (*dest).maximum_length = 0;
            (*dest).buffer = 0;
        }
        return;
    }
    let len = unsafe { cstr_len(source) };
    unsafe {
        (*dest).length = len as u16;
        (*dest).maximum_length = (len + 1) as u16;
        (*dest).buffer = source as u64;
    }
}

/// RtlAnsiStringToUnicodeString — convert ANSI to Unicode.
#[no_mangle]
pub extern "C" fn RtlAnsiStringToUnicodeString(
    dest: *mut UnicodeString, source: *const AnsiString, allocate_dest: i32,
) -> i32 {
    let _ = (dest, source, allocate_dest);
    0 // STATUS_SUCCESS
}

/// RtlCompareMemory — compare two memory blocks.
#[no_mangle]
pub extern "C" fn RtlCompareMemory(source1: *const u8, source2: *const u8, length: u64) -> u64 {
    for i in 0..length as usize {
        unsafe {
            if *source1.add(i) != *source2.add(i) {
                return i as u64;
            }
        }
    }
    length
}

/// RtlZeroMemory — zero a memory block.
#[no_mangle]
pub extern "C" fn RtlZeroMemory(dest: *mut u8, length: u64) {
    unsafe {
        core::ptr::write_bytes(dest, 0, length as usize);
    }
}

/// RtlCopyMemory — copy memory.
#[no_mangle]
pub extern "C" fn RtlCopyMemory(dest: *mut u8, source: *const u8, length: u64) {
    unsafe {
        core::ptr::copy_nonoverlapping(source, dest, length as usize);
    }
}

/// RtlFillMemory — fill memory with a byte.
#[no_mangle]
pub extern "C" fn RtlFillMemory(dest: *mut u8, length: u64, fill: u8) {
    unsafe {
        core::ptr::write_bytes(dest, fill, length as usize);
    }
}

/// RtlMoveMemory — move memory (handles overlap).
#[no_mangle]
pub extern "C" fn RtlMoveMemory(dest: *mut u8, source: *const u8, length: u64) {
    unsafe {
        core::ptr::copy(source, dest, length as usize);
    }
}

fn cstr_len(ptr: *const u8) -> usize {
    let mut len = 0;
    unsafe {
        while *ptr.add(len) != 0 {
            len += 1;
        }
    }
    len
}

/// ANSI_STRING structure.
#[repr(C)]
pub struct AnsiString {
    pub length: u16,
    pub maximum_length: u16,
    pub buffer: u64,
}

/// UNICODE_STRING structure.
#[repr(C)]
pub struct UnicodeString {
    pub length: u16,
    pub maximum_length: u16,
    pub buffer: u64,
}
