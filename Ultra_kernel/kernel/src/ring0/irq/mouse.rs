//! Mouse driver — stub.
//!
//! PS/2 mouse (or USB HID mouse) deferred. Returns `None` to indicate
//! no event pending.

use core::sync::atomic::{AtomicU32, Ordering};

/// Returns the next mouse event as a packed `u32`:
///
/// bits  0..8   = dx (signed low byte)
///
/// bits  8..16  = dy (signed low byte)
///
/// bits 16..24  = button state (bit 0 = left, bit 1 = right, bit 2 = middle)
///
/// bits 24..32  = reserved
static LAST_EVENT: AtomicU32 = AtomicU32::new(0);
static HAS_EVENT: AtomicU32 = AtomicU32::new(0);

pub fn init() {}

#[allow(dead_code)]
pub fn push_event(dx: i8, dy: i8, buttons: u8) {
    let ev = ((dx as u32) & 0xFF)
           | (((dy as u32) & 0xFF) << 8)
           | (((buttons as u32) & 0xFF) << 16);
    LAST_EVENT.store(ev, Ordering::Relaxed);
    HAS_EVENT.store(1, Ordering::Release);
}

pub fn take_legacy() -> Option<u32> {
    if HAS_EVENT.swap(0, Ordering::AcqRel) == 0 { None }
    else { Some(LAST_EVENT.load(Ordering::Relaxed)) }
}
