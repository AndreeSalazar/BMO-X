//! v2.0 — Cola de mensajes per-thread (SPSC ring).
//!
//! Producer = kernel (push). Consumer = el thread (pop). Tamaño fijo 64.
//! En overflow el kernel setea `overflow_count` y descarta el mensaje.
//! Lock atómico con AtomicU8 + compare_exchange.

#![allow(dead_code)]

use super::message::BmoMsg;
use core::sync::atomic::{AtomicU8, Ordering};

pub const QUEUE_CAP: usize = 64;
pub const QUEUE_MAGIC: u32 = 0xC1A551FE;

/// Estado de la cola de un thread.
pub struct BmoQueue {
    pub magic: u32,
    pub head: u32,       // producer (kernel) index
    pub tail: u32,       // consumer (thread) index
    pub overflow: u32,   // mensajes descartados
    pub waiting: bool,   // thread bloqueado en get_message
    pub msgs: [BmoMsg; QUEUE_CAP],
}

impl BmoQueue {
    pub const fn new() -> Self {
        const EMPTY: BmoMsg = BmoMsg::null();
        Self {
            magic: QUEUE_MAGIC,
            head: 0,
            tail: 0,
            overflow: 0,
            waiting: false,
            msgs: [EMPTY; QUEUE_CAP],
        }
    }

    /// Push por el kernel. Devuelve `true` si se encoló, `false` si
    /// la cola estaba llena (en ese caso incrementa overflow).
    pub fn push(&mut self, msg: BmoMsg) -> bool {
        let h = self.head;
        let next = (h + 1) % (QUEUE_CAP as u32);
        if next == self.tail {
            self.overflow = self.overflow.wrapping_add(1);
            return false;
        }
        self.msgs[h as usize] = msg;
        self.head = next;
        true
    }

    /// Pop por el thread. Devuelve Some(msg) si hay, None si está vacía.
    pub fn pop(&mut self) -> Option<BmoMsg> {
        let t = self.tail;
        if t == self.head { return None; }
        let m = self.msgs[t as usize];
        self.tail = (t + 1) % (QUEUE_CAP as u32);
        Some(m)
    }

    /// Peek sin consumir.
    pub fn peek(&self) -> Option<&BmoMsg> {
        if self.tail == self.head { return None; }
        Some(&self.msgs[self.tail as usize])
    }

    pub fn is_empty(&self) -> bool { self.head == self.tail }
    pub fn len(&self) -> u32 {
        if self.head >= self.tail { self.head - self.tail }
        else { (QUEUE_CAP as u32) - self.tail + self.head }
    }
}

/// Tabla global de colas per-thread. v2.0: máximo 64 threads GUI.
pub const MAX_GUI_THREADS: usize = 64;

pub struct BmoQueueTable {
    pub queues: [BmoQueue; MAX_GUI_THREADS],
    /// Map thread_id → queue slot (0..MAX_GUI_THREADS) o 0xFFFF.
    pub tid_to_slot: [u16; MAX_GUI_THREADS],
    lock: AtomicU8,
}

impl BmoQueueTable {
    pub const fn new() -> Self {
        Self {
            queues: [const { BmoQueue::new() }; MAX_GUI_THREADS],
            tid_to_slot: [0xFFFF; MAX_GUI_THREADS],
            lock: AtomicU8::new(0),
        }
    }

    pub fn acquire(&self) {
        loop {
            match self.lock.compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed) {
                Ok(_) => return,
                Err(_) => core::hint::spin_loop(),
            }
        }
    }
    pub fn release(&self) { self.lock.store(0, Ordering::Release); }

    pub fn slot_for_tid(&self, tid: u16) -> Option<u16> {
        for (i, &t) in self.tid_to_slot.iter().enumerate() {
            if t == tid { return Some(i as u16); }
        }
        None
    }
}

static mut QUEUE_TABLE: BmoQueueTable = BmoQueueTable::new();

pub fn queue_table() -> &'static mut BmoQueueTable {
    unsafe { &mut QUEUE_TABLE }
}
