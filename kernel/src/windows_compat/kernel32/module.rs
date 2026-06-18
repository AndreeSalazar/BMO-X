//! kernel32.dll — Module management (LoadLibrary, GetProcAddress).

#![allow(dead_code)]

/// GetModuleHandleA — get module handle by name (ASCII).
#[no_mangle]
pub extern "C" fn GetModuleHandleA(_name: u64) -> u64 {
    // Return handle to main executable
    0x10000 // TODO: real module handle
}

/// LoadLibraryA — load a DLL (ASCII).
#[no_mangle]
pub extern "C" fn LoadLibraryA(_name: u64) -> u64 {
    // TODO: implement DLL loading
    0
}

/// GetProcAddress — get function address from module.
#[no_mangle]
pub extern "C" fn GetProcAddress(_module: u64, _name: u64) -> u64 {
    // TODO: implement function lookup
    0
}

/// FreeLibrary — unload a DLL.
#[no_mangle]
pub extern "C" fn FreeLibrary(_module: u64) -> u64 {
    1
}
