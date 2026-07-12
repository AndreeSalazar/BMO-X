//! Manifest BEF — TOML inline declarando metadata + capabilities.
//!
//! Reemplaza:
//!   - PE: IMAGE_RESOURCE_DIRECTORY con manifest XML embebido.
//!   - ELF: .note.gnu.property (sintaxis críptica) y archivos .desktop aparte.
//!
//! El loader lo lee ANTES de mapear código, decide sandbox + capabilities.
//!
//! ## Capability-based linking
//! Cada BEF declara qué capacidades PROVEE (provides) y cuáles REQUIERE (requires).
//! El kernel/linker resuelve requires → provides entre módulos al cargar.

#![allow(dead_code)]

use crate::fs::Capabilities;

/// Manifest decodificado en memoria.
#[derive(Debug, Clone)]
pub struct Manifest {
    pub identity: Identity,
    pub capabilities: Capabilities,
    /// Capacidades que este BEF PROVEE a otros (ej: "framebuffer.write").
    pub provides: alloc::vec::Vec<alloc::string::String>,
    /// Capacidades que este BEF REQUIERE de otros (ej: "net.tcp.connect").
    pub requires: alloc::vec::Vec<alloc::string::String>,
    pub dependencies: alloc::vec::Vec<Dependency>,
    pub provenance: Provenance,
}

#[derive(Debug, Clone)]
pub struct Identity {
    pub name: alloc::string::String,
    pub version: SemVer,
    pub publisher: alloc::string::String,
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
    BefLibrary,
    BarexApi,
    Driver,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provenance {
    Native,
    PeDevoured,
    ElfDevoured,
}

impl Manifest {
    pub fn synthetic_for(name: &str, prov: Provenance) -> Self {
        let caps = match prov {
            Provenance::Native => Capabilities::NONE,
            Provenance::PeDevoured | Provenance::ElfDevoured => {
                let mut c = Capabilities::FS_READ;
                c.insert(Capabilities::SYS_TIME_HIRES);
                c
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
            provides: alloc::vec::Vec::new(),
            requires: alloc::vec::Vec::new(),
            dependencies: alloc::vec::Vec::new(),
            provenance: prov,
        }
    }

    /// Full TOML parser — identity, capabilities, provides, requires, dependencies.
    pub fn parse_toml(bytes: &[u8], prov: Provenance) -> Self {
        let text = match core::str::from_utf8(bytes) {
            Ok(s) => s,
            Err(_) => return Self::synthetic_for("(invalid-utf8)", prov),
        };

        let mut name = alloc::string::String::new();
        let mut version_str = alloc::string::String::new();
        let mut publisher = alloc::string::String::new();
        let mut caps = Capabilities::NONE;
        let mut provides = alloc::vec::Vec::new();
        let mut requires = alloc::vec::Vec::new();
        let mut dependencies = alloc::vec::Vec::new();

        let mut section = alloc::string::String::new();

        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') { continue; }

            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                section = alloc::string::String::from(trimmed[1..trimmed.len()-1].trim());
                continue;
            }

            if let Some(eq_pos) = trimmed.find('=') {
                let key = trimmed[..eq_pos].trim();
                let value = trimmed[eq_pos + 1..].trim();

                match section.as_str() {
                    "identity" => match key {
                        "name" => name = parse_toml_string(value),
                        "version" => version_str = parse_toml_string(value),
                        "publisher" => publisher = parse_toml_string(value),
                        _ => {}
                    },
                    "capabilities" => {
                        match key {
                            "fs_read" if value == "true" => caps.insert(Capabilities::FS_READ),
                            "fs_write" if value == "true" => caps.insert(Capabilities::FS_WRITE),
                            "sys_time_hires" if value == "true" => caps.insert(Capabilities::SYS_TIME_HIRES),
                            "sys_debug" if value == "true" => caps.insert(Capabilities::SYS_DEBUG),
                            "net_raw" if value == "true" => caps.insert(Capabilities::NET_RAW),
                            "gpu_direct" if value == "true" => caps.insert(Capabilities(1 << 10)),
                            "audio_output" if value == "true" => caps.insert(Capabilities(1 << 11)),
                            "ipc_send" if value == "true" => caps.insert(Capabilities(1 << 12)),
                            _ => {}
                        }
                    },
                    "provides" => {
                        let cap_name = parse_toml_string(value);
                        if !cap_name.is_empty() && value == "true" {
                            provides.push(key.into());
                        } else if key == "list" {
                            // Comma-separated list of provided capabilities
                            for item in parse_toml_string(value).split(',') {
                                let item = item.trim();
                                if !item.is_empty() { provides.push(item.into()); }
                            }
                        }
                    },
                    "requires" => {
                        if key == "list" {
                            for item in parse_toml_string(value).split(',') {
                                let item = item.trim();
                                if !item.is_empty() { requires.push(item.into()); }
                            }
                        } else if value == "true" {
                            requires.push(key.into());
                        }
                    },
                    "dependencies" => {
                        let dep_name = parse_toml_string(value);
                        if !dep_name.is_empty() {
                            dependencies.push(Dependency {
                                name: dep_name,
                                min_version: SemVer { major: 0, minor: 0, patch: 0 },
                                kind: DependencyKind::BefLibrary,
                            });
                        }
                    },
                    _ => {}
                }
            }
        }

        let version = parse_semver(&version_str);

        Self {
            identity: Identity {
                name: if name.is_empty() { alloc::string::String::from("(unnamed)") } else { name },
                version,
                publisher: if publisher.is_empty() { alloc::string::String::from("(unknown)") } else { publisher },
                binary_hash: [0; 32],
            },
            capabilities: caps,
            provides,
            requires,
            dependencies,
            provenance: prov,
        }
    }
}

fn parse_toml_string(value: &str) -> alloc::string::String {
    let v = value.trim();
    if v.starts_with('"') && v.ends_with('"') && v.len() >= 2 {
        alloc::string::String::from(&v[1..v.len() - 1])
    } else {
        alloc::string::String::from(v)
    }
}

fn parse_semver(s: &str) -> SemVer {
    let mut major = 0u16; let mut minor = 0u16; let mut patch = 0u16;
    let parts: alloc::vec::Vec<&str> = s.split('.').collect();
    if parts.len() >= 1 { major = parts[0].parse().unwrap_or(0); }
    if parts.len() >= 2 { minor = parts[1].parse().unwrap_or(0); }
    if parts.len() >= 3 { patch = parts[2].parse().unwrap_or(0); }
    SemVer { major, minor, patch }
}
