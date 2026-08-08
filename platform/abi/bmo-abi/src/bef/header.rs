//! Header BEF -- 48 bytes fijos al inicio del archivo.
//!
//! Mucho mas compacto que ELF (64 B + Phdr) o PE (264 B DOS stub + IMAGE_NT).

#![allow(dead_code)]

use crate::bmo_abi::primitives::{bx_u16, bx_u32, bx_u64, bx_u8};
use crate::{supports_abi, BMO_ABI_VERSION};

/// Magic constant -- `b"BEF1"` (4 bytes LE).
pub const BEF_MAGIC: bx_u32 = u32::from_le_bytes(*b"BEF1");

/// Version actual del formato.
pub const BEF_VERSION_MAJOR: bx_u16 = 1;
pub const BEF_VERSION_MINOR: bx_u16 = 0;

/// Magic alternativo aceptado por el loader (para devour).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BefMagic {
    /// `b"BEF1"` -- formato nativo.
    BefNative,
    /// `b"MZ"` -- PE/COFF de Windows (debe procesarse por `loader::pe`).
    PeWindows,
    /// `0x7F b"ELF"` -- ELF de Linux/Unix (procesar por `loader::elf`).
    ElfUnix,
    /// Formato desconocido.
    Unknown,
}

impl BefMagic {
    /// Detecta el formato a partir de los primeros bytes.
    pub fn detect(bytes: &[u8]) -> Self {
        if bytes.len() < 4 {
            return Self::Unknown;
        }
        match &bytes[..4] {
            b"BEF1" => Self::BefNative,
            b"\x7FELF" => Self::ElfUnix,
            _ if &bytes[..2] == b"MZ" => Self::PeWindows,
            _ => Self::Unknown,
        }
    }
}

bitflags::bitflags! {
    /// Flags del header BEF.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct BefFlags: bx_u32 {
        /// El binario es ejecutable (vs. libreria compartida).
        const EXECUTABLE         = 1 << 0;
        /// Es una libreria compartida (.bef.so equivalent).
        const SHARED_LIBRARY     = 1 << 1;
        /// Tiene seccion de manifest TOML valida.
        const HAS_MANIFEST       = 1 << 2;
        /// Tiene seccion de shaders/IR pre-compilados.
        const HAS_SHADERS        = 1 << 3;
        /// Tiene secciones comprimidas con GDeflate.
        const COMPRESSED         = 1 << 4;
        /// Esta firmado con Ed25519.
        const SIGNED             = 1 << 5;
        /// Posee TLS (Thread Local Storage).
        const HAS_TLS            = 1 << 6;
        /// Soporta hot-reload de secciones en runtime.
        const HOT_RELOADABLE     = 1 << 7;
        /// Marca PIE (Position Independent Executable) -- siempre verdadero
        /// para BEF nativo, pero util para PE/ELF devorados.
        const PIE                = 1 << 8;
        /// Importa la API BareX (vs solo usar syscalls BMO).
        const USES_BAREX         = 1 << 9;
        /// * **El binario QUIERE LA PANTALLA.** Lo pone el compilador, no el autor.
        ///
        /// # Por que existe
        ///
        /// Hasta el 2026-08-07 la pantalla era del primero que la pedia -- orden
        /// de llegada, no autoridad. Y `gui.bex` la reclama al arrancar, asi que
        /// cualquier programa grafico se llevaba un "ya tiene dueno".
        ///
        /// El arreglo provisional fue un verbo en el escritorio (`presta
        /// <ruta>`), y era el diseno equivocado: ponia la POLITICA en los dedos
        /// del usuario. Con esta bandera, el compositor **lee la cabecera antes
        /// de lanzar** y decide el -- a quien, cuando, con que prioridad, o que
        /// no. El kernel sigue dando solo el mecanismo (un dueno, `soltar`).
        ///
        /// Tres capas: el kernel arbitra, el BEF declara, el compositor manda.
        /// Es la misma separacion que un planificador de GPU.
        ///
        /// # Por que la pone el COMPILADOR y no el autor
        ///
        /// Porque asi **no puede mentir**: dice lo que el programa HACE, no lo
        /// que promete. El codegen la pone al ver una llamada a la puerta de
        /// syscalls con `BMO_OP_PANTALLA_RECLAMAR` (`0x09`) como operacion.
        ///
        /// [!] Limite honesto: si un programa calculara ese numero en tiempo de
        /// ejecucion, la deteccion no lo ve. Todo el codigo real usa el
        /// `#define`, que expande a un literal -- pero el hueco existe y queda
        /// escrito en vez de descubierto.
        const WANTS_SCREEN       = 1 << 10;
        /// Origen: PE devorado (set por el loader, no por el compilador).
        const PROVENANCE_PE      = 1 << 14;
        /// Origen: ELF devorado.
        const PROVENANCE_ELF     = 1 << 15;
    }
}

