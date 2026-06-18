//! shell32.dll compatibility — Shell operations, paths.

#![allow(dead_code)]

/// SHGetFolderPathA — get folder path (e.g., AppData, Desktop).
#[no_mangle]
pub extern "C" fn SHGetFolderPathA(
    _hwnd: u64, _csidl: i32, _token: u64, _flags: u32, _path: u64,
) -> i32 {
    // Return a default path
    if _path != 0 {
        unsafe {
            let p = _path as *mut u8;
            let default = b"/home/user\0";
            core::ptr::copy_nonoverlapping(default.as_ptr(), p, default.len());
        }
    }
    0 // S_OK
}

/// ShellExecuteA — open a file or URL.
#[no_mangle]
pub extern "C" fn ShellExecuteA(
    _hwnd: u64, _operation: u64, _file: u64, _params: u64,
    _dir: u64, _show_cmd: i32,
) -> u64 {
    crate::diag::info("wcompat::shell", "ShellExecuteA stub");
    32 // ERROR_SUCCESS (HINSTANCE > 32)
}
