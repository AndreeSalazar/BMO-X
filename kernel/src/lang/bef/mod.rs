//! `lang::bef` — Tipos comunes del BEF (Binary Executable Format).
//!
//! Este módulo define el **modelo de datos** del BEF:
//! - `BmoObject`: producto intermedio del AOT
//! - `ObjectSection`: secciones del objeto (.text, .rodata, etc.)
//! - `Symbol`: símbolos definidos e importados
//! - `Relocation`: relocaciones a aplicar
//! - `RelocationKind`: tipos de relocalización soportados
//! - `BmoObjectBuilder`: helper para construir objetos incrementalmente
//!
//! El **linker** (`lang::linker::link`) toma uno o más `BmoObject` + el
//! runtime correspondiente y produce un `LinkedBef` (BEF final).
//!
//! ## Por qué un modelo de objetos (no Vec<u8> plano)
//!
//! - Permite **múltiples lenguajes**: cada frontend produce su propio objeto.
//! - Permite **runtime modular**: el linker decide qué runtime incluir.
//! - Permite **ABI validation**: el linker verifica que los imports
//!   ABI son válidos antes de escribir el BEF.
//! - Permite **extensibilidad**: agregar Java-BMO o Python-BMO no requiere
//!   tocar el linker, solo el AOT object que produce.

#![allow(dead_code)]

extern crate alloc;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::bmo_gpu::BxResult;
use crate::bmo_abi::profile::RuntimeKind;
use crate::lang::common::ast::StrId;

// ─── Section ────────────────────────────────────────────────────────

/// Tipos de sección BEF.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SectionKind {
    /// Código ejecutable x86-64 (R|X).
    Text     = 0x01,
    /// Datos de solo lectura (R). Strings, constants.
    Rodata   = 0x02,
    /// Datos inicializados mutables (R|W).
    Data     = 0x03,
    /// Datos no inicializados (R|W, zero-init en load). Reservado.
    Bss      = 0x04,
    /// Tabla de relocalizaciones.
    Reloc    = 0x05,
    /// Tabla de símbolos.
    Symtab   = 0x06,
    /// Strings de símbolos.
    Strtab   = 0x07,
    /// Debug info.
    Debug    = 0x08,
    /// Notas, build ID, etc.
    Note     = 0x09,
    /// Tabla de imports ABI.
    Imports  = 0x0A,
    /// Metadata BMO ABI (versión, capabilities).
    Meta     = 0x0B,
    /// TLS template.
    Tls      = 0x0C,
}

/// Permisos de una sección.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SectionFlags(pub u32);

impl SectionFlags {
    pub const NONE:  Self = Self(0);
    pub const READ:  Self = Self(1 << 0);
    pub const WRITE: Self = Self(1 << 1);
    pub const EXEC:  Self = Self(1 << 2);

    pub const RX:    Self = Self(1 << 0 | 1 << 2);
    pub const RW:    Self = Self(1 << 0 | 1 << 1);
    pub const RWX:   Self = Self(1 << 0 | 1 << 1 | 1 << 2);

    pub fn contains(self, other: Self) -> bool { (self.0 & other.0) == other.0 }
}

/// Una sección de un BmoObject.
#[derive(Clone, Debug)]
pub struct ObjectSection {
    pub kind: SectionKind,
    pub flags: SectionFlags,
    /// Nombre (para debug; no afecta el BEF final).
    pub name: alloc::string::String,
    /// Datos crudos de la sección.
    pub data: Vec<u8>,
    /// Alineación requerida.
    pub align: u32,
    /// Offset de la sección dentro del BEF final (lo llena el linker).
    pub final_offset: u32,
}

impl ObjectSection {
    pub fn new(kind: SectionKind, flags: SectionFlags, name: impl Into<String>) -> Self {
        Self {
            kind,
            flags,
            name: name.into(),
            data: Vec::new(),
            align: 16,
            final_offset: 0,
        }
    }

    pub fn with_data(mut self, data: Vec<u8>) -> Self {
        self.data = data;
        self
    }

    pub fn with_align(mut self, align: u32) -> Self {
        self.align = align;
        self
    }

    pub fn len(&self) -> usize { self.data.len() }
    pub fn is_empty(&self) -> bool { self.data.is_empty() }
}

// ─── Symbol ────────────────────────────────────────────────────────

/// Binding de un símbolo.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SymbolBinding {
    /// Símbolo local (no visible fuera del objeto).
    Local     = 0,
    /// Símbolo global (exportado).
    Global    = 1,
    /// Símbolo débil (puede ser sobreescrito por otro más fuerte).
    Weak      = 2,
}

