//! BMO Channel — Ring 0 side: timer ISR processing.
//!
//! On every timer tick, the ISR checks if any registered channel
//! has its doorbell set. If so, it processes pending submissions.
//!
//! ## Usage
//!
//! ```rust
//! // Ring 3 allocates a page, maps it shared
//! let ch = channel_ptr as *mut bmo_channel::Channel;
//! (*ch).init();
//!
//! // Ring 0 registers it
//! channel::register(ch);
//!
//! // Ring 3 submits events (no syscall!)
//! (*ch).ring3_send(1, keycode, pressed, 0);
//!
//! // Timer ISR processes them
//! channel::tick(); // called from idt.rs timer handler
//! ```

use bmo_channel::Channel;
use core::sync::atomic::{AtomicPtr, Ordering};

/// Maximum registered channels.
const MAX_CHANNELS: usize = 8;

/// Registered channels (raw pointers to shared pages).
static CHANNELS: [AtomicPtr<Channel>; MAX_CHANNELS] = [
    AtomicPtr::new(core::ptr::null_mut()),
    AtomicPtr::new(core::ptr::null_mut()),
    AtomicPtr::new(core::ptr::null_mut()),
    AtomicPtr::new(core::ptr::null_mut()),
    AtomicPtr::new(core::ptr::null_mut()),
    AtomicPtr::new(core::ptr::null_mut()),
    AtomicPtr::new(core::ptr::null_mut()),
    AtomicPtr::new(core::ptr::null_mut()),
];

/// Register a channel for timer ISR processing.
pub fn register(ch: *mut Channel) -> bool {
    for slot in &CHANNELS {
        if slot.compare_exchange(
            core::ptr::null_mut(),
            ch,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ).is_ok() {
            return true;
        }
    }
    false
}

/// Process all registered channels. Called from timer ISR.
/// Returns total number of entries processed.
pub fn tick_process_all() -> usize {
    let mut total = 0;
    for slot in &CHANNELS {
        let ch = slot.load(Ordering::Acquire);
        if ch.is_null() { continue; }
        let channel = unsafe { &*ch };
        if channel.ring0_has_work() {
            total += channel.ring0_process(|opcode, a0, a1, a2| {
                // Default handler: echo with opcode * 100 for testing
                Some((opcode + 100, a0, a1, a2))
            });
        }
    }
    total
}
