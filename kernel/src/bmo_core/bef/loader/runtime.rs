//! Runtime symbol table — resolves BEF imports to actual function addresses.
//!
//! Populated from:
//!   1. BEF export tables (native binaries)
//!   2. PE thunk table (devoured Windows binaries)
//!   3. ELF thunk table (devoured Linux binaries)
//!   4. Kernel-provided system calls
//!
//! The resolution pipeline:
//!   ImportEntry → lookup (lib+name) → resolved address → patch binding_offset

#![allow(dead_code)]

extern crate alloc;

use crate::bmo_core::bef::imports::{ImportTable, ImportFlags};
use crate::bmo_core::bef::exports::ExportTable;

/// Maximum symbols in the runtime table.
const MAX_SYMBOLS: usize = 1024;

/// A resolved symbol in the runtime table.
#[derive(Debug, Clone, Copy)]
pub struct RuntimeSymbol {
    /// Library name (e.g., "kernel32.dll", "libc.so", "bmo:core").
    pub lib: &'static str,
    /// Symbol name (e.g., "ExitProcess", "malloc", "fb_fill").
    pub name: &'static str,
    /// Resolved virtual address.
    pub addr: u64,
    /// Symbol hash for fast lookup.
    pub hash: u32,
    /// Flags.
    pub flags: u32,
}

/// Flags for runtime symbols.
pub const SYM_EAGER: u32 = 1 << 0;
pub const SYM_WEAK: u32 = 1 << 1;
pub const SYM_KERNEL: u32 = 1 << 8;
pub const SYM_PE_THUNK: u32 = 1 << 9;
pub const SYM_ELF_THUNK: u32 = 1 << 10;
pub const SYM_EXPORT: u32 = 1 << 11;

/// Global runtime symbol table.
static mut SYMBOL_TABLE: [RuntimeSymbol; MAX_SYMBOLS] = [RuntimeSymbol {
    lib: "",
    name: "",
    addr: 0,
    hash: 0,
    flags: 0,
}; MAX_SYMBOLS];
static mut SYMBOL_COUNT: usize = 0;

/// Hash a string for symbol lookup (FNV-1a 32-bit).
pub fn hash_symbol(name: &[u8]) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for &b in name {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

/// Register a symbol in the runtime table.
pub fn register_symbol(lib: &'static str, name: &'static str, addr: u64, flags: u32) {
    unsafe {
        if SYMBOL_COUNT >= MAX_SYMBOLS {
            crate::bmo_core::diag::warn("bef", "runtime symbol table full");
            return;
        }
        let hash = hash_symbol(name.as_bytes());
        SYMBOL_TABLE[SYMBOL_COUNT] = RuntimeSymbol {
            lib,
            name,
            addr,
            hash,
            flags,
        };
        SYMBOL_COUNT += 1;
    }
}

/// Look up a symbol by library + name. Returns the resolved address or 0.
pub fn lookup(lib: &str, name: &str) -> u64 {
    let name_hash = hash_symbol(name.as_bytes());
    unsafe {
        for i in 0..SYMBOL_COUNT {
            let sym = &SYMBOL_TABLE[i];
            if sym.hash == name_hash && sym.name == name {
                // If lib is specified, also match library name.
                if !lib.is_empty() && !sym.lib.is_empty() && !eq_ci(sym.lib, lib) {
                    continue;
                }
                if sym.addr != 0 {
                    return sym.addr;
                }
            }
        }
    }
    0
}

/// Look up a symbol by hash only (fast path).
pub fn lookup_by_hash(hash: u32, name: &str) -> u64 {
    unsafe {
        for i in 0..SYMBOL_COUNT {
            let sym = &SYMBOL_TABLE[i];
            if sym.hash == hash && sym.name == name {
                return sym.addr;
            }
        }
    }
    0
}

/// Register symbols from a BEF export table.
pub fn register_bef_exports(table: &ExportTable, base: u64) {
    for e in table.entries {
        let name = match table.symbol_name(e) {
            Some(n) => n,
            None => continue,
        };
        let addr = base + e.virt_addr;
        let flags = SYM_EXPORT | SYM_EAGER;
        // Leak the name to get &'static str — acceptable in kernel ctx.
        let static_name = leak_str(name);
        register_symbol("bef", static_name, addr, flags);
    }
}

/// Leak a string into &'static str (acceptable in kernel ctx).
fn leak_str(s: &str) -> &'static str {
    let len = s.len();
    let layout = match core::alloc::Layout::from_size_align(len, 1) {
        Ok(l) => l,
        Err(_) => return "",
    };
    let ptr = unsafe { alloc::alloc::alloc(layout) };
    if ptr.is_null() {
        return "";
    }
    unsafe {
        core::ptr::copy_nonoverlapping(s.as_ptr(), ptr, len);
        core::str::from_utf8_unchecked(core::slice::from_raw_parts(ptr, len))
    }
}

