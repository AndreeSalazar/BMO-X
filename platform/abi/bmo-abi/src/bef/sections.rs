//! Tabla de secciones BEF.
//!
//! 10 tipos de seccion, cada una con proposito claro. Comparado con ELF
//! (~20 tipos) y PE (~11 tipos) -- mas limitado pero mas limpio: cada
//! seccion tiene una semantica unica y obligatoria, no es solo "datos".

#![allow(dead_code)]

use crate::bmo_abi::primitives::{bx_u16, bx_u32, bx_u64, bx_u8};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SectionKind {
    /// Codigo ejecutable x86-64. Cargada como RX.
    Code = 0x01,
    /// Datos inicializados de solo lectura (rodata). Cargada como R.
    RoData = 0x02,
    /// Datos inicializados mutables. Cargada como RW.
    Data = 0x03,
    /// Datos no inicializados (BSS). Reservado en RAM, no en archivo.
    Bss = 0x04,
    /// Tabla de imports (lazy/eager binding via BMO).
    Imports = 0x05,
    /// Tabla de exports (simbolos visibles desde fuera).
    Exports = 0x06,
    /// Tabla de relocations.
    Relocs = 0x07,
    /// Tabla de simbolos (debug + dynamic linking).
    Symbols = 0x08,
    /// Manifest TOML (capabilities, version, dependencies).
    Manifest = 0x09,
    /// Shaders/IR pre-compilados.
    Shaders = 0x0A,
    /// Recursos arbitrarios (texturas BC7, audio Opus, fonts, etc.).
    Resources = 0x0B,
    /// Layout de TLS (Thread Local Storage).
    Tls = 0x0C,
    /// Stack unwind info para excepciones / backtraces.
    Unwind = 0x0D,
    /// Debug info (DWARF-lite especifico de BEF).
    Debug = 0x0E,
    /// Hashes BLAKE3 + firma Ed25519 opcional.
    Signature = 0x0F,

    // --- Sesion 8: secciones de metadatos genericos multi-lenguaje ----
    /// Tabla de `TypeDescriptor` (consumida por `bmo_gpu::abi::type_system::TypeRegistry`).
    TypeMap = 0x10,
    /// VTables `BmoVTable` empacadas (`bmo_gpu::abi::vtable`).
    VTables = 0x11,
    /// Bridges de lenguaje origen (`bmo_gpu::abi::lang_bridge::LangDescriptor`).
    LangBridge = 0x12,
    /// Datos de reflection (mirrors, nombres mangled extra).
    Reflect = 0x13,
    /// Tabla de cierres `BmoClosure` con `ClosureSig`.
    Closures = 0x14,

    // --- 2026-08-10: lo que el programa REQUIERE, y el porque ---------
    /// **Requisitos declarados**: tabla binaria de lo que el programa necesita
    /// para arrancar, cada renglon con su motivo. Ver `requisitos.rs`.
    ///
    /// Existe porque hasta hoy quien deducia eso era el kernel
    /// (`bex::necesita`), y una deduccion en Ring 0 es un cerebro donde
    /// tendria que haber un contrato. `Manifest = 0x09` sigue siendo el TOML
    /// para humanos; esta es su version compilada, que se lee sin parser.
    Requisitos = 0x15,
}

impl SectionKind {
    pub fn from_u8(v: bx_u8) -> Option<Self> {
        match v {
            0x01 => Some(Self::Code),
            0x02 => Some(Self::RoData),
            0x03 => Some(Self::Data),
            0x04 => Some(Self::Bss),
            0x05 => Some(Self::Imports),
            0x06 => Some(Self::Exports),
            0x07 => Some(Self::Relocs),
            0x08 => Some(Self::Symbols),
            0x09 => Some(Self::Manifest),
            0x0A => Some(Self::Shaders),
            0x0B => Some(Self::Resources),
            0x0C => Some(Self::Tls),
            0x0D => Some(Self::Unwind),
            0x0E => Some(Self::Debug),
            0x0F => Some(Self::Signature),
            0x10 => Some(Self::TypeMap),
            0x11 => Some(Self::VTables),
            0x12 => Some(Self::LangBridge),
            0x13 => Some(Self::Reflect),
            0x14 => Some(Self::Closures),
            0x15 => Some(Self::Requisitos),
            _ => None,
        }
    }
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct SectionFlags: bx_u32 {
        /// Mapear como readable.
        const READ          = 1 << 0;
        /// Mapear como writable.
        const WRITE         = 1 << 1;
        /// Mapear como executable.
        const EXEC          = 1 << 2;
        /// Seccion comprimida con GDeflate (descomprimir al cargar).
        const COMPRESSED    = 1 << 3;
        /// Seccion requiere alineacion a pagina (4 KB).
        const PAGE_ALIGNED  = 1 << 4;
        /// Seccion requiere alineacion a huge page (2 MB).
        const HUGE_ALIGNED  = 1 << 5;
        /// Lazy: no cargar hasta que se use (file-backed).
        const LAZY          = 1 << 6;
        /// Seccion verificada por hash al cargar.
        const HASHED        = 1 << 7;
        /// Seccion sintetizada por el devour-loader (no estaba en el archivo).
        const SYNTHETIC     = 1 << 8;
    }
}

