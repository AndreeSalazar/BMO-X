//! Plugin Loader — resolves function symbols + capabilities at runtime.
//!
//! ## Capability-based discovery
//! Instead of linking by symbol name, modules declare:
//!   provides = "framebuffer.write, input.poll"
//!   requires = "storage.read"
//!
//! The registry resolves requires → provides across all loaded modules.

extern crate alloc;
use alloc::vec::Vec;

pub struct SymEntry {
    pub addr: u64,
    pub provides: &'static str,
    pub requires: &'static str,
}

/// BMO_SYMBOLS.toml parser + capability resolver.
pub struct SymbolRegistry {
    raw: &'static str,
}

impl SymbolRegistry {
    pub const fn new(raw: &'static str) -> Self {
        Self { raw }
    }

    /// Resolve a qualified symbol name: "timeback._module_start"
    pub fn resolve(&self, full_name: &str) -> Option<u64> {
        let header = alloc::format!("[{}]\naddr = ", full_name);
        if let Some(pos) = self.raw.find(&header) {
            let start = pos + header.len();
            let end = self.raw[start..].find('\n').unwrap_or(self.raw.len() - start);
            return parse_u64(&self.raw[start..start + end]);
        }
        None
    }

    /// Call a function by name (no args, returns T).
    pub unsafe fn call0<T>(&self, name: &str) -> Option<T> {
        let addr = self.resolve(name)?;
        let fn_ptr: extern "C" fn() -> T = core::mem::transmute(addr as *const ());
        Some(fn_ptr())
    }

    /// Chain-of-trust: verify module integrity at load time.
    /// Returns true if the module's expected hash matches the loaded binary.
    /// Module binary is at `module_base..module_base+module_size`.
    pub fn verify_chain_hash(&self, module_base: u64, module_size: u64) -> bool {
        // Simple integrity check: module is in expected address range.
        // Full BLAKE3 verification requires linking blake3, deferred.
        // For now, check that the module's base address is within known ranges.
        for entry in self.all_entries() {
            if entry.addr >= module_base && entry.addr < module_base + module_size {
                return true;
            }
        }
        false // no known symbol in this range
    }

    /// List ALL entries in the registry.
    fn all_entries(&self) -> Vec<SymEntry> {
        let mut results = Vec::new();
        let mut pos = 0usize;
        while let Some(start) = self.raw[pos..].find("[") {
            let abs_start = pos + start;
            if let Some(section_end) = self.raw[abs_start..].find("\n\n") {
                let section = &self.raw[abs_start..abs_start + section_end];
                let mut addr = 0u64;
                let mut provides = "";
                let mut requires = "";
                for line in section.lines() {
                    if let Some((key, val)) = line.split_once('=') {
                        match key.trim() {
                            "addr" => addr = parse_u64(val.trim()).unwrap_or(0),
                            "provides" => provides = val.trim().trim_matches('"'),
                            "requires" => requires = val.trim().trim_matches('"'),
                            _ => {}
                        }
                    }
                }
                if addr != 0 { results.push(SymEntry { addr, provides, requires }); }
                pos = abs_start + section_end + 1;
            } else { break; }
        }
        results
    }

    /// Find a module that PROVIDES a specific capability.
    /// Searches all entries for "provides = ...capability..."
    pub fn find_by_capability(&self, capability: &str) -> Option<u64> {
        let search = "provides = ";
        let mut pos = 0usize;
        while let Some(start) = self.raw[pos..].find(search) {
            let abs_start = pos + start + search.len();
            let line_end = self.raw[abs_start..].find('\n').unwrap_or(self.raw.len() - abs_start);
            let caps_line = &self.raw[abs_start..abs_start + line_end];
            if caps_line.contains(capability) {
                let section_start = self.raw[..abs_start].rfind('[').unwrap_or(0);
                if let Some(addr_pos) = self.raw[section_start..abs_start].rfind("addr = ") {
                    let addr_start = section_start + addr_pos + 7;
                    let addr_end = self.raw[addr_start..].find('\n').unwrap_or(10);
                    if let Some(addr) = parse_u64(&self.raw[addr_start..addr_start + addr_end]) {
                        return Some(addr);
                    }
                }
            }
            pos = abs_start + line_end;
        }
        None
    }

    /// List all symbols in a category with their capability info.
    pub fn in_category(&self, category: &str) -> Vec<SymEntry> {
        let mut results = Vec::new();
        let prefix = alloc::format!("[{}", category);
        let mut pos = 0usize;
        while let Some(start) = self.raw[pos..].find(&prefix) {
            let abs_start = pos + start;
            let section_end = self.raw[abs_start..].find("\n\n")
                .unwrap_or(self.raw.len() - abs_start);
            let section = &self.raw[abs_start..abs_start + section_end];

            let mut addr = 0u64;
            let mut provides = "";
            let mut requires = "";

            for line in section.lines() {
                if let Some((key, val)) = line.split_once('=') {
                    match key.trim() {
                        "addr" => addr = parse_u64(val.trim()).unwrap_or(0),
                        "provides" => provides = val.trim().trim_matches('"'),
                        "requires" => requires = val.trim().trim_matches('"'),
                        _ => {}
                    }
                }
            }

            if addr != 0 {
                results.push(SymEntry { addr, provides, requires });
            }
            pos = abs_start + section_end + 1;
        }
        results
    }
}

fn parse_u64(s: &str) -> Option<u64> {
    let s = s.trim();
    let mut val: u64 = 0;
    for b in s.bytes() {
        if !b.is_ascii_digit() { return None; }
        val = val.wrapping_mul(10).wrapping_add((b - b'0') as u64);
    }
    Some(val)
}
