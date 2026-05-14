//! Manifest BEF — TOML inline declarando metadata + capabilities.
//!
//! Reemplaza:
//!   - PE: `IMAGE_RESOURCE_DIRECTORY` con manifest XML embebido.
//!   - ELF: `.note.gnu.property` (sintaxis críptica) y archivos `.desktop` aparte.
//!
//! Está en una sección dedicada para que el loader lo lea ANTES de mapear
//! el código, decida sandbox + capabilities, y luego ejecute.

#![allow(dead_code)]

use crate::sandbox::Capability;

/// Manifest decodificado en memoria. La fuente es TOML pero aquí mostramos
/// la estructura tipada que esperamos extraer.
#[derive(Debug, Clone)]
pub struct Manifest {
    pub identity: Identity,
    pub capabilities: Capability,
    pub dependencies: alloc::vec::Vec<Dependency>,
    pub provenance: Provenance,
}

#[derive(Debug, Clone)]
pub struct Identity {
    pub name: alloc::string::String,
    pub version: SemVer,
    pub publisher: alloc::string::String,
    /// Hash BLAKE3 del binario (excluyendo la sección Signature).
    pub binary_hash: [u8; 32],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SemVer {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

#[derive(Debug, Clone)]
pub struct Dependency {
    pub name: alloc::string::String,
    pub min_version: SemVer,
    pub kind: DependencyKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DependencyKind {
    /// Otra librería BEF.
    BefLibrary,
    /// API BareX (devour-friendly).
    BarexApi,
    /// Driver del kernel.
    Driver,
}

/// De dónde viene este binario.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provenance {
    /// Compilado nativamente como BEF.
    Native,
    /// PE de Windows devorado por `loader::pe`.
    PeDevoured,
    /// ELF de Linux devorado por `loader::elf`.
    ElfDevoured,
}

impl Manifest {
    /// Manifest mínimo sintetizado por el devour-loader cuando un PE/ELF
    /// no incluye metadata BMO.
    pub fn synthetic_for(name: &str, prov: Provenance) -> Self {
        let caps = match prov {
            Provenance::Native => Capability::NONE,
            // PE/ELF devorados: sandbox restrictivo por defecto.
            Provenance::PeDevoured | Provenance::ElfDevoured => {
                Capability::FS_READ | Capability::SYS_TIME_HIRES
            }
        };
        Self {
            identity: Identity {
                name: alloc::string::String::from(name),
                version: SemVer { major: 0, minor: 0, patch: 0 },
                publisher: alloc::string::String::from("(devoured)"),
                binary_hash: [0; 32],
            },
            capabilities: caps,
            dependencies: alloc::vec::Vec::new(),
            provenance: prov,
        }
    }
}