/// Resolve all BEF imports and patch binding offsets.
///
/// For each import:
///   1. Look up the symbol in the runtime table.
///   2. If found, write the address to `binding_offset` in the target section.
///   3. If not found and not WEAK, log a warning.
pub fn resolve_imports(
    import_table: &ImportTable,
    mapped: &[super::MappedSection],
) -> Result<u32, &'static str> {
    let mut resolved = 0u32;
    let mut unresolved = 0u32;

    for entry in import_table.entries {
        let lib = import_table.library_name(entry).unwrap_or("");
        let name = import_table.symbol_name(entry).unwrap_or("");

        if name.is_empty() {
            continue;
        }

        // Look up in runtime symbol table.
        let addr = lookup(lib, name);

        if addr != 0 {
            // Patch the binding offset in the mapped section.
            if entry.binding_offset != 0 {
                patch_binding(entry.binding_offset, addr, mapped)?;
            }
            resolved += 1;
        } else if entry.flags & ImportFlags::WEAK.bits() != 0 {
            // Weak import — write 0 (caller must null-check).
            if entry.binding_offset != 0 {
                patch_binding(entry.binding_offset, 0, mapped)?;
            }
        } else {
            unresolved += 1;
            crate::bmo_core::diag::warn_u64("bef", "unresolved import", entry.binding_offset);
        }
    }

    if unresolved > 0 {
        crate::bmo_core::diag::warn_u64("bef", "total unresolved imports", unresolved as u64);
    }

    Ok(resolved)
}

/// Patch a binding offset in the mapped sections with the resolved address.
fn patch_binding(
    binding_offset: u64,
    addr: u64,
    mapped: &[super::MappedSection],
) -> Result<(), &'static str> {
    for section in mapped {
        if binding_offset >= section.virt_addr
            && binding_offset + 8 <= section.virt_addr + section.size
        {
            let offset_in_section = (binding_offset - section.virt_addr) as usize;
            unsafe {
                let ptr = (section.virt_addr as *mut u64).add(offset_in_section / 8);
                *ptr = addr;
            }
            return Ok(());
        }
    }
    Err("binding offset outside mapped sections")
}

/// Register kernel system call symbols.
pub fn register_kernel_symbols() {
    // Register core BMO syscalls as symbols.
    // These are the addresses that imported functions resolve to.
    register_symbol("bmo:kernel", "ProcessExit",   0x0000_0001, SYM_KERNEL | SYM_EAGER);
    register_symbol("bmo:kernel", "Yield",         0x0000_0003, SYM_KERNEL | SYM_EAGER);
    register_symbol("bmo:kernel", "ThreadCreate",  0x0000_0004, SYM_KERNEL | SYM_EAGER);
    register_symbol("bmo:kernel", "ThreadExit",    0x0000_0005, SYM_KERNEL | SYM_EAGER);
    register_symbol("bmo:kernel", "ClockGetTime",  0x0000_0050, SYM_KERNEL | SYM_EAGER);
    register_symbol("bmo:kernel", "FbInfo",        0x0000_0060, SYM_KERNEL | SYM_EAGER);
    register_symbol("bmo:kernel", "FbFill",        0x0000_0061, SYM_KERNEL | SYM_EAGER);
    register_symbol("bmo:kernel", "FbText",        0x0000_0062, SYM_KERNEL | SYM_EAGER);
    register_symbol("bmo:kernel", "KeyPoll",       0x0000_0070, SYM_KERNEL | SYM_EAGER);
    register_symbol("bmo:kernel", "Beep",          0x0000_0080, SYM_KERNEL | SYM_EAGER);
    register_symbol("bmo:kernel", "DebugPrint",    0x0000_00F0, SYM_KERNEL | SYM_EAGER);
}

/// Register PE thunk stubs as symbols.
pub fn register_pe_thunk_symbols() {
    use crate::bmo_gpu::shims::pe_thunks::THUNK_TABLE;
    use crate::bmo_gpu::shims::pe_thunks::silent_stub as stub_fn;

    for entry in THUNK_TABLE.iter() {
        // Register the stub address — a real implementation would
        // map ThunkTarget to actual function pointers.
        let addr = stub_fn as *const () as u64;
        register_symbol(entry.dll, entry.name, addr, SYM_PE_THUNK | SYM_EAGER);
    }
}

/// Register ELF thunk stubs as symbols.
pub fn register_elf_thunk_symbols() {
    use super::elf_thunks::THUNK_TABLE;
    use super::elf_thunks::silent_stub as stub_fn;

    for entry in THUNK_TABLE.iter() {
        let addr = stub_fn as *const () as u64;
        let lib = normalize_lib_name(entry.lib);
        register_symbol(lib, entry.name, addr, SYM_ELF_THUNK | SYM_EAGER);
    }
}

/// Normalize ELF library names (e.g., "libc.so.6" → "libc.so").
fn normalize_lib_name<'a>(name: &'a str) -> &'a str {
    // Find first '.' and use everything before it.
    if let Some(pos) = name.find('.') {
        &name[..pos]
    } else {
        name
    }
}

/// Case-insensitive string comparison.
fn eq_ci(a: &str, b: &str) -> bool {
    if a.len() != b.len() { return false; }
    let aa = a.as_bytes();
    let bb = b.as_bytes();
    for i in 0..aa.len() {
        if aa[i].to_ascii_lowercase() != bb[i].to_ascii_lowercase() {
            return false;
        }
    }
    true
}

/// Get the number of registered symbols.
pub fn symbol_count() -> usize {
    unsafe { SYMBOL_COUNT }
}

/// Initialize the runtime symbol table with kernel + thunk symbols.
pub fn init() {
    register_kernel_symbols();
    register_pe_thunk_symbols();
    register_elf_thunk_symbols();
    crate::bmo_core::diag::info_u64("bef", "runtime symbol table initialized", symbol_count() as u64);
}
