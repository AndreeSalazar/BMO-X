//! `devour` — Coordinador de devoradores PE/ELF en Ring 0.
//!
//! Toma un binario PE/ELF, llama al devorador correspondiente,
//! y entrega el BEF resultante al loader para ejecución directa.
//!
//! # Flujo
//!
//! ```text
//! bytes (PE/ELF/BEF)
//!   → BefMagic::detect()
//!     ├── BefNative → loader::load() directo (ya es BEF)
//!     ├── PeWindows → bmo_devour_pe::devour_pe()
//!     ├── ElfUnix   → bmo_devour_elf::devour_elf()
//!     └── Unknown   → error
//!   → loader::load() (ejecuta el BEF)
//! ```
//!
//! # Zero-copy
//!
//! Los devoradores producen BEF en buffers de memoria (Vec<u8>). El loader
//! los mapea directamente sin escribir a disco. El binario original se
//! descarta tras la traducción (o se cachea en RAM).

use bmo_abi::bef::{BefMagic, load, LoadedBef};

/// Determina el formato y devora/ejecuta el binario.
///
/// Acepta BEF, PE o ELF. Si es BEF nativo, lo carga directo.
/// Si es PE o ELF, lo traduce a BEF primero (devour), luego lo carga.
pub fn devour_and_load(bytes: &[u8]) -> Result<LoadedBef, &'static str> {
    match BefMagic::detect(bytes) {
        BefMagic::BefNative => {
            load(bytes, 0, resolve_import)
        }
        BefMagic::PeWindows => {
            devour_pe_then_load(bytes)
        }
        BefMagic::ElfUnix => {
            devour_elf_then_load(bytes)
        }
        BefMagic::Unknown => {
            Err("unknown binary format: not BEF, PE, or ELF")
        }
    }
}

fn resolve_import(_lib: &str, _sym: &str) -> Result<u64, &'static str> {
    // TODO: conectar con el sistema de imports del kernel
    Err("import resolution not yet wired")
}

fn devour_pe_then_load(_pe_bytes: &[u8]) -> Result<LoadedBef, &'static str> {
    // TODO: llamar a bmo_devour_pe::devour_pe() → luego load()
    Err("PE devourer not yet linked — compile with bmo-devour-pe feature")
}

fn devour_elf_then_load(_elf_bytes: &[u8]) -> Result<LoadedBef, &'static str> {
    // TODO: llamar a bmo_devour_elf::devour_elf() → luego load()
    Err("ELF devourer not yet linked — compile with bmo-devour-elf feature")
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
    fn detect_pe_returns_devour_error() {
        let pe_bytes = [b'M', b'Z', 0x90, 0x00, 0x00, 0x00];
        let result = devour_and_load(&pe_bytes);
        assert!(result.is_err());
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
