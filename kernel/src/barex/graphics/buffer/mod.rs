//! (9) `BxBuffer` — vertex / index / UBO / SSBO / raw.

use super::types::MemoryHint;

pub struct BxBuffer {
    pub size_bytes: u64,
    pub hint: MemoryHint,
}