/// Tipo de un símbolo.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SymbolType {
    /// Símbolo sin tipo (dato).
    None      = 0,
    /// Función.
    Function  = 1,
    /// Variable/object.
    Object    = 2,
    /// Sección.
    Section   = 3,
}

/// Un símbolo en el objeto.
#[derive(Clone, Debug)]
pub struct Symbol {
    pub name: String,
    pub binding: SymbolBinding,
    pub sym_type: SymbolType,
    /// ID de la sección donde está definido (None si es import).
    pub section: Option<usize>,
    /// Offset dentro de la sección.
    pub offset: u32,
    /// Tamaño en bytes (0 si desconocido).
    pub size: u32,
    /// Si es import, el nombre del símbolo del que se importa
    /// (e.g. "bmo_print" para una syscall BMO ABI).
    pub import_name: Option<String>,
}

impl Symbol {
    pub fn is_import(&self) -> bool { self.section.is_none() }
    pub fn is_function(&self) -> bool { self.sym_type == SymbolType::Function }
}

// ─── Relocation ───────────────────────────────────────────────────

/// Tipos de relocalización soportados.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelocationKind {
    /// Rel32: `call rel32` o `jmp rel32`.
    Rel32       = 0,
    /// RIP-relative: `lea rax, [rip+disp32]`.
    RipRel32    = 1,
    /// Abs64: dirección absoluta de 64 bits.
    Abs64       = 2,
    /// GOT64: índice en la GOT (futuro).
    Got64       = 3,
}

/// Una relocalización pendiente.
#[derive(Clone, Debug)]
pub struct Relocation {
    pub kind: RelocationKind,
    /// Sección donde está el patch.
    pub section: usize,
    /// Offset dentro de la sección.
    pub offset: u32,
    /// Símbolo al que se refiere.
    pub symbol: String,
    /// Offset adicional (para RipRel: 0, para Abs64: 0, etc.).
    pub addend: i64,
    /// Tamaño del campo a parchear (4 o 8 bytes).
    pub size: u32,
}

impl Relocation {
    pub fn rel32(section: usize, offset: u32, symbol: impl Into<String>) -> Self {
        Self { kind: RelocationKind::Rel32, section, offset, symbol: symbol.into(), addend: 0, size: 4 }
    }
    pub fn rip_rel32(section: usize, offset: u32, symbol: impl Into<String>, addend: i64) -> Self {
        Self { kind: RelocationKind::RipRel32, section, offset, symbol: symbol.into(), addend, size: 4 }
    }
    pub fn abs64(section: usize, offset: u32, symbol: impl Into<String>, addend: i64) -> Self {
        Self { kind: RelocationKind::Abs64, section, offset, symbol: symbol.into(), addend, size: 8 }
    }
}

// ─── Object ───────────────────────────────────────────────────────

/// Producto intermedio del AOT. Es la entrada al linker.
///
/// Un `BmoObject` representa un módulo compilado: contiene secciones
/// (code, rodata, data), símbolos (definidos + imports), y relocalizaciones
/// pendientes. El linker toma uno o más objetos + el runtime y produce
/// un BEF ejecutable.
#[derive(Clone, Debug)]
pub struct BmoObject {
    /// Nombre del módulo fuente.
    pub name: String,
    /// Arquitectura target.
    pub arch: ObjectArch,
    /// Runtime requerido (decidido por el frontend).
    pub required_runtime: RuntimeKind,
    /// Entry point (símbolo, e.g. "main").
    pub entry_symbol: Option<String>,
    /// Secciones (text, rodata, data, ...).
    pub sections: Vec<ObjectSection>,
    /// Símbolos (definidos + imports).
    pub symbols: Vec<Symbol>,
    /// Relocalizaciones pendientes.
    pub relocations: Vec<Relocation>,
    /// Metadata BMO ABI (versión, capabilities).
    pub abi_version: (u8, u8),
    /// Capabilities del módulo.
    pub capabilities: u32,
}

/// Arquitectura target del objeto.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectArch {
    X86_64   = 1,
    AArch64  = 2,
    RiscV64  = 3,
    Rdna4    = 4,
}

