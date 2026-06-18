//! user32.dll — Input handling.

#![allow(dead_code)]

/// GetKeyboardState — get keyboard state for all keys.
#[no_mangle]
pub extern "C" fn GetKeyboardState(_state: u64) -> u64 { 1 }

/// GetKeyState — get state of a virtual key.
#[no_mangle]
pub extern "C" fn GetKeyState(_vk: i32) -> i16 { 0 }

/// GetAsyncKeyState — get async key state.
#[no_mangle]
pub extern "C" fn GetAsyncKeyState(_vk: i32) -> i16 { 0 }

/// MapVirtualKeyA — map a virtual key to a scan code.
#[no_mangle]
pub extern "C" fn MapVirtualKeyA(_vk: u32, _map_type: u32) -> u32 { 0 }

/// LoadCursorA — load a cursor resource.
#[no_mangle]
pub extern "C" fn LoadCursorA(_instance: u64, _name: u64) -> u64 { 1 }

/// LoadCursorW — load a cursor resource (UTF-16).
#[no_mangle]
pub extern "C" fn LoadCursorW(_instance: u64, _name: u64) -> u64 { 1 }

/// SetCursorPos — set cursor position.
#[no_mangle]
pub extern "C" fn SetCursorPos(_x: i32, _y: i32) -> u64 { 1 }

/// GetCursorPos — get cursor position.
#[no_mangle]
pub extern "C" fn GetCursorPos(pos: u64) -> u64 {
    if pos != 0 {
        unsafe {
            *(pos as *mut (i32, i32)) = (0, 0);
        }
    }
    1
}
