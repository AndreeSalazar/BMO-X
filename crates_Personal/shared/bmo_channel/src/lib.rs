//! BMO Channel — lock-free ring buffer IPC between Ring 0 ↔ Ring 3.
//!
//! ## Architecture
//!
//! ```text
//! ┌────────────────── 4096-byte shared page ──────────────────┐
//! │  magic  │ doorbell │ submit_h/t │ complete_h/t │  pad     │
//! │  submit_ring[64]  │  complete_ring[64]                    │
//! └───────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Protocol
//!
//! Ring 3 (producer of requests):
//!   1. Write entry at `submit_ring[submit_head % 64]`
//!   2. Atomic increment submit_head
//!   3. Set doorbell = 1 (barrier)
//!
//! Ring 0 (consumer, on timer tick):
//!   1. Read doorbell (barrier)
//!   2. While submit_tail < submit_head:
//!      a. Process entry at `submit_ring[submit_tail % 64]`
//!      b. Write response to `complete_ring[complete_head % 64]`
//!      c. Increment submit_tail, complete_head
//!   3. Barrier, doorbell = 0
//!
//! Ring 3 (consumer of responses):
//!   Poll: read complete_tail, process entries up to complete_head

#![no_std]

use core::sync::atomic::{AtomicU64, Ordering};

pub const CHANNEL_MAGIC: u64 = 0x424D_4F43; // "BMOC"

pub const RING_SIZE: usize = 62;
pub const ENTRY_SIZE: usize = 32;

/// Single entry in a channel ring.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct ChannelEntry {
    /// Operation code (0 = empty, 1 = key, 2 = mouse, etc.)
    pub opcode: u64,
    pub arg0: u64,
    pub arg1: u64,
    pub arg2: u64,
}

/// Channel header at the start of the shared page.
#[repr(C)]
pub struct Channel {
    pub magic: u64,
    /// Doorbell: Ring 3 sets to 1 when there's work. Ring 0 clears to 0 when done.
    pub doorbell: AtomicU64,
    /// Submission ring head (Ring 3 writes, Ring 0 reads).
    pub submit_head: AtomicU64,
    /// Submission ring tail (Ring 0 advances).
    pub submit_tail: AtomicU64,
    /// Completion ring head (Ring 0 writes, Ring 3 reads).
    pub complete_head: AtomicU64,
    /// Completion ring tail (Ring 3 advances).
    pub complete_tail: AtomicU64,
    pub _pad: [u64; 2],
    pub submit_ring: [ChannelEntry; RING_SIZE],
    pub complete_ring: [ChannelEntry; RING_SIZE],
}

// Compile-time size check
const _SIZE_CHECK: () = assert!(core::mem::size_of::<Channel>() <= 4096);

impl Channel {
    /// Initialize a zeroed channel page. Caller must ensure the page is
    /// mapped into both Ring 0 and Ring 3 address spaces.
    pub fn init(&mut self) {
        self.magic = CHANNEL_MAGIC;
        self.doorbell.store(0, Ordering::Relaxed);
        self.submit_head.store(0, Ordering::Relaxed);
        self.submit_tail.store(0, Ordering::Relaxed);
        self.complete_head.store(0, Ordering::Relaxed);
        self.complete_tail.store(0, Ordering::Relaxed);
    }

    // ═══ Ring 3 API ═══════════════════════════════════════════════════

    /// Ring 3: submit a request. Returns true if there was room.
    pub fn ring3_submit(&self, opcode: u64, arg0: u64, arg1: u64, arg2: u64) -> bool {
        let head = self.submit_head.load(Ordering::Relaxed);
        let tail = self.submit_tail.load(Ordering::Acquire);
        if head.wrapping_sub(tail) >= RING_SIZE as u64 { return false; }

        let idx = (head % RING_SIZE as u64) as usize;
        // SAFETY: we own this slot (head is our exclusive write pointer)
        unsafe {
            let entry = &self.submit_ring[idx] as *const ChannelEntry as *mut ChannelEntry;
            (*entry).opcode = opcode;
            (*entry).arg0 = arg0;
            (*entry).arg1 = arg1;
            (*entry).arg2 = arg2;
        }
        self.submit_head.store(head.wrapping_add(1), Ordering::Release);
        true
    }

