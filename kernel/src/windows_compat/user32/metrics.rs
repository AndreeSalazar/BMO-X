//! user32.dll — System metrics and GDI bridge.

#![allow(dead_code)]

/// GetSystemMetrics — get system metric value.
#[no_mangle]
pub extern "C" fn GetSystemMetrics(index: i32) -> i32 {
    match index {
        0 => 1920,  // SM_CXSCREEN — screen width
        1 => 1080,  // SM_CYSCREEN — screen height
        5 => 1920,  // SM_CXVSCROLL
        6 => 1080,  // SM_CYHSCROLL
        8 => 48,    // SM_CYCAPTION
        13 => 1920, // SM_CXMAXimized
        14 => 1080, // SM_CYMAXIMIZED
        _ => 0,
    }
}

/// GetDC — get device context for a window.
#[no_mangle]
pub extern "C" fn GetDC(_hwnd: u64) -> u64 {
    // TODO: return framebuffer DC
    1
}

/// ReleaseDC — release a device context.
#[no_mangle]
pub extern "C" fn ReleaseDC(_hwnd: u64, _dc: u64) -> i32 { 1 }

/// BeginPaint — begin painting a window.
#[no_mangle]
pub extern "C" fn BeginPaint(_hwnd: u64, paint: u64) -> u64 {
    if paint != 0 {
        unsafe {
            let p = &mut *(paint as *mut PaintStruct);
            p.hdc = 1;
            p.f_erasing = 1;
            p.rc_left = 0;
            p.rc_top = 0;
            p.rc_right = 1920;
            p.rc_bottom = 1080;
        }
    }
    1
}

/// EndPaint — end painting a window.
#[no_mangle]
pub extern "C" fn EndPaint(_hwnd: u64, _paint: u64) -> u64 { 1 }

/// InvalidateRect — invalidate a window rectangle.
#[no_mangle]
pub extern "C" fn InvalidateRect(_hwnd: u64, _rect: u64, _erase: u32) -> u64 { 1 }

/// PAINTSTRUCT — 40 bytes on x86-64.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct PaintStruct {
    pub hdc: u64,
    pub f_erasing: u32,
    pub _pad: u32,
    pub rc_left: i32,
    pub rc_top: i32,
    pub rc_right: i32,
    pub rc_bottom: i32,
    pub _reserved: [u8; 16],
}
