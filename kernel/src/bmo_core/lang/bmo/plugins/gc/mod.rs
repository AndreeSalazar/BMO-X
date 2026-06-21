//! Garbage Collection module for BMO.
//!
//! Provides multiple GC strategies for different language requirements:
//! - `mark_sweep.rs` - Traditional mark-and-sweep GC
//! - `copying.rs` - Copying/semispace GC
//! - `generational.rs` - Generational GC (young/old generations)
//! - `reference_counting.rs` - Reference counting (ARC/RC)
//! - `concurrent.rs` - Concurrent GC (background collection)
//! - `region.rs` - Region-based memory management

#![allow(dead_code)]

extern crate alloc;
use alloc::boxed::Box;

pub mod mark_sweep;
pub mod copying;
pub mod generational;
pub mod reference_counting;
pub mod concurrent;
pub mod region;

// Re-exports
pub use mark_sweep::MarkSweepGc;
pub use copying::CopyingGc;
pub use generational::GenerationalGc;
pub use reference_counting::ReferenceCountingGc;
pub use concurrent::ConcurrentGc;
pub use region::RegionGc;

use crate::bmo_gpu::BxResult;
use super::traits::{GcType, GcPlugin};

/// Create GC plugin based on type
pub fn create_gc(gc_type: GcType, heap_size: usize) -> BxResult<Box<dyn GcPlugin>> {
    match gc_type {
        GcType::MarkSweep => {
            let mut gc = MarkSweepGc::new(heap_size);
            gc.init(heap_size)?;
            Ok(Box::new(gc))
        }
        GcType::Copying => {
            let mut gc = CopyingGc::new(heap_size);
            gc.init(heap_size)?;
            Ok(Box::new(gc))
        }
        GcType::Generational => {
            let mut gc = GenerationalGc::new(heap_size);
            gc.init(heap_size)?;
            Ok(Box::new(gc))
        }
        GcType::ReferenceCounting => {
            let mut gc = ReferenceCountingGc::new();
            gc.init(heap_size)?;
            Ok(Box::new(gc))
        }
        GcType::Concurrent => {
            let mut gc = ConcurrentGc::new(heap_size);
            gc.init(heap_size)?;
            Ok(Box::new(gc))
        }
        GcType::RegionBased => {
            let mut gc = RegionGc::new(heap_size);
            gc.init(heap_size)?;
            Ok(Box::new(gc))
        }
        GcType::Incremental => {
            let mut gc = GenerationalGc::new(heap_size);
            gc.init(heap_size)?;
            Ok(Box::new(gc))
        }
        GcType::None => {
            Err(crate::bmo_gpu::BxError::Unsupported)
        }
    }
}
