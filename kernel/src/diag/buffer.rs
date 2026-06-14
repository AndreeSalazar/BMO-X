//! Caja negra circular en RAM para diag/.

use super::event::Event;

pub const MAX_EVENTS: usize = 64;

static mut EVENTS: [Event; MAX_EVENTS] = [Event::empty(); MAX_EVENTS];
static mut NEXT_SEQ: u64 = 1;

pub(crate) fn push(mut event: Event) -> Event {
    unsafe {
        event.seq = NEXT_SEQ;
        NEXT_SEQ = NEXT_SEQ.wrapping_add(1).max(1);
        EVENTS[(event.seq as usize - 1) % MAX_EVENTS] = event;
    }
    event
}

pub(crate) fn event_by_seq(seq: u64) -> Option<Event> {
    if seq == 0 { return None; }
    let ev = unsafe { EVENTS[(seq as usize - 1) % MAX_EVENTS] };
    if ev.seq == seq { Some(ev) } else { None }
}

pub(crate) fn next_seq() -> u64 {
    unsafe { NEXT_SEQ }
}