/// Arquitectura target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BefArch {
    /// Reservado / desconocido.
    Reserved = 0x00,
    /// AMD64 / x86-64 (baseline BMO).
    X86_64 = 0x01,
    /// AArch64 (futuro, no implementado).
    Aarch64 = 0x02,
    /// RISC-V 64-bit (futuro).
    Rv64gc = 0x03,
}

/// Orden de bytes de la imagen.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BefEndian {
    Little = 0x00,
    Big = 0x01,
}

/// Extensiones de CPU que una imagen puede DECLARAR que usa.
///
/// El programa lo sabe porque su compilador lo sabe: BMO C y BMO COBOL son
/// tuyos y sabe cada uno si emitio una instruccion ancha. Declararlo es un
/// contrato verificable; adivinarlo en tiempo de ejecucion no lo es.
pub struct BefCpuFeature;
impl BefCpuFeature {
    /// Vectores de 256 bits (AVX/AVX2 en x86-64, SVE en aarch64). Obliga al
    /// kernel a guardar el estado ancho en cada cambio de contexto.
    pub const WIDE_VECTORS: u16 = 1 << 0;
    /// Vectores de 512 bits (AVX-512). Area de estado aun mayor.
    pub const WIDER_VECTORS: u16 = 1 << 1;
    /// Bits definidos hoy. Cualquier bit fuera de esta mascara se RECHAZA.
    pub const KNOWN: u16 = Self::WIDE_VECTORS | Self::WIDER_VECTORS;
}

/// Header BEF -- 48 bytes, alineado a 16.
///
/// Campos pequenos primero (BMO ABI struct layout).
#[repr(C, align(16))]
#[derive(Debug, Clone, Copy)]
pub struct BefHeader {
    /// `BEF_MAGIC` = `BEF1` LE.
    pub magic: bx_u32,
    /// Version del formato.
    pub version_major: bx_u16,
    pub version_minor: bx_u16,
    /// Flags `BefFlags`.
    pub flags: bx_u32,
    /// Arquitectura `BefArch as u8`.
    pub arch: bx_u8,
    /// **Orden de bytes** de la imagen: 0 = little-endian, 1 = big-endian.
    ///
    /// Reservado y CONGELADO aqui aunque hoy solo exista little. El dia que
    /// haya un objetivo big-endian (PowerPC, un RISC-V configurado asi), este
    /// byte es la diferencia entre anadir una comprobacion y reescribir todos
    /// los parsers del sistema. Cuesta cero ahora.
    pub endianness: bx_u8,
    /// **Extensiones de CPU que la imagen USA** (mapa de bits, ver
    /// `BefCpuFeature`). El kernel lo necesita para dimensionar el area de
    /// estado que guarda en cada cambio de contexto: hoy `FXSAVE` preserva
    /// x87 y SSE, pero NO la mitad alta de los YMM, asi que un programa que
    /// use AVX sin declararlo se corrompe en silencio.
    ///
    /// Un bit desconocido se RECHAZA, al reves que una seccion desconocida:
    /// una seccion que no entiendo es data inerte, pero una extension de CPU
    /// que no entiendo es estado que no voy a saber preservar.
    pub cpu_features: bx_u16,
    /// Version del BMO ABI esperado (major, minor).
    pub abi_version_major: bx_u8,
    pub abi_version_minor: bx_u8,
    /// Reservado. DEBE ser cero: un productor que escriba basura aqui se
    /// encontrara con que el campo significa algo en una version futura.
    pub _reserved: [bx_u8; 6],
    /// Offset del entry point dentro de la seccion `.code`.
    pub entry_offset: bx_u64,
    /// Offset absoluto del section table.
    pub section_table_offset: bx_u64,
    /// Cantidad de secciones.
    pub section_count: bx_u32,
    /// Tamano total del archivo en bytes (validacion rapida).
    pub total_size: bx_u32,
}

impl BefHeader {
    pub const SIZE: usize = 48;
    const _SIZE_ASSERT: () = assert!(core::mem::size_of::<BefHeader>() == 48);

    /// Construye el header default para un ejecutable BEF v1.0 nativo.
    pub const fn new_executable() -> Self {
        Self {
            magic: BEF_MAGIC,
            version_major: BEF_VERSION_MAJOR,
            version_minor: BEF_VERSION_MINOR,
            flags: BefFlags::EXECUTABLE.bits() | BefFlags::PIE.bits() | BefFlags::USES_BAREX.bits(),
            arch: BefArch::X86_64 as u8,
            endianness: BefEndian::Little as u8,
            cpu_features: 0,
            abi_version_major: BMO_ABI_VERSION.0,
            abi_version_minor: BMO_ABI_VERSION.1,
            _reserved: [0; 6],
            entry_offset: 0,
            section_table_offset: 0,
            section_count: 0,
            total_size: 0,
        }
    }

    pub fn is_valid(&self) -> bool {
        self.magic == BEF_MAGIC
            && self.version_major == BEF_VERSION_MAJOR
            && supports_abi((self.abi_version_major, self.abi_version_minor))
            && self.section_count > 0
            && self.section_count < 256
    }
}
