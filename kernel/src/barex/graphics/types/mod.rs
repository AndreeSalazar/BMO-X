//! Tipos transversales (Format, MemoryHint, barriers). Sólo enums BMO.

pub mod format;
pub mod memory_hint;
pub mod barrier;

pub use format::Format;
pub use memory_hint::MemoryHint;
pub use barrier::{BxBarrier, Sync, Access, Layout};
