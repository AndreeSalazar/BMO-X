//! Synchronization primitives for GIL implementations.

#![allow(dead_code)]

use core::sync::atomic::{AtomicBool, Ordering};

/// Simple spinlock
pub struct SpinLock {
    locked: AtomicBool,
}

impl SpinLock {
    pub fn new() -> Self {
        Self {
            locked: AtomicBool::new(false),
        }
    }

    pub fn lock(&self) {
        while self.locked.compare_exchange_weak(
            false,
            true,
            Ordering::Acquire,
            Ordering::Relaxed,
        ).is_err() {
            // Spin wait
            core::hint::spin_loop();
        }
    }

    pub fn unlock(&self) {
        self.locked.store(false, Ordering::Release);
    }

    pub fn try_lock(&self) -> bool {
        self.locked.compare_exchange(
            false,
            true,
            Ordering::Acquire,
            Ordering::Relaxed,
        ).is_ok()
    }

    pub fn is_locked(&self) -> bool {
        self.locked.load(Ordering::Relaxed)
    }
}

/// Read-Write lock
pub struct RwLock {
    write_locked: AtomicBool,
    read_count: AtomicBool, // Simplified - would use AtomicUsize
}

impl RwLock {
    pub fn new() -> Self {
        Self {
            write_locked: AtomicBool::new(false),
            read_count: AtomicBool::new(false),
        }
    }

    pub fn read_lock(&self) {
        while self.write_locked.load(Ordering::Acquire) {
            core::hint::spin_loop();
        }
        // Would increment read count here
    }

    pub fn read_unlock(&self) {
        // Would decrement read count here
    }

    pub fn write_lock(&self) {
        while self.write_locked.compare_exchange_weak(
            false,
            true,
            Ordering::Acquire,
            Ordering::Relaxed,
        ).is_err() {
            core::hint::spin_loop();
        }
        // Wait for readers to finish
    }

    pub fn write_unlock(&self) {
        self.write_locked.store(false, Ordering::Release);
    }

    pub fn try_write_lock(&self) -> bool {
        self.write_locked.compare_exchange(
            false,
            true,
            Ordering::Acquire,
            Ordering::Relaxed,
        ).is_ok()
    }

    pub fn is_write_locked(&self) -> bool {
        self.write_locked.load(Ordering::Relaxed)
    }
}