impl BmoObject {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            arch: ObjectArch::X86_64,
            required_runtime: RuntimeKind::None,
            entry_symbol: Some("main".into()),
            sections: Vec::new(),
            symbols: Vec::new(),
            relocations: Vec::new(),
            abi_version: (1, 0),
            capabilities: 0,
        }
    }

    /// Encuentra una sección por kind.
    pub fn section_by_kind(&self, kind: SectionKind) -> Option<usize> {
        self.sections.iter().position(|s| s.kind == kind)
    }

    /// Encuentra o crea una sección.
    pub fn get_or_create_section(&mut self, kind: SectionKind, name: &str) -> usize {
        if let Some(idx) = self.section_by_kind(kind) { return idx; }
        let flags = match kind {
            SectionKind::Text => SectionFlags::RX,
            SectionKind::Rodata => SectionFlags::READ,
            SectionKind::Data | SectionKind::Bss => SectionFlags::RW,
            _ => SectionFlags::READ,
        };
        self.sections.push(ObjectSection::new(kind, flags, name));
        self.sections.len() - 1
    }

    /// Encuentra un símbolo por nombre.
    pub fn symbol_by_name(&self, name: &str) -> Option<usize> {
        self.symbols.iter().position(|s| s.name == name)
    }

    /// Encuentra o crea un símbolo definido.
    pub fn define_symbol(&mut self, name: impl Into<String>, section: usize, offset: u32, sym_type: SymbolType) -> usize {
        let name = name.into();
        if let Some(idx) = self.symbol_by_name(&name) { return idx; }
        self.symbols.push(Symbol {
            name,
            binding: SymbolBinding::Global,
            sym_type,
            section: Some(section),
            offset,
            size: 0,
            import_name: None,
        });
        self.symbols.len() - 1
    }

    /// Encuentra o crea un símbolo importado.
    pub fn import_symbol(&mut self, name: impl Into<String>, import_name: Option<String>) -> usize {
        let name = name.into();
        if let Some(idx) = self.symbol_by_name(&name) { return idx; }
        self.symbols.push(Symbol {
            name,
            binding: SymbolBinding::Global,
            sym_type: SymbolType::Function,
            section: None,
            offset: 0,
            size: 0,
            import_name,
        });
        self.symbols.len() - 1
    }

    /// Agrega una relocalización.
    pub fn add_relocation(&mut self, reloc: Relocation) {
        self.relocations.push(reloc);
    }

    /// Serializa el objeto a bytes (formato interno del linker).
    /// El linker es el único que usa esto.
    pub fn serialize(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(b"BMO_OBJ\0");
        out.push(self.arch as u8);
        out.push(self.required_runtime as u8);
        out.extend_from_slice(&(self.abi_version.0 as u16).to_le_bytes());
        out.extend_from_slice(&(self.abi_version.1 as u16).to_le_bytes());
        out.extend_from_slice(&self.capabilities.to_le_bytes());
        let n_sections = self.sections.len() as u32;
        let n_symbols = self.symbols.len() as u32;
        let n_relocs = self.relocations.len() as u32;
        out.extend_from_slice(&n_sections.to_le_bytes());
        out.extend_from_slice(&n_symbols.to_le_bytes());
        out.extend_from_slice(&n_relocs.to_le_bytes());
        out
    }
}

// ─── Builder ───────────────────────────────────────────────────────

/// Builder helper para crear `BmoObject` incrementalmente (usado por el AOT).
pub struct BmoObjectBuilder {
    /// El objeto en construcción (público para que el codegen pueda
    /// agregar secciones/relocs directamente).
    pub obj: BmoObject,
    /// Cache de StrId → nombre (para resolver los patches del codegen).
    pub str_cache: BTreeMap<u32, String>,
}

impl BmoObjectBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            obj: BmoObject::new(name),
            str_cache: BTreeMap::new(),
        }
    }

    /// Registra un string (StrId → nombre real).
    pub fn register_str(&mut self, id: u32, name: &str) {
        self.str_cache.insert(id, alloc::string::String::from(name));
    }

    /// Crea una sección (devuelve el índice).
    pub fn create_section(&mut self, kind: SectionKind, name: &str) -> usize {
        self.obj.get_or_create_section(kind, name)
    }

    /// Define un símbolo (devuelve el índice).
    pub fn define(&mut self, name: &str, section: usize, offset: u32, sym_type: SymbolType) -> usize {
        self.obj.define_symbol(name, section, offset, sym_type)
    }

    /// Importa un símbolo (devuelve el índice).
    pub fn import(&mut self, name: &str, import_name: Option<&str>) -> usize {
        self.obj.import_symbol(name, import_name.map(|s| alloc::string::String::from(s)))
    }

    /// Agrega una relocalización.
    pub fn add_reloc(&mut self, reloc: Relocation) {
        self.obj.add_relocation(reloc);
    }

    /// Establece el entry point.
    pub fn set_entry(&mut self, name: &str) {
        self.obj.entry_symbol = Some(alloc::string::String::from(name));
    }

    /// Establece el runtime requerido.
    pub fn set_runtime(&mut self, rk: RuntimeKind) {
        self.obj.required_runtime = rk;
    }

    /// Append data a una sección existente.
    pub fn append_to_section(&mut self, idx: usize, data: &[u8]) {
        self.obj.sections[idx].data.extend_from_slice(data);
    }

    /// Construye el objeto.
    pub fn build(self) -> BmoObject {
        self.obj
    }
}
