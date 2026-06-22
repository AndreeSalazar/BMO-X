//! `ring0::security::memory_policy` — W^X y permisos en page tables.
//!
//! v1.8.8: stub. La política por defecto es W^X (nunca W+X).

#![allow(dead_code)]

/// ¿La página `[vaddr, vaddr+len)` debe ser W^X?
pub fn is_wx_violation(_vaddr: u64, _len: u64) -> bool {
    // v1.8.8: stub. En v1.9, leer las PTE y detectar W+X.
    false
}

pub fn init() {}
