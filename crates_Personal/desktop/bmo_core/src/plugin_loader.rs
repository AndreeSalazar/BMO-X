//! Plugin Loader — resolves function symbols at runtime via BMO_SYMBOLS.toml.
//!
//! The symbol table is embedded at compile time via `include_str!`.
//! Functions from shared modules (timeback, cabina) are resolved by name.

extern crate alloc;
use alloc::vec::Vec;

pub struct SymEntry {
    pub addr: u64,
}

/// Simple TOML parser for: [category.name]\naddr = X
pub struct SymbolRegistry {
    raw: &'static str,
}

impl SymbolRegistry {
    pub const fn new(raw: &'static str) -> Self {
        Self { raw }
    }

    /// Resolve a qualified symbol: "timeback._module_start" → address
    pub fn resolve(&self, full_name: &str) -> Option<u64> {
        let header = alloc::format!("[{}]\naddr = ", full_name);
        if let Some(pos) = self.raw.find(&header) {
            let start = pos + header.len();
            let end = self.raw[start..].find('\n').unwrap_or(self.raw.len() - start);
            let num_str = &self.raw[start..start + end];
            let addr = parse_u64(num_str);
            return addr;
        }
        None
    }

    /// Call a function by name (no args, returns T).
    /// # Safety
    /// Function must exist at the resolved address with matching signature.
    pub unsafe fn call0<T>(&self, name: &str) -> Option<T> {
        let addr = self.resolve(name)?;
        let fn_ptr: extern "C" fn() -> T = core::mem::transmute(addr as *const ());
        Some(fn_ptr())
    }

    /// List all symbols in a category.
    pub fn in_category(&self, category: &str) -> Vec<SymEntry> {
        let mut results = Vec::new();
        let prefix = alloc::format!("[{}", category);
        let mut pos = 0usize;
        while let Some(start) = self.raw[pos..].find(&prefix) {
            let abs_start = pos + start;
            let section_end = self.raw[abs_start..]
                .find("\n\n")
                .unwrap_or(self.raw.len() - abs_start);
            let section = &self.raw[abs_start..abs_start + section_end];
            if let Some(addr_line) = section.find("addr = ") {
                let addr_start = abs_start + addr_line + 7;
                let addr_end = self.raw[addr_start..].find('\n').unwrap_or(10);
                let num_str = &self.raw[addr_start..addr_start + addr_end];
                if let Some(addr) = parse_u64(num_str) {
                    results.push(SymEntry { addr });
                }
            }
            pos = abs_start + section_end + 1;
        }
        results
    }
}

/// Simple u64 parser (no_std, no alloc).
fn parse_u64(s: &str) -> Option<u64> {
    let s = s.trim();
    let mut val: u64 = 0;
    for b in s.bytes() {
        if !b.is_ascii_digit() { return None; }
        val = val.wrapping_mul(10).wrapping_add((b - b'0') as u64);
    }
    Some(val)
}
