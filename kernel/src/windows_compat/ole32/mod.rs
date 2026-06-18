//! ole32.dll compatibility — COM/OLE.

#![allow(dead_code)]

/// CoInitialize — initialize COM library.
#[no_mangle]
pub extern "C" fn CoInitialize(_reserved: u64) -> i32 { 0 } // S_OK

/// CoInitializeEx — initialize COM with flags.
#[no_mangle]
pub extern "C" fn CoInitializeEx(_reserved: u64, _flags: u32) -> i32 { 0 }

/// CoUninitialize — uninitialize COM library.
#[no_mangle]
pub extern "C" fn CoUninitialize() {}

/// CoCreateInstance — create a COM object.
#[no_mangle]
pub extern "C" fn CoCreateInstance(
    _clsid: u64, _outer: u64, _ctx: u32, _iid: u64, _ppv: u64,
) -> i32 {
    crate::diag::info("wcompat::com", "CoCreateInstance stub");
    0x80004002 // E_NOINTERFACE
}

/// CoTaskMemAlloc — allocate COM task memory.
#[no_mangle]
pub extern "C" fn CoTaskMemAlloc(size: u64) -> u64 {
    crate::windows_compat::msvcrt::memory::malloc(size)
}

/// CoTaskMemFree — free COM task memory.
#[no_mangle]
pub extern "C" fn CoTaskMemFree(ptr: u64) {
    crate::windows_compat::msvcrt::memory::free(ptr);
}
