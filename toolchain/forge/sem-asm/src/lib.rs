//! Motor de codificación semántica (`sem-asm`) — la 3ª librería del pipeline.
//!
//! Lee las tablas TOML de `tables/` y encodea mnemónicos → bytes. Reemplaza
//! el hardcodeo de bytes DUPLICADO en `lang/c/src/codegen.rs` y
//! `lang/cobol/src/codegen.rs` (ambos escriben 0x48/0xB8… a mano).
//!
//! Librería que el frontend ELIGE enlazar, no un embudo.

use std::path::{Path, PathBuf};

pub mod x86_64;

/// Ubica `tables/` relativo a este crate — sin depender de las rutas muertas
/// `X:\FastOS\...` que arrastraban los frontends.
pub fn tables_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tables")
}

/// Bytes de una instrucción (x86-64: 1..15 bytes).
pub type Encoded = Vec<u8>;

#[derive(Debug)]
pub enum SemAsmError {
    TablesNotFound(PathBuf),
    Parse(String),
    UnknownMnemonic(String),
}

/// Tabla de instrucciones cargada desde `arch/<isa>/instructions.toml`.
///
/// El TOML es heterogéneo (unas entradas traen `opcode = [..]`, otras
/// `opcode_base`, otras una lista `forms`), así que se navega como
/// `toml::Value` en vez de forzar un `struct` rígido — robusto a la forma.
pub struct Instructions {
    root: toml::Table,
}

impl Instructions {
    /// Carga `tables/arch/x86_64/instructions.toml`.
    pub fn load_x86_64() -> Result<Self, SemAsmError> {
        let path = tables_dir().join("arch/x86_64/instructions.toml");
        let text = std::fs::read_to_string(&path)
            .map_err(|_| SemAsmError::TablesNotFound(path))?;
        let root: toml::Table = text.parse().map_err(|e| SemAsmError::Parse(format!("{e}")))?;
        Ok(Self { root })
    }

    /// Nombre de la ISA (`[instruction_set].name`), si está.
    pub fn isa_name(&self) -> Option<&str> {
        self.root.get("instruction_set")?.get("name")?.as_str()
    }

    /// Opcode base de un mnemónico. Cubre las tres formas del TOML:
    /// `opcode = [..]`, `opcode_base = 0x..`, o la 1ª entrada de `forms`.
    /// Es el primer ladrillo del encoder; la codificación completa de
    /// operandos (ModRM/REX/imm) se construye encima en pasos siguientes.
    pub fn opcode(&self, mnemonic: &str) -> Result<Encoded, SemAsmError> {
        let entry = self
            .root
            .get(mnemonic)
            .ok_or_else(|| SemAsmError::UnknownMnemonic(mnemonic.to_string()))?;

        // Forma A: opcode = [0x..]
        if let Some(arr) = entry.get("opcode").and_then(|v| v.as_array()) {
            return Ok(arr.iter().filter_map(toml_byte).collect());
        }
        // Forma B: opcode_base = 0x..
        if let Some(b) = entry.get("opcode_base").and_then(toml_byte) {
            return Ok(vec![b]);
        }
        // Forma C: forms = [ { opcode = [..] }, .. ] — toma la primera.
        if let Some(forms) = entry.get("forms").and_then(|v| v.as_array()) {
            if let Some(first) = forms.first() {
                if let Some(arr) = first.get("opcode").and_then(|v| v.as_array()) {
                    return Ok(arr.iter().filter_map(toml_byte).collect());
                }
            }
        }
        Err(SemAsmError::UnknownMnemonic(mnemonic.to_string()))
    }
}

/// Un entero TOML (0..=255) como byte.
fn toml_byte(v: &toml::Value) -> Option<u8> {
    v.as_integer().and_then(|i| u8::try_from(i).ok())
}

/// Un intrínseco: bytes exactos + convención de retorno.
/// La fusión lenguaje↔ASM: los frontends los exponen como `__nombre()`.
pub struct IntrinsicDef {
    pub bytes: Vec<u8>,
    /// El resultado sale en edx:eax (rdtsc y familia) — el codegen lo
    /// combina a rax con `shl rdx,32; or rax,rdx`.
    pub returns_edx_eax: bool,
}

/// Tabla de intrínsecos cargada desde `arch/<isa>/intrinsics.toml`.
pub struct Intrinsics {
    map: std::collections::HashMap<String, IntrinsicDef>,
}

impl Intrinsics {
    /// Carga `tables/arch/x86_64/intrinsics.toml`.
    pub fn load_x86_64() -> Result<Self, SemAsmError> {
        let path = tables_dir().join("arch/x86_64/intrinsics.toml");
        let text = std::fs::read_to_string(&path)
            .map_err(|_| SemAsmError::TablesNotFound(path))?;
        let root: toml::Table = text.parse().map_err(|e| SemAsmError::Parse(format!("{e}")))?;
        let mut map = std::collections::HashMap::new();
        for (name, entry) in &root {
            // entradas sin `bytes` (p.ej. [meta]) no son intrínsecos
            let Some(arr) = entry.get("bytes").and_then(|v| v.as_array()) else { continue };
            let bytes: Vec<u8> = arr.iter().filter_map(toml_byte).collect();
            if bytes.is_empty() { continue; }
            let returns_edx_eax =
                entry.get("returns").and_then(|v| v.as_str()) == Some("u64_edx_eax");
            map.insert(name.clone(), IntrinsicDef { bytes, returns_edx_eax });
        }
        Ok(Self { map })
    }

    pub fn get(&self, name: &str) -> Option<&IntrinsicDef> {
        self.map.get(name)
    }

    pub fn len(&self) -> usize { self.map.len() }
    pub fn is_empty(&self) -> bool { self.map.is_empty() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_are_present() {
        assert!(tables_dir().join("arch/x86_64/instructions.toml").exists());
    }

    #[test]
    fn loads_real_x86_64_table() {
        let isa = Instructions::load_x86_64().expect("carga instructions.toml");
        assert_eq!(isa.isa_name(), Some("x86-64"));
    }

    #[test]
    fn loads_intrinsics_table() {
        let intr = Intrinsics::load_x86_64().expect("carga intrinsics.toml");
        assert!(!intr.is_empty());
        assert_eq!(intr.get("hlt").unwrap().bytes, vec![0xF4]);
        assert_eq!(intr.get("pause").unwrap().bytes, vec![0xF3, 0x90]);
        let rdtsc = intr.get("rdtsc").unwrap();
        assert_eq!(rdtsc.bytes, vec![0x0F, 0x31]);
        assert!(rdtsc.returns_edx_eax, "rdtsc devuelve edx:eax");
        assert!(intr.get("meta").is_none(), "[meta] no es un intrínseco");
    }

    #[test]
    fn encodes_known_opcodes_from_the_table() {
        let isa = Instructions::load_x86_64().unwrap();
        // mov r/m, r → 0x89 (primera forma de [mov]).
        assert_eq!(isa.opcode("mov").unwrap(), vec![0x89]);
        // mov_imm → opcode_base 0xB8.
        assert_eq!(isa.opcode("mov_imm").unwrap(), vec![0xB8]);
        // El motor ya LEE la tabla: sem-asm dejó de ser TOML muerto.
    }
}
