use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use core::ptr::addr_of_mut;
use cabina_core::Event;

#[cfg(not(test))]
pub const MAX_EVENTS: usize = 256;
#[cfg(test)]
pub const MAX_EVENTS: usize = 16;

static mut EVENTS: [Event; MAX_EVENTS] = [Event::ZERO; MAX_EVENTS];
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

pub fn init() {}

pub fn push(event: &Event) -> u64 {
    acquire();
    let seq = NEXT_SEQ.fetch_add(1, Ordering::Relaxed);
    unsafe {
        let slot = (seq as usize - 1) % MAX_EVENTS;
        let base = addr_of_mut!(EVENTS) as *mut Event;
        core::ptr::write_volatile(base.add(slot), *event);
        (*base.add(slot)).seq = seq;
    }
    release();
    seq
}

pub fn event_by_seq(seq: u64) -> Option<Event> {
    if seq == 0 { return None; }
    unsafe {
        let slot = (seq as usize - 1) % MAX_EVENTS;
        let base = addr_of_mut!(EVENTS) as *const Event;
        let ev = core::ptr::read_volatile(base.add(slot));
        if ev.seq == seq { Some(ev) } else { None }
    }
}

pub fn next_seq() -> u64 {
    NEXT_SEQ.load(Ordering::Relaxed)
}

pub fn last(n: usize) -> alloc::vec::Vec<Event> {
    let cur = NEXT_SEQ.load(Ordering::Relaxed);
    let start = if cur > (n as u64) { cur - (n as u64) } else { 1 };
    let mut out = alloc::vec::Vec::with_capacity(n);
    for seq in start..cur {
        if let Some(ev) = event_by_seq(seq) {
            out.push(ev);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use cabina_core::{Severity, Layer, Entity};

    #[test]
    fn push_and_read_back() {
        init();
        let ev = Event::new(Severity::Info, Layer::Ring0, Entity::Module, "test", 0, "hello", 42);
        push(&ev);
        let seq = next_seq() - 1;
        let read = event_by_seq(seq).unwrap();
        assert_eq!(read.module_str(), "test");
        assert_eq!(read.msg_str(), "hello");
        assert_eq!(read.seq, seq);
    }

    #[test]
    fn last_events() {
        init();
        for i in 0..10 {
            let ev = Event::new(Severity::Info, Layer::Ring0, Entity::Module, "buf", 0, &i.to_string(), i);
            push(&ev);
        }
        let last5 = last(5);
        assert_eq!(last5.len(), 5);
    }

    #[test]
    fn buffer_wraparound() {
        init();
        for i in 0..MAX_EVENTS + 10 {
            let ev = Event::new(Severity::Info, Layer::Ring0, Entity::Module, "wrap", 0, &i.to_string(), i as u64);
            push(&ev);
        }
        let seq = next_seq() - 1;
        let ev = event_by_seq(seq).unwrap();
        assert!(ev.msg_str().len() > 0);
    }

    #[test]
    fn empty_buffer() {
        // Reset the sequence to simulate empty buffer
        NEXT_SEQ.store(1, Ordering::Relaxed);
        let last0 = last(5);
        assert_eq!(last0.len(), 0);
    }
}
