//! Semantic_ASM loader — shared syscall, type, and stdlib registry.
//!
//! Single source of truth for all language frontends (C, COBOL, C++).
//! Each frontend reads files from disk and passes the content here.

#![allow(dead_code)]

pub mod defs;

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

fn str_to_string(s: &str) -> String {
    String::from(s)
}

// ─ Syscall definitions ─────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyscallDef {
    pub name: String,
    pub nr: u32,
    pub arg_count: u8,
}

pub fn parse_syscall_file(content: &str) -> Vec<SyscallDef> {
    let mut defs = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(eq_pos) = line.find('=') {
            let name = str_to_string(line[..eq_pos].trim());
            let rest = line[eq_pos + 1..].trim();
            let (val_str, arg_count) = if let Some(comma_pos) = rest.find(',') {
                (
                    rest[..comma_pos].trim(),
                    rest[comma_pos + 1..].trim().parse::<u8>().unwrap_or(0),
                )
            } else {
                (rest, 0u8)
            };
            let nr = if val_str.starts_with("0x") || val_str.starts_with("0X") {
                u32::from_str_radix(&val_str[2..], 16).unwrap_or(0)
            } else {
                val_str.parse::<u32>().unwrap_or(0)
            };
            defs.push(SyscallDef {
                name,
                nr,
                arg_count,
            });
        }
    }
    defs
}

// ─ Type aliases ──────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeAlias {
    pub name: String,
    pub underlying: String,
    pub value: Option<i64>,
}

pub fn parse_types_file(content: &str) -> Vec<TypeAlias> {
    let mut aliases = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(eq_pos) = line.find('=') {
            let name = str_to_string(line[..eq_pos].trim());
            let val = line[eq_pos + 1..].trim().trim_matches('"');
            if let Ok(n) = val.parse::<i64>() {
                aliases.push(TypeAlias {
                    name,
                    underlying: String::new(),
                    value: Some(n),
                });
            } else {
                aliases.push(TypeAlias {
                    name,
                    underlying: str_to_string(val),
                    value: None,
                });
            }
        }
    }
    aliases
}

// ─ Stdlib manifest ────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StdlibExport {
    pub name: String,
    pub signature: String,
    pub return_type: String,
    pub param_types: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct StdlibManifest {
    pub exports: Vec<StdlibExport>,
}

pub fn parse_stdlib_manifest(content: &str) -> StdlibManifest {
    let mut exports = Vec::new();
    let mut in_exports = false;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line == "[exports]" {
            in_exports = true;
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            in_exports = false;
            continue;
        }
        if in_exports {
            if let Some(eq_pos) = line.find('=') {
                let name = str_to_string(line[..eq_pos].trim());
                let sig_str = str_to_string(line[eq_pos + 1..].trim().trim_matches('"'));
                let sig_clone = sig_str.clone();
                let arrow = sig_clone.find("->");
                let (params_str, ret_str): (&str, &str) = if let Some(pos) = arrow {
                    (&sig_clone[..pos], &sig_clone[pos + 2..])
                } else {
                    (&sig_clone, "void")
                };
                let param_types: Vec<String> = if params_str.is_empty() || params_str == "void" {
                    Vec::new()
                } else {
                    params_str
                        .split(',')
                        .map(|s| str_to_string(s.trim()))
                        .collect()
                };
                exports.push(StdlibExport {
                    name,
                    signature: sig_str,
                    return_type: str_to_string(ret_str.trim()),
                    param_types,
                });
            }
        }
    }
    StdlibManifest { exports }
}

// ─ Module manifest ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ModuleManifest {
    pub name: String,
    pub version: String,
    pub exports: Vec<String>,
    pub source_files: Vec<String>,
}

pub fn parse_module_manifest(content: &str, default_name: &str) -> ModuleManifest {
    let mut name = str_to_string(default_name);
    let mut version = str_to_string("0.1.0");
    let mut exports = Vec::new();
    let mut source_files = Vec::new();
    let mut current_section = String::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            current_section = str_to_string(line[1..line.len() - 1].trim());
            continue;
        }
        if let Some((key, val)) = line.split_once('=') {
            let key = key.trim();
            let val = val.trim().trim_matches('"');
            match current_section.as_str() {
                "module" => match key {
                    "name" => name = str_to_string(val),
                    "version" => version = str_to_string(val),
                    _ => {}
                },
                "exports" => {
                    if key == "functions" {
                        for f in val.split(',').map(|s| s.trim().trim_matches('"')) {
                            if !f.is_empty() {
                                exports.push(str_to_string(f));
                            }
                        }
                    } else {
                        exports.push(str_to_string(key));
                    }
                }
                "sources" => {
                    if key == "files" {
                        for f in val.split(',').map(|s| s.trim().trim_matches('"')) {
                            if !f.is_empty() {
                                source_files.push(str_to_string(f));
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    ModuleManifest {
        name,
        version,
        exports,
        source_files,
    }
}

// ─ ABI data model ──────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AbiDataModel {
    pub pointer_size: u8,
    pub endianness: String,
    pub type_sizes: BTreeMap<String, u8>,
}

pub fn parse_abi_file(content: &str) -> AbiDataModel {
    let mut pointer_size: u8 = 8;
    let mut endianness = str_to_string("little");
    let mut type_sizes = BTreeMap::new();
    let mut current_section = String::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            current_section = str_to_string(line[1..line.len() - 1].trim());
            continue;
        }
        if let Some((key, val)) = line.split_once('=') {
            let key = key.trim();
            let val = val.trim().trim_matches('"');
            match current_section.as_str() {
                "data_model" => match key {
                    "pointer_size" => pointer_size = val.parse().unwrap_or(8),
                    "endianness" => endianness = str_to_string(val),
                    _ => {}
                },
                "type_sizes" => {
                    if let Ok(size) = val.parse::<u8>() {
                        type_sizes.insert(str_to_string(key), size);
                    }
                }
                _ => {}
            }
        }
    }
    AbiDataModel {
        pointer_size,
        endianness,
        type_sizes,
    }
}
