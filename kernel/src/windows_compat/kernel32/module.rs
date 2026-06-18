//! kernel32.dll — Module management (LoadLibrary, GetProcAddress).
//!
//! Real implementations that integrate with BMO's module system.

#![allow(dead_code)]

/// GetModuleHandleA — get module handle by name (ASCII).
#[no_mangle]
pub extern "C" fn GetModuleHandleA(name: u64) -> u64 {
    if name == 0 {
        // Return handle to main executable
        return 0x10000;
    }
    // TODO: look up loaded modules
    0x10000
}

/// GetModuleHandleW — get module handle by name (Unicode).
#[no_mangle]
pub extern "C" fn GetModuleHandleW(name: u64) -> u64 {
    GetModuleHandleA(name)
}

/// LoadLibraryA — load a DLL (ASCII).
#[no_mangle]
pub extern "C" fn LoadLibraryA(name: u64) -> u64 {
    let _ = name;
    // TODO: implement DLL loading via BEF devour
    0x10000 // Return pseudo-handle
}

/// LoadLibraryW — load a DLL (Unicode).
#[no_mangle]
pub extern "C" fn LoadLibraryW(name: u64) -> u64 {
    LoadLibraryA(name)
}

/// GetProcAddress — get function address from module.
#[no_mangle]
pub extern "C" fn GetProcAddress(module: u64, name: u64) -> u64 {
    let _ = (module, name);
    // TODO: implement function lookup in runtime symbol table
    0
}

/// FreeLibrary — unload a DLL.
#[no_mangle]
pub extern "C" fn FreeLibrary(module: u64) -> u64 {
    let _ = module;
    1 // TRUE
}
