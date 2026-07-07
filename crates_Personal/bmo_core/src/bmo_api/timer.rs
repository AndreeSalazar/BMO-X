//! v2.0 — Timer wheel (linear scan, ~1 ms resolution).
//!
//! TSC calibrado via CPUID o estimación de ~3 GHz para Ryzen 5 5600X.
//! Capacidad 1024 timers.

#![allow(dead_code)]

use super::window::WID_INVALID;
use super::message::{BmoMsg, BmoMsgKind};

pub const MAX_TIMERS: usize = 1024;

#[derive(Debug, Clone, Copy)]
pub struct TimerEntry {
    pub used: bool,
    pub id: u32,
    pub target_window: u32,
    pub target_tid: u16,
    pub expiration_ms: u64,
    pub interval_ms: u32,
    pub in_flight: bool,
}

impl TimerEntry {
    pub const fn empty() -> Self {
        Self {
            used: false, id: 0,
            target_window: WID_INVALID, target_tid: 0,
            expiration_ms: 0, interval_ms: 0,
            in_flight: false,
        }
    }
}

pub struct TimerWheel {
    pub timers: [TimerEntry; MAX_TIMERS],
    pub now_ms: u64,
    pub next_id: u32,
    /// TSC ticks per millisecond (calibrado en init).
    pub tsc_per_ms: u64,
}

impl TimerWheel {
    pub const fn new() -> Self {
        const T: TimerEntry = TimerEntry::empty();
        Self {
            timers: [T; MAX_TIMERS],
            now_ms: 0,
            next_id: 1,
            tsc_per_ms: 3_000_000,
        }
    }

    pub fn init(&mut self) {
        for t in self.timers.iter_mut() { *t = TimerEntry::empty(); }
        self.now_ms = 0;
        self.next_id = 1;
        self.tsc_per_ms = calibrate_tsc();
    }

    pub fn alloc(&mut self, target_window: u32, target_tid: u16,
                 expiration_ms: u64, interval_ms: u32) -> Option<(u32, u32)> {
        for (i, t) in self.timers.iter_mut().enumerate() {
            if !t.used {
                t.used = true;
                t.id = self.next_id;
                t.target_window = target_window;
                t.target_tid = target_tid;
                t.expiration_ms = expiration_ms;
                t.interval_ms = interval_ms;
                t.in_flight = false;
                self.next_id = self.next_id.wrapping_add(1);
                return Some((i as u32, t.id));
            }
        }
        None
    }

    pub fn free(&mut self, slot: u32) -> bool {
        if let Some(t) = self.timers.get_mut(slot as usize) {
            if !t.used { return false; }
            t.used = false;
            true
        } else { false }
    }

    pub fn free_by_id(&mut self, pub_id: u32) -> bool {
        for t in self.timers.iter_mut() {
            if t.used && t.id == pub_id {
                t.used = false;
                return true;
            }
        }
        false
    }

    pub fn tick(&mut self) {
        let now = self.now_ms;
        for i in 0..MAX_TIMERS {
            let t = self.timers[i];
            if !t.used || t.in_flight { continue; }
            if t.expiration_ms <= now {
                self.timers[i].in_flight = true;
                let qtid = t.target_tid;
                let qslot = super::queue::queue_table().slot_for_tid(qtid);
                if let Some(qs) = qslot {
                    let msg = BmoMsg::new(BmoMsgKind::Timer, t.target_window as u16, 0, t.id as u64, 0);
                    let _ = super::event::post_coalesced(
                        &mut super::queue::queue_table().queues[qs as usize], msg);
                }
                if t.interval_ms > 0 {
                    self.timers[i].expiration_ms = now + t.interval_ms as u64;
                    self.timers[i].in_flight = false;
                } else {
                    self.timers[i].used = false;
                }
            }
        }
    }
}

fn calibrate_tsc() -> u64 {
    let base = crate::cpu::rdtsc();
    let mut _count = 0u64;
    for _ in 0..1_000_000u32 {
        core::hint::spin_loop();
        _count += 1;
    }
    let elapsed = crate::cpu::rdtsc().wrapping_sub(base);
    let per_ms = elapsed / 10;
    if per_ms > 1_000_000 && per_ms < 10_000_000 {
        per_ms
    } else {
        3_000_000
    }
}

pub fn tick_global() {
    let s = super::state();
    s.lock();
    let tsc = crate::cpu::rdtsc();
    let per_ms = s.timers.tsc_per_ms;
    s.timers.now_ms = if per_ms > 0 { tsc / per_ms } else { tsc / 3_000_000 };
    s.timers.tick();
    s.unlock();
}
