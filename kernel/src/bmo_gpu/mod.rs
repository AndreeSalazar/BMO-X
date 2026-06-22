//! BMO GPU — el bridge entre BMO Core (Ring 3) y la GPU real.
//!
//! # Política (v1.7.9)
//!
//! BMO GPU es **separado** de BMO Core. No es un "driver" — es un
//! conjunto de APIs que:
//!
//! 1. **Validan** código que viene del exterior (BSF shaders, PE/ELF
//!    thunks para migración de apps Windows).
//! 2. **Firman** ese código con BLAKE3 antes de pasarlo al driver real
//!    en `ring0/dev/amdgpu.rs` (futuro).
//! 3. **Exponen** una API limpia de errores (`BxError`) y resultados.
//!
//! # Estructura
//!
//! ```text
//! bmo_gpu/
//! ├── mod.rs           — entry point, BAREX_VERSION, BxError
//! ├── shims/           — Compatibilidad con apps externas (PE, ELF)
//! ├── shader/          — BSF (BareX Shader Format) loader
//! ├── compositor/       — Ring 0 ↔ Ring 3 GPU composition
//! └── commands/         — GPU command buffers (ring submission)
//! ```
//!
//! # Relación con BMO Core y Ring 0
//!
//! ```text
//! App Ring 3 (BMO)
//!   │
//!   │  syscall 0x140 GPU_SUBMIT_SHADER
//!   ▼
//! bmo_core/api/syscall.rs   ← dispatch 0x100..0x1FF
//!   │
//!   │  GPU syscall
//!   ▼
//! ring0/dev/amdgpu.rs       ← driver real (futuro v1.8)
//!   │
//!   │  MMIO a la GPU
//!   ▼
//! Ryzen 5 5600X + RX 580/5600XT
//! ```
//!
//! BMO GPU vive **entre** BMO Core y Ring 0. Es la capa que valida y
//! normaliza lo que Ring 0 va a ejecutar.

#![allow(dead_code)]

pub mod shims;
pub mod shader;
pub mod compositor;
pub mod commands;

// ── Versión y plataforma ────────────────────────────────────────────────

/// Versión mayor.menor.patch de la API BMO GPU expuesta a Ring 3.
pub const BAREX_VERSION: (u16, u16, u16) = (1, 0, 0);

/// Identificador de plataforma objetivo funcional: UEFI GOP + CPU x86_64.
pub const HW_TARGET: &str = "UEFI-GOP+x86_64";

/// Magic bytes del BSF (BareX Shader Format).
pub const BSF_MAGIC: [u8; 4] = *b"BSF\0";
/// BSF current version.
pub const BSF_VERSION: u32 = 1;
/// BSF header size.
pub const BSF_HEADER_SIZE: usize = 0x74;

// ── Tipos comunes ────────────────────────────────────────────────────────

/// Arch identifier (qué CPU corre el shader).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum BsfArch {
    X86_64  = 0,
    Aarch64 = 1,
    Riscv64 = 2,
}

/// Shader stage (vertex, fragment, compute).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum BsfStage {
    Vertex   = 0,
    Fragment = 1,
    Compute  = 2,
}

// ── Error type (sustituye a BxError, sin HRESULT) ───────────────────────

/// Resultado canónico para toda la superficie BMO GPU.
///
/// En el BMO ABI viaja como `BmoStatus { code, flags, value }` empacado
/// en `RAX:RDX` (16 bytes), sin tocar memoria.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum BxError {
    OutOfMemory     = 1,
    InvalidArgument = 2,
    NotInitialized  = 3,
    DeviceLost      = 4,
    NotImplemented  = 5,
    Unsupported     = 6,
    Timeout         = 7,
    IoError         = 8,
    PermissionDenied = 9,
    AlreadyExists   = 10,
    NotFound        = 11,
    BadHandle       = 12,
    BufferTooSmall  = 13,
}

pub type BxResult<T> = core::result::Result<T, BxError>;

impl BxError {
    /// Convierte a `BmoStatus` para retorno via BMO ABI (RAX:RDX).
    #[inline(always)]
    pub const fn to_status(self) -> crate::bmo_abi::status::BmoStatus {
        crate::bmo_abi::status::BmoStatus::err(self as u32)
    }
}

/// Inicializa el subsistema BMO GPU. v1.7.9: no-op.
pub fn init() {
    // v1.8: precargar shaders built-in, init compositor.
}
