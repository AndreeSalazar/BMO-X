//! Caja negra circular en RAM para diag/.
//!
//! Thread-safe usando AtomicU64 para el sequence counter.
//! Los eventos se escriben por slot index (seq % MAX).

use super::event::Event;
use core::sync::atomic::{AtomicU64, AtomicBool, Ordering};

pub const MAX_EVENTS: usize = 256;

static EVENTS: [Event; MAX_EVENTS] = [Event::empty(); MAX_EVENTS];
static NEXT_SEQ: AtomicU64 = AtomicU64::new(1);
static LOCKED: AtomicBool = AtomicBool::new(false);

fn acquire() {
    loop {
        match LOCKED.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed) {
            Ok(_) => return,
            Err(_) => core::hint::spin_loop(),
        }
    }
}

fn release() {
    LOCKED.store(false, Ordering::Release);
}

pub(crate) fn push(mut event: Event) -> Event {
    acquire();
    let seq = NEXT_SEQ.fetch_add(1, Ordering::Relaxed);
    event.seq = seq;
    unsafe {
        // Safety: acquires spinlock, single writer
        let slot = (seq as usize) % MAX_EVENTS;
        core::ptr::write_volatile(
            (&EVENTS[slot] as *const Event as *mut Event).add(0),
            event,
        );
    }
    release();
    event
}

pub(crate) fn event_by_seq(seq: u64) -> Option<Event> {
    if seq == 0 { return None; }
    let ev = unsafe {
        core::ptr::read_volatile(&EVENTS[(seq as usize - 1) % MAX_EVENTS])
    };
    if ev.seq == seq { Some(ev) } else { None }
}

pub(crate) fn next_seq() -> u64 {
    NEXT_SEQ.load(Ordering::Relaxed)
}
