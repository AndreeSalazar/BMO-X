#![allow(dead_code)]

extern crate alloc;

use crate::bef::imports::{ImportTable, ImportFlags};
use crate::bef::exports::ExportTable;

use crate::bmo_abi::bef::linker as BefLinker;

/// Initialize the runtime symbol table — delegates to the BEF Linker.
pub fn init() {
    BefLinker::init();
    register_kernel_symbols();
    register_elf_thunk_symbols();
    crate::cabina::info_u64("bef", "linker initialized", BefLinker::symbol_count() as u64);
}

/// Register a symbol in the linker's global registry.
pub fn register_symbol(lib: &str, name: &str, addr: u64, _flags: u32) {
    BefLinker::register_symbol(lib, name, addr);
}

/// Look up a symbol by library + name via the linker.
pub fn lookup(lib: &str, name: &str) -> u64 {
    BefLinker::lookup(lib, name)
}

/// Look up a symbol by hash only (fast path).
pub fn lookup_by_hash(hash: u32, name: &str) -> u64 {
    BefLinker::lookup_by_hash(hash, name)
}

/// Register symbols from a BEF export table.
pub fn register_bef_exports(table: &ExportTable, base: u64) {
    BefLinker::register_library_exports("bef", table, base);
}

/// Resolve all BEF imports and patch binding offsets.
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

        let addr = BefLinker::lookup(lib, name);

        if addr != 0 {
            if entry.binding_offset != 0 {
                patch_binding(entry.binding_offset, addr, mapped)?;
            }
            resolved += 1;
        } else if entry.flags & ImportFlags::WEAK.bits() != 0 {
            if entry.binding_offset != 0 {
                patch_binding(entry.binding_offset, 0, mapped)?;
            }
        } else {
            unresolved += 1;
            crate::cabina::warn_u64("bef", "unresolved import @", entry.binding_offset);
        }
    }

    if unresolved > 0 {
        crate::cabina::warn_u64("bef", "total unresolved imports", unresolved as u64);
    }

    Ok(resolved)
}

/// Patch a binding offset in mapped sections with the resolved address.
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
            if section.data_ptr == 0 {
                continue;
            }
            unsafe {
                let ptr = (section.data_ptr as *mut u8).add(offset_in_section) as *mut u64;
                core::ptr::write(ptr, addr);
            }
            return Ok(());
        }
    }
    Err("binding offset outside mapped sections")
}

/// Register kernel system call symbols.
fn register_kernel_symbols() {
    register_symbol("bmo:kernel", "ProcessExit",   0x0000_0001, 0);
    register_symbol("bmo:kernel", "Yield",         0x0000_0003, 0);
    register_symbol("bmo:kernel", "ThreadCreate",  0x0000_0004, 0);
    register_symbol("bmo:kernel", "ThreadExit",    0x0000_0005, 0);
    register_symbol("bmo:kernel", "ClockGetTime",  0x0000_0050, 0);
    register_symbol("bmo:kernel", "FbInfo",        0x0000_0060, 0);
    register_symbol("bmo:kernel", "FbFill",        0x0000_0061, 0);
    register_symbol("bmo:kernel", "FbText",        0x0000_0062, 0);
    register_symbol("bmo:kernel", "KeyPoll",       0x0000_0070, 0);
    register_symbol("bmo:kernel", "Beep",          0x0000_0080, 0);
    register_symbol("bmo:kernel", "DebugPrint",    0x0000_00F0, 0);
}

/// Register ELF thunk REAL addresses as linker symbols.
fn register_elf_thunk_symbols() {
    use super::elf_thunks::THUNK_TABLE;

    for entry in THUNK_TABLE.iter() {
        let lib = normalize_lib_name(entry.lib);
        let addr = super::elf_thunks::resolve_fn_ptr(entry.lib, entry.name)
            .map(|p| p as u64)
            .unwrap_or(super::elf_thunks::silent_stub as *const () as u64);
        register_symbol(&lib, entry.name, addr, 0);
    }
}

fn normalize_lib_name<'a>(name: &'a str) -> &'a str {
    if let Some(pos) = name.rfind('.') {
        let after = &name[pos + 1..];
        if after.bytes().all(|b| b.is_ascii_digit()) && !after.is_empty() {
            return &name[..pos];
        }
    }
    name
}

/// Get the number of registered symbols.
pub fn symbol_count() -> usize {
    BefLinker::symbol_count()
}