/// Una entrada del section table -- 48 bytes.
#[repr(C, align(8))]
#[derive(Debug, Clone, Copy)]
pub struct SectionEntry {
    /// `SectionKind as u8`.
    pub kind: bx_u8,
    /// Padding.
    pub _pad: [bx_u8; 3],
    /// `SectionFlags`.
    pub flags: bx_u32,
    /// Offset dentro del archivo (0 si es BSS o sintetica).
    pub file_offset: bx_u64,
    /// Tamano en archivo.
    pub file_size: bx_u64,
    /// Tamano en memoria (>= file_size; diferencia es zero-fill).
    pub mem_size: bx_u64,
    /// Direccion virtual deseada (0 = elige el loader).
    pub virt_addr: bx_u64,
    /// Alineacion requerida (potencia de 2, default 8).
    pub alignment: bx_u16,
    /// Indice de la seccion Signature que contiene su hash, o 0xFFFF si no aplica.
    pub hash_index: bx_u16,
    /// Reservado.
    pub _reserved: bx_u32,
}
const _: () = assert!(core::mem::size_of::<SectionEntry>() == 48);

impl SectionEntry {
    pub const SIZE: usize = 48;

    pub const ZERO: Self = Self {
        kind: 0,
        _pad: [0; 3],
        flags: 0,
        file_offset: 0,
        file_size: 0,
        mem_size: 0,
        virt_addr: 0,
        alignment: 8,
        hash_index: 0xFFFF,
        _reserved: 0,
    };

    pub fn kind(&self) -> Option<SectionKind> {
        SectionKind::from_u8(self.kind)
    }
}

/// Vista in-memory del section table. Cero-copy sobre los bytes del archivo.
pub struct SectionTable<'a> {
    pub entries: &'a [SectionEntry],
}

impl<'a> SectionTable<'a> {
    pub fn parse(bytes: &'a [u8], offset: u64, count: u32) -> Result<Self, &'static str> {
        let off = offset as usize;
        let needed = count as usize * SectionEntry::SIZE;
        if off + needed > bytes.len() {
            return Err("section table fuera de rango");
        }
        let raw_ptr = unsafe { bytes.as_ptr().add(off) };
        if (raw_ptr as usize) % core::mem::align_of::<SectionEntry>() != 0 {
            return Err("section table pointer mal alineado");
        }
        let ptr = raw_ptr as *const SectionEntry;
        let entries = unsafe { core::slice::from_raw_parts(ptr, count as usize) };
        Ok(Self { entries })
    }

    pub fn find(&self, kind: SectionKind) -> Option<&SectionEntry> {
        self.entries.iter().find(|e| e.kind == kind as u8)
    }
}

// ============================================================================
// LAS SECCIONES DE TABLA + CADENAS
// ============================================================================

/// **Cabecera de las secciones que llevan entradas de tamano fijo seguidas de
/// un blob de cadenas**: `Imports`, `Exports` y `Symbols`.
///
/// # El fallo que obliga a que esto exista (2026-08-14)
///
/// Las tres secciones se escribian asi:
///
/// ```text
///   [entradas][cadenas]        y nada que diga CUANTAS entradas hay
/// ```
///
/// Y las tres se leian asi, en `validator.rs`:
///
/// ```text
///   count        = data.len() / tamano_de_entrada     <- TODO son entradas
///   string_start = count * tamano_de_entrada          <- ...luego no caben
/// ```
///
/// ** Ese modelo es IMPOSIBLE: si todo el dato son entradas, el blob de cadenas
/// empieza donde acaba la seccion y mide cero. Solo cuadra cuando no hay
/// cadenas -- o sea, cuando la seccion no sirve para nada.
///
/// Escritor y lector se escribieron por separado, ninguno de los dos se uso
/// nunca, y por eso llevaban desde el diseno de BEF sin coincidir. Se descubrio
/// al escribir el PRIMER productor de verdad (la tabla de simbolos de BMO C):
/// `bmo-verify` rechazo el binario diciendo `name_off 0x616d7573 out of range`
/// -- y `0x616d7573` es la palabra `"suma"`, o sea el validador leyendo las
/// cadenas como si fueran entradas.
///
/// La leccion, que es la del dia: **un formato con escritor y sin lector (o al
/// reves) no esta definido, esta escrito.** Lo que lo define es que alguien lo
/// recorra de punta a punta.
///
/// # La disposicion, ahora dicha
///
/// ```text
///   [TablaCadenas][entrada; count][cadenas]
///                  ^^^^^^^^^^^^^^  `name_off` es relativo a AQUI
/// ```
#[repr(C, align(8))]
#[derive(Debug, Clone, Copy)]
pub struct TablaCadenas {
    /// Cuantas entradas de tamano fijo vienen detras.
    pub count: bx_u32,
    /// Reservado. **Debe ser cero** -- misma regla que el resto del formato:
    /// un campo futuro no puede heredar basura de un productor de hoy.
    pub _reserved: bx_u32,
}
const _: () = assert!(core::mem::size_of::<TablaCadenas>() == 8);

impl TablaCadenas {
    pub const SIZE: usize = 8;

    pub const fn de(count: u32) -> Self {
        Self { count, _reserved: 0 }
    }

    /// Lee la cabecera y devuelve `(count, donde_empiezan_las_cadenas)`.
    ///
    /// `None` si la seccion no da ni para la cabecera, o si el numero de
    /// entradas que declara no cabe en lo que mide. Un `count` inventado haria
    /// que el lector recorriera cadenas creyendo que son entradas -- que es
    /// exactamente el fallo que esta cabecera viene a cerrar.
    pub fn leer(data: &[u8], tamano_de_entrada: usize) -> Option<(usize, usize)> {
        if data.len() < Self::SIZE {
            return None;
        }
        let count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
        let fin = Self::SIZE.checked_add(count.checked_mul(tamano_de_entrada)?)?;
        if fin > data.len() {
            return None;
        }
        Some((count, fin))
    }
}
