//! Keyboard driver — stub.
//!
//! In the Ring 0 base, no keyboard input is wired. The stub returns
//! `None` to indicate an empty queue. When USB HID or PS/2 is wired,
//! this will return `Some(scancode)`.

use core::sync::atomic::{AtomicU8, Ordering};

/// A tiny ring buffer for scancodes. In the stub it's always empty.
static QUEUE: [AtomicU8; 32] = {
    const Z: AtomicU8 = AtomicU8::new(0);
    [Z; 32]
};
static HEAD: AtomicU8 = AtomicU8::new(0);
static TAIL: AtomicU8 = AtomicU8::new(0);

pub fn init() {}

/// Push a scancode (called from the PS/2 or HID driver).
#[allow(dead_code)]
pub fn push_scancode(sc: u8) {
    let h = HEAD.load(Ordering::Relaxed);
    let next = (h + 1) % 32;
    if next == TAIL.load(Ordering::Relaxed) { return; } // full
    QUEUE[h as usize].store(sc, Ordering::Relaxed);
    HEAD.store(next, Ordering::Relaxed);
}

/// Pop the next scancode. Returns `None` if the queue is empty.
pub fn pop_scancode() -> Option<u8> {
    let t = TAIL.load(Ordering::Relaxed);
    if t == HEAD.load(Ordering::Relaxed) { return None; }
    let sc = QUEUE[t as usize].load(Ordering::Relaxed);
    TAIL.store((t + 1) % 32, Ordering::Relaxed);
    Some(sc)
}
