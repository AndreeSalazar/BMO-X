//! advapi32.dll compatibility — Registry, Security, Crypto.

#![allow(dead_code)]

/// RegOpenKeyExA — open a registry key.
#[no_mangle]
pub extern "C" fn RegOpenKeyExA(
    _parent: u64, _name: u64, _options: u32, _desired: u32, _result: *mut u64,
) -> i32 {
    // TODO: implement registry (map to config files)
    if !_result.is_null() { unsafe { *_result = 0; } }
    2 // ERROR_FILE_NOT_FOUND
}

/// RegQueryValueExA — query a registry value.
#[no_mangle]
pub extern "C" fn RegQueryValueExA(
    _key: u64, _name: u64, _reserved: *mut u32, _type: *mut u32,
    _data: *mut u8, _data_len: *mut u32,
) -> i32 {
    2 // ERROR_FILE_NOT_FOUND
}

/// RegCreateKeyExA — create or open a registry key.
#[no_mangle]
pub extern "C" fn RegCreateKeyExA(
    _parent: u64, _name: u64, _reserved: u32, _class: u64,
    _options: u32, _desired: u32, _attrs: u64,
    _result: *mut u64, _disposition: *mut u32,
) -> i32 {
    if !_result.is_null() { unsafe { *_result = 0; } }
    if !_disposition.is_null() { unsafe { *_disposition = 1; } } // REG_CREATED_NEW_KEY
    0 // ERROR_SUCCESS
}

/// RegSetValueExA — set a registry value.
#[no_mangle]
pub extern "C" fn RegSetValueExA(
    _key: u64, _name: u64, _reserved: u32, _type: u32,
    _data: *const u8, _data_len: u32,
) -> i32 { 0 }

/// RegCloseKey — close a registry key.
#[no_mangle]
pub extern "C" fn RegCloseKey(_key: u64) -> i32 { 0 }
