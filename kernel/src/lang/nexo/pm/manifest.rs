//! ÑEXO Package Manifest — Parser de nexo.toml.
//!
//! Formato simplificado (TOML-subset):
//!
//! ```toml
//! [package]
//! name = "mi_proyecto"
//! version = "0.1.0"
//! author = "autor"
//! description = "Descripcion del proyecto"
//!
//! [dependencies]
//! nexo_io = "0.1.0"
//! nexo_fs = { version = "0.2.0", optional = true }
//!
//! [dev-dependencies]
//! nexo_test = "0.1.0"
//! ```

#![allow(dead_code)]

extern crate alloc;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use alloc::format;
use alloc::collections::BTreeMap;

use crate::barex::BxResult;

/// Package manifest.
#[derive(Debug, Clone)]
pub struct Manifest {
    pub package: PackageInfo,
    pub dependencies: BTreeMap<String, DependencySpec>,
    pub dev_dependencies: BTreeMap<String, DependencySpec>,
}

/// Package metadata.
#[derive(Debug, Clone)]
pub struct PackageInfo {
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
}

/// Dependency specification.
#[derive(Debug, Clone)]
pub enum DependencySpec {
    Version(String),
    Detailed { version: String, optional: bool, path: Option<String> },
}

impl DependencySpec {
    pub fn version(&self) -> &str {
        match self {
            DependencySpec::Version(v) => v,
            DependencySpec::Detailed { version, .. } => version,
        }
    }

    pub fn is_optional(&self) -> bool {
        match self {
            DependencySpec::Version(_) => false,
            DependencySpec::Detailed { optional, .. } => *optional,
        }
    }

    pub fn path(&self) -> Option<&str> {
        match self {
            DependencySpec::Version(_) => None,
            DependencySpec::Detailed { path, .. } => path.as_deref(),
        }
    }
}

/// Simple TOML-like parser for nexo.toml.
pub struct ManifestParser {
    lines: Vec<(usize, String)>,
    pos: usize,
}

impl ManifestParser {
    pub fn new(source: &str) -> Self {
        let lines: Vec<(usize, String)> = source.lines()
            .enumerate()
            .map(|(i, l)| (i, l.trim().to_string()))
            .filter(|(_, l)| !l.is_empty() && !l.starts_with('#'))
            .collect();
        Self { lines, pos: 0 }
    }

    fn peek(&self) -> Option<&str> {
        self.lines.get(self.pos).map(|(_, l)| l.as_str())
    }

    fn advance(&mut self) -> Option<&str> {
        let line = self.lines.get(self.pos).map(|(_, l)| l.as_str());
        if line.is_some() { self.pos += 1; }
        line
    }

    fn parse_section(&mut self) -> Option<String> {
        let line = self.peek()?.to_string();
        if line.starts_with('[') && line.ends_with(']') {
            let name = line[1..line.len()-1].to_string();
            self.advance();
            Some(name)
        } else {
            None
        }
    }

    fn parse_key_value(&mut self) -> Option<(String, String)> {
        let line = self.peek()?.to_string();
        if let Some(eq_pos) = line.find('=') {
            let key = line[..eq_pos].trim().to_string();
            let val = line[eq_pos+1..].trim().to_string();
            let val = val.strip_prefix('"').unwrap_or(&val).to_string();
            let val = val.strip_suffix('"').unwrap_or(&val).to_string();
            self.advance();
            Some((key, val))
        } else {
            None
        }
    }

    fn parse_inline_table(&mut self, line: &str) -> DependencySpec {
        // { version = "0.1.0", optional = true }
        let mut version = String::new();
        let mut optional = false;
        let mut path = None;

        let content = line.trim().strip_prefix('{').unwrap_or(line);
        let content = content.strip_suffix('}').unwrap_or(content);

        for part in content.split(',') {
            let part = part.trim();
            if let Some((k, v)) = part.split_once('=') {
                let k = k.trim();
                let v = v.trim().strip_prefix('"').unwrap_or(v).strip_suffix('"').unwrap_or(v);
                match k {
                    "version" => version = v.to_string(),
                    "optional" => optional = v == "true",
                    "path" => path = Some(v.to_string()),
                    _ => {}
                }
            }
        }

        DependencySpec::Detailed { version, optional, path }
    }

    /// Parse nexo.toml source into Manifest.
    pub fn parse(&mut self) -> BxResult<Manifest> {
        let mut package = PackageInfo {
            name: String::new(),
            version: "0.1.0".to_string(),
            author: String::new(),
            description: String::new(),
        };
        let mut dependencies = BTreeMap::new();
        let mut dev_dependencies = BTreeMap::new();
        let mut current_section = String::new();

        while self.peek().is_some() {
            if let Some(section) = self.parse_section() {
                current_section = section;
                continue;
            }

            if let Some((key, value)) = self.parse_key_value() {
                match current_section.as_str() {
                    "package" => {
                        match key.as_str() {
                            "name" => package.name = value,
                            "version" => package.version = value,
                            "author" => package.author = value,
                            "description" => package.description = value,
                            _ => {}
                        }
                    }
                    "dependencies" => {
                        if value.starts_with('{') {
                            dependencies.insert(key, self.parse_inline_table(&value));
                        } else {
                            dependencies.insert(key, DependencySpec::Version(value));
                        }
                    }
                    "dev-dependencies" => {
                        if value.starts_with('{') {
                            dev_dependencies.insert(key, self.parse_inline_table(&value));
                        } else {
                            dev_dependencies.insert(key, DependencySpec::Version(value));
                        }
                    }
                    _ => {}
                }
            } else {
                self.advance(); // skip unknown lines
            }
        }

        Ok(Manifest { package, dependencies, dev_dependencies })
    }
}

/// Parse a nexo.toml string into a Manifest.
pub fn parse_manifest(source: &str) -> BxResult<Manifest> {
    ManifestParser::new(source).parse()
}

/// Create a default manifest for a new project.
pub fn default_manifest(name: &str) -> Manifest {
    Manifest {
        package: PackageInfo {
            name: name.to_string(),
            version: "0.1.0".to_string(),
            author: String::new(),
            description: String::new(),
        },
        dependencies: BTreeMap::new(),
        dev_dependencies: BTreeMap::new(),
    }
}

/// Serialize a Manifest to nexo.toml format.
pub fn serialize_manifest(manifest: &Manifest) -> String {
    let mut out = String::new();
    out.push_str("[package]\n");
    out.push_str(&format!("name = \"{}\"\n", manifest.package.name));
    out.push_str(&format!("version = \"{}\"\n", manifest.package.version));
    if !manifest.package.author.is_empty() {
        out.push_str(&format!("author = \"{}\"\n", manifest.package.author));
    }
    if !manifest.package.description.is_empty() {
        out.push_str(&format!("description = \"{}\"\n", manifest.package.description));
    }
    if !manifest.dependencies.is_empty() {
        out.push_str("\n[dependencies]\n");
        for (name, spec) in &manifest.dependencies {
            match spec {
                DependencySpec::Version(v) => out.push_str(&format!("{} = \"{}\"\n", name, v)),
                DependencySpec::Detailed { version, optional, path } => {
                    let mut parts = vec![format!("version = \"{}\"", version)];
                    if *optional { parts.push("optional = true".to_string()); }
                    if let Some(p) = path { parts.push(format!("path = \"{}\"", p)); }
                    out.push_str(&format!("{} = {{ {} }}\n", name, parts.join(", ")));
                }
            }
        }
    }
    out
}
