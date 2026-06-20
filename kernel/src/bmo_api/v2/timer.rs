//! v2.0 — Timer wheel jerárquico (4 niveles × 256 buckets, ~1 ms).
//!
//! Para v2.0 simplificamos a una lista lineal de timers con tick
//! resolution de 1 ms y capacidad 1024.

#![allow(dead_code)]

use super::window::WID_INVALID;
use super::message::{BmoMsg, BmoMsgKind};

pub const MAX_TIMERS: usize = 1024;

#[derive(Debug, Clone, Copy)]
pub struct TimerEntry {
    pub used: bool,
    pub id: u32,         // identificador público (≤ 16 bits)
    pub target_window: u32,
    pub target_tid: u16,
    pub expiration_ms: u64, // timestamp en ms cuando expira
    pub interval_ms: u32,   // 0 = one-shot; >0 = repeating
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
}

impl TimerWheel {
    pub const fn new() -> Self {
        const T: TimerEntry = TimerEntry::empty();
        Self { timers: [T; MAX_TIMERS], now_ms: 0, next_id: 1 }
    }

    pub fn init(&mut self) {
        for t in self.timers.iter_mut() { *t = TimerEntry::empty(); }
        self.now_ms = 0;
        self.next_id = 1;
    }

    /// Reserva un timer. Devuelve (slot, public_id).
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

    /// Llamado desde el tick global. Procesa todos los timers que
    /// hayan expirado y los postee como BMO_MSG_TIMER.
    pub fn tick(&mut self) {
        let now = self.now_ms;
        for i in 0..MAX_TIMERS {
            let t = self.timers[i];
            if !t.used || t.in_flight { continue; }
            if t.expiration_ms <= now {
                // Marca in_flight para evitar reentrancia.
                self.timers[i].in_flight = true;
                let qtid = t.target_tid;
                let qslot = super::queue::queue_table().slot_for_tid(qtid);
                if let Some(qs) = qslot {
                    let msg = BmoMsg::new(BmoMsgKind::Timer, t.target_window as u16, 0, t.id as u64, 0);
                    let _ = super::event::post_coalesced(
                        &mut super::queue::queue_table().queues[qs as usize], msg);
                }
                // Repeating → reagenda; one-shot → libera.
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

/// Helper: tick global llamado desde `BmoState::tick`.
pub fn tick_global() {
    let s = super::state();
    s.lock();
    // now_ms desde TSC: rdtsc / 3_000_000 (asumiendo 3 GHz).
    let tsc = crate::arch::cpu::rdtsc();
    s.timers.now_ms = tsc / 3_000_000;
    s.timers.tick();
    s.unlock();
}
