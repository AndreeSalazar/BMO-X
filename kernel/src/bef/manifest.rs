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

    /// Parse a minimal TOML manifest from raw bytes.
    ///
    /// Supports only the top-level keys needed for BEF:
    ///   [identity]
    ///   name = "..."
    ///   version = "X.Y.Z"
    ///   publisher = "..."
    ///
    ///   [capabilities]
    ///   fs_read = true
    ///   sys_time_hires = true
    ///   ... (maps to Capability flags)
    ///
    /// Returns a synthetic manifest on parse failure.
    pub fn parse_toml(bytes: &[u8], prov: Provenance) -> Self {
        let text = match core::str::from_utf8(bytes) {
            Ok(s) => s,
            Err(_) => return Self::synthetic_for("(invalid-utf8)", prov),
        };

        let mut name = alloc::string::String::new();
        let mut version_str = alloc::string::String::new();
        let mut publisher = alloc::string::String::new();
        let mut caps = Capability::NONE;

        let mut in_identity = false;
        let mut in_capabilities = false;

        for line in text.lines() {
            let trimmed = line.trim();

            // Skip empty lines and comments.
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            // Section headers.
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                let section = &trimmed[1..trimmed.len() - 1].trim();
                in_identity = *section == "identity";
                in_capabilities = *section == "capabilities";
                continue;
            }

            // Key = value pairs.
            if let Some(eq_pos) = trimmed.find('=') {
                let key = trimmed[..eq_pos].trim();
                let value = trimmed[eq_pos + 1..].trim();

                if in_identity {
                    match key {
                        "name" => {
                            name = parse_toml_string(value);
                        }
                        "version" => {
                            version_str = parse_toml_string(value);
                        }
                        "publisher" => {
                            publisher = parse_toml_string(value);
                        }
                        _ => {}
                    }
                } else if in_capabilities {
                    match key {
                        "fs_read" if value == "true" => caps |= Capability::FS_READ,
                        "fs_write" if value == "true" => caps |= Capability::FS_WRITE,
                        "sys_time_hires" if value == "true" => caps |= Capability::SYS_TIME_HIRES,
                        "sys_debug" if value == "true" => caps |= Capability::SYS_DEBUG,
                        "net_raw" if value == "true" => caps |= Capability::NET_RAW,
                        _ => {}
                    }
                }
            }
        }

        // Parse version string "X.Y.Z".
        let version = parse_semver(&version_str);

        Self {
            identity: Identity {
                name: if name.is_empty() { alloc::string::String::from("(unnamed)") } else { name },
                version,
                publisher: if publisher.is_empty() { alloc::string::String::from("(unknown)") } else { publisher },
                binary_hash: [0; 32],
            },
            capabilities: caps,
            dependencies: alloc::vec::Vec::new(),
            provenance: prov,
        }
    }
}

/// Parse a TOML string value (handles quotes).
fn parse_toml_string(value: &str) -> alloc::string::String {
    let v = value.trim();
    if v.starts_with('"') && v.ends_with('"') && v.len() >= 2 {
        alloc::string::String::from(&v[1..v.len() - 1])
    } else {
        alloc::string::String::from(v)
    }
}

/// Parse a semver string "X.Y.Z".
fn parse_semver(s: &str) -> SemVer {
    let mut major = 0u16;
    let mut minor = 0u16;
    let mut patch = 0u16;

    let parts: alloc::vec::Vec<&str> = s.split('.').collect();
    if parts.len() >= 1 { major = parts[0].parse().unwrap_or(0); }
    if parts.len() >= 2 { minor = parts[1].parse().unwrap_or(0); }
    if parts.len() >= 3 { patch = parts[2].parse().unwrap_or(0); }

    SemVer { major, minor, patch }
}
