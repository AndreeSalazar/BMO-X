//! Loader BEF nativo.
//!
//! Pipeline:
//!   1. Validar header + magic + versión.
//!   2. Parsear section table.
//!   3. Verificar hashes BLAKE3 (sección Signature).
//!   4. Cargar manifest TOML → resolver capabilities.
//!   5. Mapear secciones (RO/RW/RX) en el address space del proceso.
//!   6. Aplicar relocations (`Abs64`/`Rel32`/`Got64`).
//!   7. Resolver imports (eager o instalar trampolines lazy).
//!   8. Setup TLS template del thread principal.
//!   9. Saltar al `entry_point`.

#![allow(dead_code)]

use crate::bef::header::{BefHeader, BEF_MAGIC};
use crate::bef::sections::SectionTable;
use super::{Image, LoadError, fake_provenance_image};
use crate::bef::manifest::Provenance;

pub fn load(bytes: &[u8]) -> Result<Image, LoadError> {
    if bytes.len() < BefHeader::SIZE {
        return Err(LoadError::Truncated);
    }
    // SAFETY: alignment garantizado por el chequeo de tamaño + repr(C, align(16))
    // sobre input. Para uso real, copiar a struct local antes.
    let hdr = unsafe { &*(bytes.as_ptr() as *const BefHeader) };
    if hdr.magic != BEF_MAGIC {
        return Err(LoadError::InvalidHeader);
    }
    if !hdr.is_valid() {
        return Err(LoadError::InvalidHeader);
    }
    if hdr.arch != crate::bef::header::BefArch::X86_64 as u8 {
        return Err(LoadError::UnsupportedArch);
    }
    if hdr.abi_version_major != 1 {
        return Err(LoadError::UnsupportedAbi);
    }

    let _table = SectionTable::parse(bytes, hdr.section_table_offset, hdr.section_count)
        .map_err(|_| LoadError::SectionOutOfRange)?;

    // TODO: resto del pipeline. Por ahora devolvemos un Image vacío con
    // provenance Native para que el kernel tenga algo bien-formado.
    let mut img = fake_provenance_image(Provenance::Native);
    img.entry_point = hdr.entry_offset;
    Ok(img)
}
