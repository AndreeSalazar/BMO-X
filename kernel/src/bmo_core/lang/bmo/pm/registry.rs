//! ÑEXO Package Registry — Registro local de paquetes.
//!
//! Registry local (sin red) — paquetes disponibles en el RAMdisk
//! o en la RAM filesystem.

#![allow(dead_code)]

extern crate alloc;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use alloc::collections::BTreeMap;

/// Package entry in the registry.
#[derive(Debug, Clone)]
pub struct PackageEntry {
    pub name: String,
    pub versions: Vec<String>,
    pub description: String,
    pub dependencies: BTreeMap<String, String>, // name → version_req
}

/// Local package registry.
pub struct Registry {
    packages: BTreeMap<String, PackageEntry>,
}

impl Registry {
    pub fn new() -> Self {
        Self { packages: BTreeMap::new() }
    }

    /// Load registry from RAMdisk.
    pub fn load() -> Self {
        let mut reg = Self::new();
        reg.load_builtin_packages();
        reg
    }

    /// Register built-in packages (stdlib modules).
    fn load_builtin_packages(&mut self) {
        let builtins = [
            ("nexo_io", "0.1.0", "E/S serial y framebuffer"),
            ("nexo_mem", "0.1.0", "Gestión de memoria"),
            ("nexo_str", "0.1.0", "Operaciones con strings"),
            ("nexo_math", "0.1.0", "Aritmética"),
            ("nexo_fs", "0.1.0", "Sistema de archivos"),
            ("nexo_proc", "0.1.0", "Gestión de procesos"),
            ("nexo_time", "0.1.0", "Reloj y temporización"),
            ("nexo_gfx", "0.1.0", "Primitivas gráficas"),
            ("nexo_sys", "0.1.0", "Llamadas al sistema"),
        ];

        for (name, version, desc) in builtins {
            self.packages.insert(name.to_string(), PackageEntry {
                name: name.to_string(),
                versions: vec![version.to_string()],
                description: desc.to_string(),
                dependencies: BTreeMap::new(),
            });
        }
    }

    /// Search for a package by name.
    pub fn find(&self, name: &str) -> Option<&PackageEntry> {
        self.packages.get(name)
    }

    /// Check if a package exists with a compatible version.
    pub fn satisfies(&self, name: &str, version_req: &str) -> bool {
        if let Some(entry) = self.packages.get(name) {
            entry.versions.iter().any(|v| v == version_req || version_req == "*")
        } else {
            false
        }
    }

    /// List all packages.
    pub fn list(&self) -> Vec<&PackageEntry> {
        self.packages.values().collect()
    }

    /// Register a new package.
    pub fn register(&mut self, entry: PackageEntry) {
        self.packages.insert(entry.name.clone(), entry);
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}
