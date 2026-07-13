//! Cache and TLB topology for the Ryzen 5 5600X.
//!
//! Recovers the legacy `cache_topology.rs` from the deleted
//! `crates_Personal/ring0/cpu_vendor_profile/.../cache_topology.rs`,
//! simplified for the minimal Ring 0 base (we only expose the
//! known-good values for the 5600X; full detection via CPUID
//! 0x80000005/06/1D is left as future work).
//!
//! 5600X (Vermeer) cache hierarchy:
//!   L1d:  32 KB, 8-way, 64 B line, per-core
//!   L1i:  32 KB, 8-way, 64 B line, per-core
//!   L2:  512 KB, 8-way, 64 B line, per-core (victim)
//!   L3:  32 MB, 16-way, 64 B line, shared across CCX (single-CCD)

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheType { Data, Instruction, Unified, Unknown }

#[derive(Debug, Clone, Copy)]
pub struct CacheInfo {
    pub level: u8,
    pub size_kb: u32,
    pub line_size_bytes: u8,
    pub associativity: u8,
    pub shared_threads: u8,
    pub cache_type: CacheType,
}

#[derive(Debug, Clone, Copy)]
pub struct CacheTopology {
    pub l1d: Option<CacheInfo>,
    pub l1i: Option<CacheInfo>,
    pub l2:  Option<CacheInfo>,
    pub l3:  Option<CacheInfo>,
}

/// Hardcoded for the 5600X. A future revision should run CPUID
/// 0x80000005/06/1D to detect at runtime.
pub fn detect_5600x() -> CacheTopology {
    CacheTopology {
        l1d: Some(CacheInfo {
            level: 1, size_kb: 32, line_size_bytes: 64,
            associativity: 8, shared_threads: 1, cache_type: CacheType::Data,
        }),
        l1i: Some(CacheInfo {
            level: 1, size_kb: 32, line_size_bytes: 64,
            associativity: 8, shared_threads: 1, cache_type: CacheType::Instruction,
        }),
        l2: Some(CacheInfo {
            level: 2, size_kb: 512, line_size_bytes: 64,
            associativity: 8, shared_threads: 1, cache_type: CacheType::Unified,
        }),
        l3: Some(CacheInfo {
            level: 3, size_kb: 32 * 1024, line_size_bytes: 64,
            associativity: 16, shared_threads: 12, cache_type: CacheType::Unified,
        }),
    }
}
