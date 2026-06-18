//! user32.dll — Message handling.

#![allow(dead_code)]

/// MSG structure — 48 bytes on x86-64.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Msg {
    pub hwnd: u64,
    pub message: u32,
    pub _pad: u32,
    pub wparam: u64,
    pub lparam: u64,
    pub time: u32,
    pub pt_x: i32,
    pub pt_y: i32,
}

static mut MSG_QUEUE: [Msg; 64] = [Msg {
    hwnd: 0, message: 0, _pad: 0, wparam: 0, lparam: 0,
    time: 0, pt_x: 0, pt_y: 0,
}; 64];
static mut MSG_COUNT: u32 = 0;
static mut MSG_HEAD: u32 = 0;

/// GetMessageA — get a message from the queue.
#[no_mangle]
pub extern "C" fn GetMessageA(msg: u64, _hwnd: u64, _min: u32, _max: u32) -> i32 {
    if msg == 0 { return 0; }
    // TODO: implement real message queue
    unsafe {
        let m = &mut *(msg as *mut Msg);
        // WM_QUIT = 0x0012
        if MSG_COUNT > 0 {
            *m = MSG_QUEUE[MSG_HEAD as usize];
            MSG_HEAD = (MSG_HEAD + 1) % 64;
            MSG_COUNT -= 1;
            1
        } else {
            // Block until message
            m.message = 0x0012; // WM_QUIT
            0
        }
    }
}

/// PeekMessageA — peek at a message without removing it.
#[no_mangle]
pub extern "C" fn PeekMessageA(msg: u64, _hwnd: u64, _min: u32, _max: u32, remove: u32) -> u64 {
    let _ = (msg, remove);
    0
}

/// TranslateMessage — translate virtual-key messages.
#[no_mangle]
pub extern "C" fn TranslateMessage(_msg: u64) -> u64 { 0 }

/// DispatchMessageA — dispatch a message to a window procedure.
#[no_mangle]
pub extern "C" fn DispatchMessageA(_msg: u64) -> u64 { 0 }

/// PostQuitMessage — post WM_QUIT to the message queue.
#[no_mangle]
pub extern "C" fn PostQuitMessage(_exit_code: i32) {
    unsafe {
        if MSG_COUNT < 64 {
            MSG_QUEUE[(MSG_HEAD + MSG_COUNT) as usize].message = 0x0012;
            MSG_COUNT += 1;
        }
    }
}

/// PostMessageA — post a message to a window.
#[no_mangle]
pub extern "C" fn PostMessageA(_hwnd: u64, _msg: u32, _wparam: u64, _lparam: u64) -> u64 { 1 }

/// SendMessageA — send a message to a window.
#[no_mangle]
pub extern "C" fn SendMessageA(_hwnd: u64, _msg: u32, _wparam: u64, _lparam: u64) -> u64 { 0 }
