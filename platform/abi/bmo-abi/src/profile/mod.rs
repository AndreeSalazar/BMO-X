//! `bmo_abi::profile` -- Perfiles de lenguaje.
//!
//! Cada frontend (C, COBOL, BMO, Java-BMO, Python-BMO, ...) implementa un
//! `BmoLanguageProfile` que describe **como** se compila el codigo
//! fuente a BEF.
//!
//! ## Modelo
//!
//! ```text
//!  +-------------------------------------------------------------+
//!  | BmoLanguageProfile                                          |
//!  |   name:        &'static str                                 |
//!  |   frontend:    FrontendKind (C | COBOL | BMO | JavaBMO)     |
//!  |   backend:     BackendKind  (AotX86_64 | PortableIR)        |
//!  |   runtime:     RuntimeKind  (None | CMin | JavaCore | ...)  |
//!  |   output:      BEF                                          |
//!  +-------------------------------------------------------------+
//! ```
//!
//! El kernel **no compila** nada. Solo provee:
//! - el BEF loader (`crate::bmo_core::bef::loader`)
//! - los runtimes opcionales (`crate::bmo_abi::profile::RuntimeKind`)
//! - los syscalls (`crate::bmo_abi::syscalls`)
//!
//! Compilar es **offline** (el dev lo hace en su maquina). El kernel
//! solo ejecuta.

#![allow(dead_code)]

// --- Frontend / backend / runtime kinds ---------------------------

/// Frontend soportado.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrontendKind {
    /// Lenguaje BMO nativo.
    Bmo = 0,
    /// C estandar.
    C = 1,
    /// C++ (subset + runtime).
    Cpp = 2,
    /// Rust (subset + runtime).
    Rust = 3,
    /// Java-BMO (no JVM completo).
    JavaBmo = 4,
    /// Python-BMO (typed + dynamic).
    PythonBmo = 5,
    /// Ada.
    Ada = 6,
    /// COBOL clasico/empresarial compilado AOT a BEF.
    Cobol = 7,
    /// Lenguaje custom / third-party.
    Custom = 0xFF,
}

impl FrontendKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Bmo => "bmo",
            Self::C => "c",
            Self::Cpp => "cpp",
            Self::Rust => "rust",
            Self::JavaBmo => "java-bmo",
            Self::PythonBmo => "python-bmo",
            Self::Ada => "ada",
            Self::Cobol => "cobol",
            Self::Custom => "custom",
        }
    }
}

/// Backend de compilacion.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackendKind {
    /// AOT puro x86-64 (default).
    AotX86_64 = 0,
    /// IR portable (cualquier CPU).
    PortableIR = 1,
    /// AOT RDNA4 (GPU shaders, no implementado todavia).
    AotRdna4 = 2,
}

impl BackendKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::AotX86_64 => "aot-x86_64",
            Self::PortableIR => "portable-ir",
            Self::AotRdna4 => "aot-rdna4",
        }
    }
}

/// Runtime requerido por el lenguaje.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeKind {
    /// Sin runtime. AOT puro, binario minusculo.
    None = 0,
    /// Runtime C minimo (`_start`, `memcpy`, syscall wrappers).
    CMin = 1,
    /// Runtime C++ (constructors, vtables).
    CppMin = 2,
    /// Runtime Java-BMO (class model, strings, arrays).
    JavaCore = 3,
    /// Runtime Python-BMO (dicts, types dinamicos).
    PythonCore = 4,
    /// Runtime Rust (panic handler, allocator).
    RustCore = 5,
    /// Runtime COBOL minimo: decimal fijo, records, DISPLAY/ACCEPT y archivos.
    CobolCore = 6,
}

impl RuntimeKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::CMin => "c_min",
            Self::CppMin => "cpp_min",
            Self::JavaCore => "java_core",
            Self::PythonCore => "python_core",
            Self::RustCore => "rust_core",
            Self::CobolCore => "cobol_core",
        }
    }
}

// --- Language profile ---------------------------------------------

/// Perfil de un lenguaje. Define como se compila a BEF.
#[derive(Clone, Copy, Debug)]
pub struct BmoLanguageProfile {
    /// Nombre legible (ej: "C", "BMO", "Java-BMO").
    pub name: &'static str,
    /// Frontend usado.
    pub frontend: FrontendKind,
    /// Backend usado.
    pub backend: BackendKind,
    /// Runtime requerido.
    pub runtime: RuntimeKind,
    /// ABI que se usa para llamadas al sistema.
    pub uses_bmo_abi: bool,
    /// `true` si el lenguaje puede correr en Ring 0 (drivers).
    pub ring0_capable: bool,
    /// Language standard version (e.g. "c11", "cpp17", "cobol85").
    pub standard_version: &'static str,
}

impl BmoLanguageProfile {
    /// Perfil canonico de C: AOT puro, runtime CMin.
    pub const C: Self = Self {
        name: "C",
        frontend: FrontendKind::C,
        backend: BackendKind::AotX86_64,
        runtime: RuntimeKind::CMin,
        uses_bmo_abi: true,
        ring0_capable: true,
        standard_version: "c11",
    };
    pub const BMO: Self = Self {
        name: "BMO",
        frontend: FrontendKind::Bmo,
        backend: BackendKind::AotX86_64,
        runtime: RuntimeKind::None,
        uses_bmo_abi: true,
        ring0_capable: true,
        standard_version: "latest",
    };
    pub const JAVA_BMO: Self = Self {
        name: "Java-BMO",
        frontend: FrontendKind::JavaBmo,
        backend: BackendKind::AotX86_64,
        runtime: RuntimeKind::JavaCore,
        uses_bmo_abi: true,
        ring0_capable: false,
        standard_version: "latest",
    };

    /// Perfil de Python-BMO: AOT typed + PythonCore runtime.
    pub const PYTHON_BMO: Self = Self {
        name: "Python-BMO",
        frontend: FrontendKind::PythonBmo,
        backend: BackendKind::AotX86_64,
        runtime: RuntimeKind::PythonCore,
        uses_bmo_abi: true,
        ring0_capable: false,
        standard_version: "latest",
    };

    /// Perfil de COBOL: AOT CPU + runtime minimo orientado a datos/archivos.
    ///
    /// COBOL no requiere GPU: su valor inicial esta en CPU, registros fijos,
    /// decimal empaquetado, batch y FS sobre syscalls BMO.
    pub const COBOL: Self = Self {
        name: "COBOL",
        frontend: FrontendKind::Cobol,
        backend: BackendKind::AotX86_64,
        runtime: RuntimeKind::CobolCore,
        uses_bmo_abi: true,
        ring0_capable: false,
        standard_version: "cobol85",
    };
}

// --- Profiles predefinidos ----------------------------------------

/// Todos los perfiles predefinidos.
pub const ALL_PROFILES: &[BmoLanguageProfile] = &[
    BmoLanguageProfile::BMO,
    BmoLanguageProfile::C,
    BmoLanguageProfile::COBOL,
    BmoLanguageProfile::JAVA_BMO,
    BmoLanguageProfile::PYTHON_BMO,
];

/// Busca un perfil por nombre.
pub fn find_profile(name: &str) -> Option<&'static BmoLanguageProfile> {
    ALL_PROFILES.iter().find(|p| p.name == name)
}
