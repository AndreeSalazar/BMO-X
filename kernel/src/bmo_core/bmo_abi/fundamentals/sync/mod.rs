//! `bmo_abi::sync::atomic` — Atomic operations with BMO ABI types.
//!
//! Thin wrapper around `core::sync::atomic` to keep BMO ABI types
//! (BxU64 instead of u64) in the type signature. The wrapper is `const`
//! where possible to allow static initialization.

#![allow(dead_code)]

use crate::bmo_core::bmo_abi::primitives::bx_u64;

/// Memory ordering (mirrors `core::sync::Ordering`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemOrder {
    Relaxed,
    Acquire,
    Release,
    AcqRel,
    SeqCst,
}

impl MemOrder {
    pub const fn to_core(self) -> core::sync::atomic::Ordering {
        match self {
            MemOrder::Relaxed => core::sync::atomic::Ordering::Relaxed,
            MemOrder::Acquire => core::sync::atomic::Ordering::Acquire,
            MemOrder::Release => core::sync::atomic::Ordering::Release,
            MemOrder::AcqRel  => core::sync::atomic::Ordering::AcqRel,
            MemOrder::SeqCst  => core::sync::atomic::Ordering::SeqCst,
        }
    }
}

#[repr(transparent)]
pub struct BmoAtomicU64(core::sync::atomic::AtomicU64);

impl BmoAtomicU64 {
    pub const fn new(v: bx_u64) -> Self {
        Self(core::sync::atomic::AtomicU64::new(v))
    }
    pub fn load(&self, o: MemOrder) -> bx_u64 {
        self.0.load(o.to_core())
    }
    pub fn store(&self, v: bx_u64, o: MemOrder) {
        self.0.store(v, o.to_core());
    }
    pub fn fetch_add(&self, v: bx_u64, o: MemOrder) -> bx_u64 {
        self.0.fetch_add(v, o.to_core())
    }
    pub fn compare_exchange(&self, current: bx_u64, new: bx_u64, o: MemOrder) -> Result<bx_u64, bx_u64> {
        self.0.compare_exchange(current, new, o.to_core(), o.to_core())
    }
}
