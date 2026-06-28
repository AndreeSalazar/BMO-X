//! `bmo_devour_pe` — PE → BEF devourer.
//!
//! Toma un ejecutable PE (Portable Executable, formato nativo de Windows)
//! y lo traduce a BEF nativo (BMO Executable Format).
//!
//! # Pipeline
//!
//! 1. Parsear headers PE (DOS header → NT headers → section table)
//! 2. Traducir secciones PE a secciones BEF (.text → Code, .rdata → RoData, etc.)
//! 3. Resolver imports (IAT → BEF ImportTable)
//! 4. Traducir exports (EDT → BEF ExportTable)
//! 5. Traducir relocalizaciones (.reloc → BEF Relocation)
//! 6. Generar manifiesto con Provenance::PeDevoured
//! 7. Escribir BEF usando BefBuilder
//!
//! # Soporte
//!
//! - PE32+ (64-bit) con soporte para Windows 10/11 (2020–2026)
//! - Control-flow Guard (CFG) — se traduce como metadata
//! - Import Address Table (IAT) resuelta o delay-load
//! - TLS callbacks
//! - .NET metadata (opcional, se copia como sección Raw)
//!
//! # Limitaciones
//!
//! - No soporta PE32 (32-bit) — solo PE32+
//! - No soporta drivers firmados con WHQL (requiere descifrado de firmware)
//! - No ejecuta .NET JIT — pasa la sección .NET como dato

#![no_std]
extern crate alloc;

/// Resultado del devorado: BEF bytes + metadatos.
pub struct DevourResult {
    /// BEF completo listo para cargar/ejecutar.
    pub bef_bytes: alloc::vec::Vec<u8>,
    /// Offset del entry point original (RVA traducido a BEF).
    pub entry_offset: u64,
    /// Origen del binario (PE).
    pub provenance: bmo_abi::bef::Provenance,
    /// Advertencias encontradas durante el devorado.
    pub warnings: alloc::vec::Vec<&'static str>,
}

/// Devora un PE32+ en buffer BEF.
///
/// # Arguments
///
/// * `pe_bytes` — contenido completo del archivo PE (incluye DOS header)
///
/// # Returns
///
/// `DevourResult` con el BEF listo para el loader, o `&str` de error.
pub fn devour_pe(pe_bytes: &[u8]) -> Result<DevourResult, &'static str> {
    // TODO: implementar parseo de headers PE + traducción a BEF
    let _ = pe_bytes;
    Err("PE devourer no implementado aún")
}

#[cfg(test)]
mod tests {
    #[test]
    fn detect_pe_magic() {
        let mz = [b'M', b'Z', 0x90, 0x00];
        let magic = bmo_abi::bef::BefMagic::detect(&mz);
        assert_eq!(magic, bmo_abi::bef::BefMagic::PeWindows);
    }

    #[test]
    fn devour_minimal_pe_fails_not_implemented() {
        let result = crate::devour_pe(&[]);
        assert!(result.is_err());
    }
}
