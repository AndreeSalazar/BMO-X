//! Memory types — local copies of the legacy `bmo_boot_protocol` types.
//!
//! We keep them in `mm::types` so that any future migration to the
//! proper `BootContext::MemoryEntry` can be done in one place.

/// Convert a slice of `BootContext` memory entries to the local
/// `MemoryEntry` representation.
pub fn from_ctx(entries: &[boot_context::MemoryEntry]) -> alloc_types::Vec<MemoryEntry> {
    let mut v = alloc_types::Vec::new();
    for e in entries {
        v.push(MemoryEntry {
            base: e.base,
            size: e.size,
            mem_type: map_kind(e.kind),
        });
    }
    v
}

fn map_kind(k: u32) -> MemType {
    match k {
        1 => MemType::Usable,
        2 => MemType::Reserved,
        3 => MemType::AcpiReclaimable,
        4 => MemType::AcpiNvs,
        5 => MemType::Unusable,
        _ => MemType::Reserved,
    }
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemType {
    Usable = 0,
    Reserved = 1,
    AcpiReclaimable = 2,
    AcpiNvs = 3,
    Unusable = 4,
}

impl MemType {
    pub fn is_usable(self) -> bool {
        matches!(self, Self::Usable | Self::AcpiReclaimable)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MemoryEntry {
    pub base: u64,
    pub size: u64,
    pub mem_type: MemType,
}

impl MemoryEntry {
    pub fn end(&self) -> u64 { self.base.wrapping_add(self.size) }
}

/// Local allocator helper namespace — keeps the `alloc` crate
/// dependency out of `mm` modules that don't actually need it.
pub mod alloc_types {
    pub type Vec<T> = ::alloc::vec::Vec<T>;
}
