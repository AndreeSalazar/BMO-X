//! Global Interpreter Lock (GIL) module for ÑEXO.
//!
//! Provides different GIL implementations for languages that need them:
//! - `implementations.rs` - Various GIL strategies
//! - `sync.rs` - Synchronization primitives

#![allow(dead_code)]

extern crate alloc;
use alloc::boxed::Box;

pub mod implementations;
pub mod sync;

// Re-exports
pub use implementations::{TraditionalGil, FineGrainedGil, ReadWriteGil, LockFreeGil};

use crate::bmo_gpu::BxResult;
use super::traits::{GilType, GilPlugin};

/// Create GIL plugin based on type
pub fn create_gil(gil_type: GilType) -> BxResult<Box<dyn GilPlugin>> {
    match gil_type {
        GilType::Traditional => Ok(Box::new(TraditionalGil::new())),
        GilType::FineGrained => Ok(Box::new(FineGrainedGil::new())),
        GilType::ReadWriteLock => Ok(Box::new(ReadWriteGil::new())),
        GilType::LockFree => Ok(Box::new(LockFreeGil::new())),
        GilType::None => Err(crate::bmo_gpu::BxError::Unsupported),
    }
}
