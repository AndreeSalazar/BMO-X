use alloc::vec::Vec;
use crate::bmo_abi::bef::{
    header::*,
    sections::*,
    relocations,
    imports::ImportTable,
};

#[derive(Debug)]
pub struct LoadedSection {
    pub kind: SectionKind,
    pub virt_addr: u64,
    pub size: u64,
    pub data: Vec<u8>,
}

#[derive(Debug)]
pub struct LoadedBef {
    pub entry_point: u64,
    pub sections: Vec<LoadedSection>,
    pub tls_base: u64,
    pub base_addr: u64,
}

pub fn load<F>(
    bytes: &[u8],
    base_addr: u64,
    mut _resolve_import: F,
) -> Result<LoadedBef, &'static str>
where
    F: FnMut(&str, &str) -> Result<u64, &'static str>,
{
    if bytes.len() < BefHeader::SIZE {
        return Err("file too small");
    }

    let header = unsafe { &*(bytes.as_ptr() as *const BefHeader) };
    if !header.is_valid() {
        return Err("invalid header");
    }

    let table_offset = header.section_table_offset as usize;
    let table_size = header.section_count as usize * SectionEntry::SIZE;
    if table_offset + table_size > bytes.len() {
        return Err("section table out of bounds");
    }

    let entries = unsafe {
        core::slice::from_raw_parts(
            bytes[table_offset..].as_ptr() as *const SectionEntry,
            header.section_count as usize,
        )
    };

    let base = if base_addr > 0 { base_addr } else { 0x7F00_0000_0000 };
    let mut loaded = Vec::new();
    let mut current_va = base;

    for (_i, entry) in entries.iter().enumerate() {
        let kind = SectionKind::from_u8(entry.kind).ok_or("unknown section kind")?;
        let size = entry.mem_size as usize;

        let align = (entry.alignment as u64).max(8);
        current_va = (current_va + align - 1) & !(align - 1);

        let mut data = Vec::with_capacity(size);

        if entry.kind == SectionKind::Bss as u8 {
            data.resize(size, 0);
        } else {
            let file_start = entry.file_offset as usize;
            let file_end = file_start + entry.file_size as usize;
            if file_end > bytes.len() {
                return Err("section data out of bounds");
            }
            data.extend_from_slice(&bytes[file_start..file_end]);
            if data.len() < size {
                data.resize(size, 0);
            }
        }

        loaded.push(LoadedSection {
            kind,
            virt_addr: current_va,
            size: size as u64,
            data,
        });

        current_va += size as u64;
    }

    // Resolve imports
    if let Some(section) = loaded.iter().find(|s| s.kind == SectionKind::Imports) {
        if let Ok(import_table) = ImportTable::parse(&section.data, 256) {
            for entry in import_table.entries {
                let _lib = import_table.library_name(entry).unwrap_or("");
                let _sym = import_table.symbol_name(entry).unwrap_or("");
            }
        }
    }

    // Apply relocations
    if let Some(section) = loaded.iter().find(|s| s.kind == SectionKind::Relocs) {
        let reloc_bytes = &section.data;
        if !reloc_bytes.is_empty() {
            let reloc_slice = unsafe {
                core::slice::from_raw_parts(
                    reloc_bytes.as_ptr() as *const relocations::Relocation,
                    reloc_bytes.len() / relocations::Relocation::SIZE,
                )
            };
            for reloc in reloc_slice {
                let target_idx = reloc.target_section as usize;
                if target_idx >= loaded.len() {
                    return Err("relocation target section out of range");
                }
                let reloc_va = loaded[target_idx].virt_addr + reloc.offset;
                let symbol_addr = reloc.symbol_idx as u64;
                let _ = relocations::apply(reloc, &mut loaded[target_idx].data, reloc_va, symbol_addr);
            }
        }
    }

    // Setup TLS
    let tls_base = if let Some(section) = loaded.iter().find(|s| s.kind == SectionKind::Tls) {
        let tls_bytes = &section.data;
        if tls_bytes.len() >= core::mem::size_of::<crate::bmo_abi::bef::tls::TlsTemplate>() {
            let template = unsafe { &*(tls_bytes.as_ptr() as *const crate::bmo_abi::bef::tls::TlsTemplate) };
            let data_start = core::mem::size_of::<crate::bmo_abi::bef::tls::TlsTemplate>();
            let data = if data_start < tls_bytes.len() { &tls_bytes[data_start..] } else { &[] };
            crate::bmo_abi::bef::tls::setup_for_thread(template, data).unwrap_or(0)
        } else { 0 }
    } else { 0 };

    let entry_point = loaded.iter()
        .find(|s| s.kind == SectionKind::Code)
        .map(|s| s.virt_addr + header.entry_offset)
        .unwrap_or(0);

    Ok(LoadedBef {
        entry_point,
        sections: loaded,
        tls_base,
        base_addr: base,
    })
}

pub fn no_imports(_lib: &str, _sym: &str) -> Result<u64, &'static str> {
    Err("no imports resolver configured")
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use crate::bmo_abi::bef::writer::{BefBuilder, BefSection};

    #[test]
    fn load_self_contained() {
        let mut b = BefBuilder::new();
        b.add_section(BefSection::code(vec![0xC3; 16]));
        let bytes = b.build().unwrap();
        let loaded = load(&bytes, 0, no_imports).unwrap();
        assert!(loaded.entry_point > 0);
    }
}
