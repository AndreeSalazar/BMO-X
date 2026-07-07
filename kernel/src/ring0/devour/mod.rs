//! devour — Coordinador de devoradores ELF en Ring 0.
//!
//! Toma un binario ELF/BEF, llama al devorador correspondiente,
//! y entrega el BEF resultante al loader para ejecución directa.
//!
//! # Flujo
//!
//! `	ext
//! bytes (ELF/BEF)
//!   ? BefMagic::detect()
//!     +-- BefNative ? loader::load() directo (ya es BEF)
//!     +-- ElfUnix   ? bmo_core::bef::parsers::elf::load()
//!     +-- Unknown   ? error
//!   ? loader::load() (ejecuta el BEF)
//! `
//!
//! # Zero-copy
//!
//! Los devoradores producen BEF en buffers de memoria (Vec<u8>). El loader
//! los mapea directamente sin escribir a disco. El binario original se
//! descarta tras la traducción (o se cachea en RAM).

use bmo_abi::bef::{BefMagic, LoadedBef, LoadedSection};
use bmo_abi::bef::sections::SectionKind;
use alloc::vec::Vec;

pub fn devour_and_load(bytes: &[u8]) -> Result<LoadedBef, &'static str> {
    let img = match BefMagic::detect(bytes) {
        BefMagic::BefNative => {
            return bmo_abi::bef::load(bytes, 0, resolve_import);
        }
        BefMagic::ElfUnix => {
            bmo_core::bef::parsers::elf::load(bytes)
                .map_err(|_| "ELF parse failed")?
        }
        _ => return Err("unknown binary format"),
    };
    Ok(image_to_loaded(&img))
}

fn resolve_import(lib: &str, sym: &str) -> Result<u64, &'static str> {
    let addr = bmo_core::bef::parsers::runtime::lookup(lib, sym);
    if addr != 0 {
        Ok(addr)
    } else {
        Err("unresolved import")
    }
}

fn image_to_loaded(img: &bmo_core::bef::parsers::Image) -> LoadedBef {
    let mut sections = Vec::with_capacity(img.sections.len());
    for s in &img.sections {
        let data = if s.data_ptr != 0 {
            unsafe {
                core::slice::from_raw_parts(s.data_ptr as *const u8, s.size as usize).to_vec()
            }
        } else {
            Vec::new()
        };
        sections.push(LoadedSection {
            kind: SectionKind::from_u8(s.kind).unwrap_or(SectionKind::Data),
            virt_addr: s.virt_addr,
            size: s.size,
            data,
        });
    }
    LoadedBef {
        entry_point: img.entry_point,
        sections,
        tls_base: 0,
        base_addr: img.baseess,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_bef_native() {
        let mut b = bmo_abi::bef::BefBuilder::new();
        b.add_section(bmo_abi::bef::BefSection::code(alloc::vec![0xC3; 16]));
        let bytes = b.build().unwrap();
        let loaded = devour_and_load(&bytes);
        assert!(loaded.is_ok());
    }

    #[test]
    fn detect_elf_returns_devour_error() {
        let elf_bytes = [0x7F, b'E', b'L', b'F', 0x02, 0x01];
        let result = devour_and_load(&elf_bytes);
        assert!(result.is_err());
    }

    #[test]
    fn detect_unknown_returns_error() {
        let result = devour_and_load(b"not a binary");
        assert!(result.is_err());
    }
}

