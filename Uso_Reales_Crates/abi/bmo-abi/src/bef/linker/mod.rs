#![allow(dead_code)]

pub mod registry;
pub mod resolver;

use crate::bmo_abi::bef::exports::ExportTable;
use crate::bmo_abi::bef::imports::ImportTable;
use registry::Registry;
use resolver::ResolveResult;

/// Inicializa el linker. Debe llamarse una vez al boot.
pub fn init() {
    Registry::clear();
}

/// Resuelve imports de una tabla contra el Registry global.
pub fn resolve_imports(
    import_table: &ImportTable,
    binding_data: &mut [u8],
    binding_base: u64,
) -> ResolveResult {
    resolver::resolve_imports(import_table, binding_data, binding_base)
}

/// Registra todos los exports de una ExportTable en el Registry global.
pub fn register_exports(table: &ExportTable, image_base: u64) -> u32 {
    let mut count = 0u32;
    for entry in table.entries {
        let name = table.symbol_name(entry).unwrap_or("");
        if name.is_empty() {
            continue;
        }
        Registry::insert("", name, image_base + entry.virt_addr);
        count += 1;
    }
    count
}

/// Registra exports con namespace de librería.
pub fn register_library_exports(lib_name: &str, table: &ExportTable, image_base: u64) -> u32 {
    let mut count = 0u32;
    for entry in table.entries {
        let name = table.symbol_name(entry).unwrap_or("");
        if name.is_empty() {
            continue;
        }
        Registry::insert(lib_name, name, image_base + entry.virt_addr);
        count += 1;
    }
    count
}

/// Registra un símbolo individual.
pub fn register_symbol(lib: &str, name: &str, addr: u64) {
    Registry::insert(lib, name, addr);
}

/// Busca un símbolo.
pub fn lookup(lib: &str, name: &str) -> u64 {
    Registry::lookup(lib, name)
}

/// Busca por hash.
pub fn lookup_by_hash(hash: u32, name: &str) -> u64 {
    Registry::lookup_by_hash(hash, name)
}

/// Cantidad de símbolos registrados.
pub fn symbol_count() -> usize {
    Registry::len()
}
