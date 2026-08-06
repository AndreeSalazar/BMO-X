use crate::bmo_abi::bef::{
    header::*,
    imports::{ImportFlags, ImportTable},
    relocations,
    sections::*,
};
use alloc::vec::Vec;

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
    mut resolve_import: F,
) -> Result<LoadedBef, &'static str>
where
    F: FnMut(&str, &str) -> Result<u64, &'static str>,
{
    if bytes.len() < BefHeader::SIZE {
        return Err("file too small");
    }

    let header: BefHeader =
        unsafe { core::ptr::read_unaligned(bytes.as_ptr() as *const BefHeader) };
    if !header.is_valid() {
        return Err("invalid header");
    }

    let table_offset = header.section_table_offset as usize;
    let table_size = header.section_count as usize * SectionEntry::SIZE;
    if table_offset + table_size > bytes.len() {
        return Err("section table out of bounds");
    }

    // Read section entries via unaligned copies
    let mut entries: Vec<SectionEntry> = Vec::with_capacity(header.section_count as usize);
    let table_bytes = &bytes[table_offset..table_offset + table_size];
    for i in 0..header.section_count as usize {
        let off = i * SectionEntry::SIZE;
        let e: SectionEntry = unsafe {
            core::ptr::read_unaligned(table_bytes[off..].as_ptr() as *const SectionEntry)
        };
        entries.push(e);
    }

    let base = if base_addr > 0 {
        base_addr
    } else {
        0x7F00_0000_0000
    };
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

    // Resolve imports — collect entries first to avoid borrow conflict with patch_binding
    let import_entries: Vec<(u64, u64)> = {
        let imports_section = loaded.iter().find(|s| s.kind == SectionKind::Imports);
        match imports_section.and_then(|s| ImportTable::parse(&s.data, 256).ok()) {
            Some(table) => table
                .entries
                .iter()
                .filter_map(|entry| {
                    let sym = table.symbol_name(entry).unwrap_or("");
                    if sym.is_empty() {
                        return None;
                    }
                    let lib = table.library_name(entry).unwrap_or("");
                    let addr = resolve_import(lib, sym);
                    match addr {
                        Ok(resolved) if resolved != 0 => Some((entry.binding_offset, resolved)),
                        Ok(_) if entry.flags & ImportFlags::WEAK.bits() != 0 => {
                            Some((entry.binding_offset, 0))
                        }
                        _ => None,
                    }
                })
                .collect(),
            None => Vec::new(),
        }
    };
    for &(binding_offset, addr) in &import_entries {
        patch_binding(binding_offset, addr, &mut loaded)?;
    }

    // Apply relocations via read_unaligned
    let reloc_data: Vec<Vec<u8>> = loaded
        .iter()
        .filter(|s| s.kind == SectionKind::Relocs)
        .map(|s| s.data.clone())
        .collect();
    if let Some(reloc_bytes) = reloc_data.first() {
        let n = reloc_bytes.len() / relocations::Relocation::SIZE;
        for i in 0..n {
            let off = i * relocations::Relocation::SIZE;
            let reloc: relocations::Relocation = unsafe {
                core::ptr::read_unaligned(
                    reloc_bytes[off..].as_ptr() as *const relocations::Relocation
                )
            };
            let target_idx = reloc.target_section as usize;
            if target_idx >= loaded.len() {
                return Err("relocation target section out of range");
            }
            let reloc_va = loaded[target_idx].virt_addr + reloc.offset;
            let symbol_addr = reloc.symbol_idx as u64;
            // ★ ESTE RESULTADO NO SE TIRA, y antes sí.
            //
            // `apply` devuelve por qué no pudo —"offset Abs64 fuera de rango",
            // "kind de relocation desconocido"— y ese error se descartaba. Una
            // relocación que no se aplica deja en el binario la dirección SIN
            // CORREGIR: el cargador decía "cargado" y el programa saltaba a
            // donde apuntara la basura. El fallo aparecía luego, lejos, como un
            // #PF con una dirección sin sentido y nada que lo relacionara con
            // este momento.
            //
            // Un binario mal relocado no es un binario degradado: es otro
            // binario. Por eso aquí se corta y no se avisa y sigue.
            relocations::apply(&reloc, &mut loaded[target_idx].data, reloc_va, symbol_addr)?;
        }
    }

    // Setup TLS
    let tls_base = if let Some(section) = loaded.iter().find(|s| s.kind == SectionKind::Tls) {
        let tls_bytes = &section.data;
        if tls_bytes.len() >= core::mem::size_of::<crate::bmo_abi::bef::tls::TlsTemplate>() {
            let template: crate::bmo_abi::bef::tls::TlsTemplate = unsafe {
                core::ptr::read_unaligned(
                    tls_bytes.as_ptr() as *const crate::bmo_abi::bef::tls::TlsTemplate
                )
            };
            let data_start = core::mem::size_of::<crate::bmo_abi::bef::tls::TlsTemplate>();
            let data = if data_start < tls_bytes.len() {
                &tls_bytes[data_start..]
            } else {
                &[]
            };
            // El `unwrap_or(0)` que había aquí confundía dos cosas MUY
            // distintas: "este binario no usa TLS" (base 0, correcto) y "usa
            // TLS y no se pudo preparar" (base 0, y el primer acceso a una
            // variable de hilo lee la página cero). Dos causas, un valor.
            crate::bmo_abi::bef::tls::setup_for_thread(&template, data)?
        } else {
            0
        }
    } else {
        0
    };

    // ★ Un BEF sin sección de código daba `entry_point = 0` y el cargador
    // devolvía `Ok`. O sea: "cargado correctamente, salta a la dirección cero".
    // El fallo no era el salto —eso al menos hace ruido—: era que ESTA función
    // decía que todo había ido bien.
    let entry_point = loaded
        .iter()
        .find(|s| s.kind == SectionKind::Code)
        .map(|s| s.virt_addr + header.entry_offset)
        .ok_or("el BEF no trae seccion de codigo: no hay a donde saltar")?;

    Ok(LoadedBef {
        entry_point,
        sections: loaded,
        tls_base,
        base_addr: base,
    })
}

/// Write `addr` (8 bytes, little-endian) at `binding_offset` in the correct section.
fn patch_binding(
    binding_offset: u64,
    addr: u64,
    loaded: &mut [LoadedSection],
) -> Result<(), &'static str> {
    for section in loaded.iter_mut() {
        let start = section.virt_addr;
        let end = start + section.size;
        if binding_offset >= start && binding_offset + 8 <= end {
            let offset_in_section = (binding_offset - start) as usize;
            let data = &mut section.data;
            if offset_in_section + 8 > data.len() {
                return Err("binding offset out of data bounds");
            }
            data[offset_in_section..offset_in_section + 8].copy_from_slice(&addr.to_le_bytes());
            return Ok(());
        }
    }
    Err("binding offset outside all sections")
}

pub fn no_imports(_lib: &str, _sym: &str) -> Result<u64, &'static str> {
    Err("no imports resolver configured")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bmo_abi::bef::writer::{BefBuilder, BefSection};
    use alloc::vec;

    #[test]
    fn load_self_contained() {
        let mut b = BefBuilder::new();
        b.add_section(BefSection::code(vec![0xC3; 16]));
        let bytes = b.build().unwrap();
        let loaded = load(&bytes, 0, no_imports).unwrap();
        assert!(loaded.entry_point > 0);
    }
}