    /// Ring 3: signal Ring 0 that there's work (set doorbell).
    pub fn ring3_doorbell(&self) {
        self.doorbell.store(1, Ordering::Release);
    }

    /// Ring 3: submit and immediately signal. Convenience.
    pub fn ring3_send(&self, opcode: u64, arg0: u64, arg1: u64, arg2: u64) -> bool {
        if !self.ring3_submit(opcode, arg0, arg1, arg2) { return false; }
        self.ring3_doorbell();
        true
    }

    /// Ring 3: poll for completed responses. Returns number of entries consumed.
    /// The callback receives each completed entry.
    pub fn ring3_poll<F: FnMut(u64, u64, u64, u64)>(&self, mut callback: F) -> usize {
        let head = self.complete_head.load(Ordering::Acquire);
        let mut tail = self.complete_tail.load(Ordering::Relaxed);
        let mut count = 0;

        while tail < head && count < RING_SIZE {
            let idx = (tail % RING_SIZE as u64) as usize;
            let entry = unsafe { &*core::ptr::addr_of!(self.complete_ring[idx]) };
            if entry.opcode != 0 {
                callback(entry.opcode, entry.arg0, entry.arg1, entry.arg2);
            }
            tail = tail.wrapping_add(1);
            count += 1;
        }
        if count > 0 {
            self.complete_tail.store(tail, Ordering::Release);
        }
        count
    }

    // ═══ Ring 0 API ═══════════════════════════════════════════════════

    /// Ring 0: check doorbell. Called from timer ISR at ~1kHz.
    pub fn ring0_has_work(&self) -> bool {
        self.doorbell.load(Ordering::Acquire) != 0
    }

    /// Ring 0: process all pending submissions. Returns number processed.
    /// The handler receives each request and returns a response entry.
    /// Handler return: (opcode, arg0, arg1, arg2) or None to skip response.
    pub fn ring0_process<F: FnMut(u64, u64, u64, u64) -> Option<(u64, u64, u64, u64)>>(
        &self,
        mut handler: F,
    ) -> usize {
        let head = self.submit_head.load(Ordering::Acquire);
        let mut tail = self.submit_tail.load(Ordering::Relaxed);
        let mut count = 0;

        while tail < head && count < RING_SIZE {
            let idx = (tail % RING_SIZE as u64) as usize;
            let entry = unsafe { &*core::ptr::addr_of!(self.submit_ring[idx]) };
            let (opcode, a0, a1, a2) = (entry.opcode, entry.arg0, entry.arg1, entry.arg2);

            // Clear the entry so Ring 3 doesn't reuse stale data
            unsafe {
                let e = &self.submit_ring[idx] as *const ChannelEntry as *mut ChannelEntry;
                (*e).opcode = 0;
            }

            if let Some((r_op, r0, r1, r2)) = handler(opcode, a0, a1, a2) {
                let c_idx = (self.complete_head.load(Ordering::Relaxed) % RING_SIZE as u64) as usize;
                unsafe {
                    let re = &self.complete_ring[c_idx] as *const ChannelEntry as *mut ChannelEntry;
                    (*re).opcode = r_op;
                    (*re).arg0 = r0;
                    (*re).arg1 = r1;
                    (*re).arg2 = r2;
                }
                self.complete_head.fetch_add(1, Ordering::Release);
            }

            tail = tail.wrapping_add(1);
            count += 1;
        }

        self.submit_tail.store(tail, Ordering::Release);
        self.doorbell.store(0, Ordering::Release);
        count
    }
}
