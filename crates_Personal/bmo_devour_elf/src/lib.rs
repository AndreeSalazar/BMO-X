//! `bmo_devour_elf` — ELF → BEF devourer.
//!
//! Toma un ejecutable ELF (Executable and Linkable Format, nativo de Linux)
//! y lo traduce a BEF nativo (BMO Executable Format).
//!
//! # Pipeline
//!
//! 1. Parsear headers ELF (ELF header → Program headers → Section headers)
//! 2. Traducir segmentos/secciones ELF a secciones BEF
//! 3. Resolver imports (`.dynamic` `DT_NEEDED` → BEF ImportTable)
//! 4. Traducir PLT/GOT a relocalizaciones BEF (Rel32/Got64)
//! 5. Traducir TLS (`.tdata`/`.tbss` → BEF TlsTemplate)
//! 6. Generar manifiesto con Provenance::ElfDevoured
//! 7. Escribir BEF usando BefBuilder
//!
//! # Soporte
//!
//! - ELF64 x86_64 con `DT_GNU_HASH` (glibc 2.32+, Ubuntu 20.04+)
//! - PIE obligatorio (ASLR)
//! - FULL RELRO
//! - TLS nativo
//! - Thread Local Storage (`.tdata`/`.tbss`)
//!
//! # Limitaciones
//!
//! - No soporta ELF32 (32-bit)
//! - No soporta `ld.so` preloading externo
//! - No implementa `LD_PRELOAD` ni `LD_LIBRARY_PATH`

#![no_std]
extern crate alloc;

/// Resultado del devorado: BEF bytes + metadatos.
pub struct DevourResult {
    /// BEF completo listo para cargar/ejecutar.
    pub bef_bytes: alloc::vec::Vec<u8>,
    /// Offset del entry point original.
    pub entry_offset: u64,
    /// Origen del binario (ELF).
    pub provenance: bmo_abi::bef::Provenance,
    /// Advertencias encontradas durante el devorado.
    pub warnings: alloc::vec::Vec<&'static str>,
}

/// Devora un ELF64 en buffer BEF.
///
/// # Arguments
///
/// * `elf_bytes` — contenido completo del archivo ELF
///
/// # Returns
///
/// `DevourResult` con el BEF listo para el loader, o `&str` de error.
pub fn devour_elf(elf_bytes: &[u8]) -> Result<DevourResult, &'static str> {
    // TODO: implementar parseo de headers ELF + traducción a BEF
    let _ = elf_bytes;
    Err("ELF devourer no implementado aún")
}

#[cfg(test)]
mod tests {
    #[test]
    fn detect_elf_magic() {
        let elf = [0x7F, b'E', b'L', b'F'];
        let magic = bmo_abi::bef::BefMagic::detect(&elf);
        assert_eq!(magic, bmo_abi::bef::BefMagic::ElfUnix);
    }

    #[test]
    fn devour_minimal_elf_fails_not_implemented() {
        let result = crate::devour_elf(&[]);
        assert!(result.is_err());
    }
}
