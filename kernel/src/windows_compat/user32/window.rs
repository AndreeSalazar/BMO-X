//! user32.dll — Window management.

#![allow(dead_code)]

/// RegisterClassExA — register a window class.
#[no_mangle]
pub extern "C" fn RegisterClassExA(_wndclass: u64) -> u16 {
    // TODO: implement window class registration
    1 // Atom (non-zero = success)
}

/// RegisterClassExW — register a window class (UTF-16).
#[no_mangle]
pub extern "C" fn RegisterClassExW(_wndclass: u64) -> u16 { 1 }

/// CreateWindowExA — create a window.
#[no_mangle]
pub extern "C" fn CreateWindowExA(
    _ex_style: u32, _class: u64, _title: u64, _style: u32,
    _x: i32, _y: i32, _w: i32, _h: i32,
    _parent: u64, _menu: u64, _instance: u64, _param: u64,
) -> u64 {
    // TODO: implement window creation via BMO desktop
    crate::diag::info("wcompat::u32", "CreateWindowExA stub");
    1 // HWND (non-zero = success)
}

/// CreateWindowExW — create a window (UTF-16).
#[no_mangle]
pub extern "C" fn CreateWindowExW(
    _ex_style: u32, _class: u64, _title: u64, _style: u32,
    _x: i32, _y: i32, _w: i32, _h: i32,
    _parent: u64, _menu: u64, _instance: u64, _param: u64,
) -> u64 { 1 }

/// DestroyWindow — destroy a window.
#[no_mangle]
pub extern "C" fn DestroyWindow(_hwnd: u64) -> u64 { 1 }

/// ShowWindow — show/hide a window.
#[no_mangle]
pub extern "C" fn ShowWindow(_hwnd: u64, _cmd: i32) -> u64 { 1 }

/// UpdateWindow — update a window.
#[no_mangle]
pub extern "C" fn UpdateWindow(_hwnd: u64) -> u64 { 1 }

/// DefWindowProcA — default window procedure.
#[no_mangle]
pub extern "C" fn DefWindowProcA(_hwnd: u64, _msg: u32, _wparam: u64, _lparam: u64) -> u64 { 0 }

/// DefWindowProcW — default window procedure (UTF-16).
#[no_mangle]
pub extern "C" fn DefWindowProcW(_hwnd: u64, _msg: u32, _wparam: u64, _lparam: u64) -> u64 { 0 }

/// MessageBoxA — display a message box.
#[no_mangle]
pub extern "C" fn MessageBoxA(_hwnd: u64, _text: u64, _caption: u64, _style: u32) -> i32 {
    crate::diag::info("wcompat::u32", "MessageBoxA stub");
    1 // IDOK
}

/// MessageBoxW — display a message box (UTF-16).
#[no_mangle]
pub extern "C" fn MessageBoxW(_hwnd: u64, _text: u64, _caption: u64, _style: u32) -> i32 { 1 }
