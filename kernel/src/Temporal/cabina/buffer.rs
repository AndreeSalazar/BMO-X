//! `cabina::buffer` — Caja negra circular en RAM (blackbox 256 eventos).
//!
//! Thread-safe usando AtomicU64 para el sequence counter y un spinlock
//! para proteger el slot. Los eventos se escriben por índice `(seq-1) % MAX`.
//!
//! v1.8.8: adaptado desde `bmo_core::diag::buffer` para usar la API
//! moderna de `cabina::event` (campos: seq, tick_ns, severity, layer,
//! entity, entity_id, module, msg, value).

#![allow(dead_code)]

use super::event::Event;
use core::sync::atomic::{AtomicU64, AtomicBool, Ordering};

pub const MAX_EVENTS: usize = 256;

use core::mem::MaybeUninit;

#[repr(transparent)]
struct SyncSlot(MaybeUninit<Event>);
unsafe impl Sync for SyncSlot {}

const SLOT: SyncSlot = SyncSlot(MaybeUninit::uninit());
static EVENTS: [SyncSlot; MAX_EVENTS] = [SLOT; MAX_EVENTS];
static NEXT_SEQ: AtomicU64 = AtomicU64::new(1);
static LOCKED: AtomicBool = AtomicBool::new(false);

/// Placeholder para inicializar el array estático.
const EMPTY: Event = Event::empty();

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

/// Inicializa la blackbox. (No-op en v1.8.8.)
pub fn init() {}

/// Empuja un evento a la blackbox. Le asigna `seq` y guarda por valor.
pub fn push(event: &Event) {
    acquire();
    let seq = NEXT_SEQ.fetch_add(1, Ordering::Relaxed);
    let mut e = Event::empty();
    e.seq = seq;
    e.severity = event.severity;
    e.layer = event.layer;
    e.entity = event.entity;
    e.entity_id = event.entity_id;
    e.module = event.module.clone();
    e.msg = event.msg.clone();
    e.value = event.value;
    e.tick_ns = event.tick_ns;
    unsafe {
        let slot = ((seq as usize) - 1) % MAX_EVENTS;
        let p = EVENTS[slot].0.as_ptr() as *mut Event;
        core::ptr::write_volatile(p, e);
    }
    release();
}

/// Busca un evento por su `seq`.
pub fn event_by_seq(seq: u64) -> Option<Event> {
    if seq == 0 { return None; }
    let ev = unsafe {
        let p = EVENTS[((seq as usize) - 1) % MAX_EVENTS].0.as_ptr();
        core::ptr::read_volatile(p)
    };
    if ev.seq == seq { Some(ev) } else { None }
}

/// Devuelve el próximo `seq` que se asignará.
pub fn next_seq() -> u64 {
    NEXT_SEQ.load(Ordering::Relaxed)
}

/// Devuelve los últimos `n` eventos (en orden cronológico).
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
