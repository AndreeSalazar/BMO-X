//! Atomicos del BMO ABI — wrappers `#[repr(transparent)]` sobre los
//! `core::sync::atomic::*` de Rust.
//!
//! Reemplaza `<stdatomic.h>` (`atomic_uint`, `atomic_compare_exchange`),
//! Win32 `Interlocked*`, POSIX `__atomic_*`. Mismo costo runtime, mejor
//! type safety.

#![allow(dead_code)]

use core::sync::atomic::{
    AtomicBool as CoreAtomicBool,
    AtomicU32  as CoreAtomicU32,
    AtomicU64  as CoreAtomicU64,
    Ordering,
};

/// Memoria ordering del BMO ABI — alias 1-a-1 con Rust pero documentado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MemOrder {
    /// Sin garantías de ordenamiento (sólo atomicidad).
    Relaxed = 0,
    /// Acquire — ningún read/write posterior puede reordenarse antes.
    Acquire = 1,
    /// Release — ningún read/write previo puede reordenarse después.
    Release = 2,
    /// Acquire + Release.
    AcqRel  = 3,
    /// Sequentially consistent (más estricto, default razonable).
    SeqCst  = 4,
}

impl MemOrder {
    #[inline(always)]
    fn into_core(self) -> Ordering {
        match self {
            Self::Relaxed => Ordering::Relaxed,
            Self::Acquire => Ordering::Acquire,
            Self::Release => Ordering::Release,
            Self::AcqRel  => Ordering::AcqRel,
            Self::SeqCst  => Ordering::SeqCst,
        }
    }
}

#[repr(transparent)]
#[derive(Debug)]
pub struct BmoAtomicU32(pub CoreAtomicU32);

impl BmoAtomicU32 {
    pub const fn new(v: u32) -> Self { Self(CoreAtomicU32::new(v)) }
    #[inline(always)]
    pub fn load(&self, o: MemOrder) -> u32 { self.0.load(o.into_core()) }
    #[inline(always)]
    pub fn store(&self, v: u32, o: MemOrder) { self.0.store(v, o.into_core()) }
    #[inline(always)]
    pub fn swap(&self, v: u32, o: MemOrder) -> u32 { self.0.swap(v, o.into_core()) }
    #[inline(always)]
    pub fn fetch_add(&self, v: u32, o: MemOrder) -> u32 { self.0.fetch_add(v, o.into_core()) }
    #[inline(always)]
    pub fn fetch_sub(&self, v: u32, o: MemOrder) -> u32 { self.0.fetch_sub(v, o.into_core()) }
    #[inline(always)]
    pub fn compare_exchange(&self, current: u32, new: u32, success: MemOrder, failure: MemOrder)
        -> Result<u32, u32>
    {
        self.0.compare_exchange(current, new, success.into_core(), failure.into_core())
    }
}

#[repr(transparent)]
#[derive(Debug)]
pub struct BmoAtomicU64(pub CoreAtomicU64);

impl BmoAtomicU64 {
    pub const fn new(v: u64) -> Self { Self(CoreAtomicU64::new(v)) }
    #[inline(always)]
    pub fn load(&self, o: MemOrder) -> u64 { self.0.load(o.into_core()) }
    #[inline(always)]
    pub fn store(&self, v: u64, o: MemOrder) { self.0.store(v, o.into_core()) }
    #[inline(always)]
    pub fn fetch_add(&self, v: u64, o: MemOrder) -> u64 { self.0.fetch_add(v, o.into_core()) }
}

#[repr(transparent)]
#[derive(Debug)]
pub struct BmoAtomicBool(pub CoreAtomicBool);

impl BmoAtomicBool {
    pub const fn new(v: bool) -> Self { Self(CoreAtomicBool::new(v)) }
    #[inline(always)]
    pub fn load(&self, o: MemOrder) -> bool { self.0.load(o.into_core()) }
    #[inline(always)]
    pub fn store(&self, v: bool, o: MemOrder) { self.0.store(v, o.into_core()) }
    #[inline(always)]
    pub fn swap(&self, v: bool, o: MemOrder) -> bool { self.0.swap(v, o.into_core()) }
}
