//! GIL implementations for different language requirements.

#![allow(dead_code)]

use crate::bmo_core::barex::BxResult;
use super::super::traits::{GilType, GilPlugin, GilStats};
use super::sync::{SpinLock, RwLock};

/// Traditional GIL (like CPython)
pub struct TraditionalGil {
    locked: SpinLock,
    stats: GilStats,
}

impl TraditionalGil {
    pub fn new() -> Self {
        Self {
            locked: SpinLock::new(),
            stats: GilStats {
                acquisitions: 0,
                releases: 0,
                contention: 0,
                wait_time_us: 0,
            },
        }
    }
}

impl GilPlugin for TraditionalGil {
    fn gil_type(&self) -> GilType {
        GilType::Traditional
    }

    fn acquire(&self) -> BxResult<()> {
        self.locked.lock();
        Ok(())
    }

    fn release(&self) -> BxResult<()> {
        self.locked.unlock();
        Ok(())
    }

    fn is_held(&self) -> bool {
        self.locked.is_locked()
    }

    fn try_acquire(&self) -> bool {
        self.locked.try_lock()
    }

    fn stats(&self) -> GilStats {
        self.stats.clone()
    }
}

/// Fine-grained GIL (per-object locks)
pub struct FineGrainedGil {
    stats: GilStats,
}

impl FineGrainedGil {
    pub fn new() -> Self {
        Self {
            stats: GilStats {
                acquisitions: 0,
                releases: 0,
                contention: 0,
                wait_time_us: 0,
            },
        }
    }
}

impl GilPlugin for FineGrainedGil {
    fn gil_type(&self) -> GilType {
        GilType::FineGrained
    }

    fn acquire(&self) -> BxResult<()> {
        // Fine-grained locking doesn't need global lock
        Ok(())
    }

    fn release(&self) -> BxResult<()> {
        Ok(())
    }

    fn is_held(&self) -> bool {
        false
    }

    fn try_acquire(&self) -> bool {
        true
    }

    fn stats(&self) -> GilStats {
        self.stats.clone()
    }
}

/// Read-Write Lock GIL
pub struct ReadWriteGil {
    lock: RwLock,
    stats: GilStats,
}

impl ReadWriteGil {
    pub fn new() -> Self {
        Self {
            lock: RwLock::new(),
            stats: GilStats {
                acquisitions: 0,
                releases: 0,
                contention: 0,
                wait_time_us: 0,
            },
        }
    }
}

impl GilPlugin for ReadWriteGil {
    fn gil_type(&self) -> GilType {
        GilType::ReadWriteLock
    }

    fn acquire(&self) -> BxResult<()> {
        self.lock.write_lock();
        Ok(())
    }

    fn release(&self) -> BxResult<()> {
        self.lock.write_unlock();
        Ok(())
    }

    fn is_held(&self) -> bool {
        self.lock.is_write_locked()
    }

    fn try_acquire(&self) -> bool {
        self.lock.try_write_lock()
    }

    fn stats(&self) -> GilStats {
        self.stats.clone()
    }
}

/// Lock-free GIL (no actual locking)
pub struct LockFreeGil {
    stats: GilStats,
}

impl LockFreeGil {
    pub fn new() -> Self {
        Self {
            stats: GilStats {
                acquisitions: 0,
                releases: 0,
                contention: 0,
                wait_time_us: 0,
            },
        }
    }
}

impl GilPlugin for LockFreeGil {
    fn gil_type(&self) -> GilType {
        GilType::LockFree
    }

    fn acquire(&self) -> BxResult<()> {
        // Lock-free: no actual locking needed
        Ok(())
    }

    fn release(&self) -> BxResult<()> {
        Ok(())
    }

    fn is_held(&self) -> bool {
        false
    }

    fn try_acquire(&self) -> bool {
        true
    }

    fn stats(&self) -> GilStats {
        self.stats.clone()
    }
}
