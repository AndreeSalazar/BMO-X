//! `bmo_abi::error_code` — Códigos de error extendidos.
//!
//! Los **tipos** `BmoError` y `BmoStatus` viven en
//! `crate::bmo_abi::fundamentals::status`. Este módulo solo agrega
//! los **códigos extendidos** y las utilidades de propagación.
//!
//! ## Modelo
//!
//! ```text
//! bits  0..15  = code (BmoErrorCode)
//! bits 16..23  = severity (BmoErrorSeverity)
//! bits 24..31  = flags (recoverable, transient, ...)
//! ```
//!
//! Los syscalls retornan `BmoStatus` (32 bits). El código de error
//! es siempre no-cero si la syscall falló. Cero = OK.

#![allow(dead_code)]

// ─── Codes ──────────────────────────────────────────────────────────

/// Códigos de error canónicos.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BmoErrorCode {
    /// Sin error.
    Ok              = 0,
    /// Sin memoria (heap exhausted, swap full, etc).
    OutOfMemory     = 1,
    /// Handle inválido o de tipo incorrecto.
    InvalidHandle   = 2,
    /// Permiso denegado.
    PermissionDenied = 3,
    /// Recurso no encontrado.
    NotFound        = 4,
    /// Recurso ocupado (lock contention, file in use, etc).
    Busy            = 5,
    /// Timeout alcanzado.
    Timeout         = 6,
    /// Argumento inválido (NULL, out of range, etc).
    InvalidArgument = 7,
    /// Error de I/O (disk, network, etc).
    Io              = 8,
    /// Error interno del kernel. Bug probable.
    Internal        = 9,
    /// Operación no soportada en este OS/config.
    Unsupported     = 10,
    /// Operación cancelada.
    Cancelled       = 11,
    /// Deadlock detectado.
    Deadlock        = 12,
    /// Recurso temporalmente no disponible, reintentar.
    Again           = 13,
    /// Buffer demasiado pequeño.
    BufferTooSmall  = 14,
    /// Estado inconsistente.
    InvalidState    = 15,
    /// Checksum o CRC no coincide.
    Checksum        = 16,
    /// Versión incompatible.
    Version         = 17,
    /// Path no encontrado o malformado.
    PathNotFound    = 18,
    /// Ya existe (file create, port name, etc).
    AlreadyExists   = 19,
    /// Fin de archivo/directorio.
    EndOfStream     = 20,
}

impl BmoErrorCode {
    /// `true` si el código indica éxito.
    #[inline]
    pub fn is_ok(self) -> bool { self == Self::Ok }

    /// `true` si el código indica un error que vale la pena reintentar.
    #[inline]
    pub fn is_transient(self) -> bool {
        matches!(self, Self::Busy | Self::Timeout | Self::Again)
    }

    /// Descripción humana corta.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::OutOfMemory => "out of memory",
            Self::InvalidHandle => "invalid handle",
            Self::PermissionDenied => "permission denied",
            Self::NotFound => "not found",
            Self::Busy => "busy",
            Self::Timeout => "timeout",
            Self::InvalidArgument => "invalid argument",
            Self::Io => "i/o error",
            Self::Internal => "internal error",
            Self::Unsupported => "unsupported",
            Self::Cancelled => "cancelled",
            Self::Deadlock => "deadlock",
            Self::Again => "try again",
            Self::BufferTooSmall => "buffer too small",
            Self::InvalidState => "invalid state",
            Self::Checksum => "checksum mismatch",
            Self::Version => "version mismatch",
            Self::PathNotFound => "path not found",
            Self::AlreadyExists => "already exists",
            Self::EndOfStream => "end of stream",
        }
    }
}

// ─── Severity ──────────────────────────────────────────────────────

/// Severidad de un error. Ocupa los bits 16..23.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BmoErrorSeverity {
    /// No es un error (status code = 0).
    None      = 0,
    /// Advertencia, el programa puede continuar.
    Warning   = 1,
    /// Error, la operación falló pero el programa sigue vivo.
    Error     = 2,
    /// Error fatal, el programa debería terminar.
    Fatal     = 3,
}

impl BmoErrorSeverity {
    pub const MASK: u32 = 0x00FF_0000;
    pub const SHIFT: u32 = 16;
}

// ─── Flags ──────────────────────────────────────────────────────────

/// Flags extras de un error. Bits 24..31.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BmoErrorFlags(pub u32);

impl BmoErrorFlags {
    pub const NONE:         Self = Self(0);
    /// El error es recuperable: reintentar puede funcionar.
    pub const RECOVERABLE:  Self = Self(1 << 0);
    /// El error es transitorio: desaparecerá solo.
    pub const TRANSIENT:    Self = Self(1 << 1);
    /// El error es de usuario (input inválido, etc).
    pub const USER:         Self = Self(1 << 2);
    /// El error es interno (bug).
    pub const INTERNAL:     Self = Self(1 << 3);

    pub const MASK: u32 = 0xFF00_0000;
    pub const SHIFT: u32 = 24;
}

// ─── Raw code constants ─────────────────────────────────────────────

pub const OK:                u32 = 0;
pub const OUT_OF_MEMORY:     u32 = 1;
pub const INVALID_HANDLE:    u32 = 2;
pub const PERMISSION_DENIED: u32 = 3;
pub const NOT_FOUND:         u32 = 4;
pub const BUSY:              u32 = 5;
pub const TIMEOUT:           u32 = 6;
pub const INVALID_ARGUMENT:  u32 = 7;
pub const IO:                u32 = 8;
pub const INTERNAL:          u32 = 9;
pub const UNSUPPORTED:       u32 = 10;
pub const CANCELLED:         u32 = 11;
pub const DEADLOCK:          u32 = 12;
pub const AGAIN:             u32 = 13;
pub const BUFFER_TOO_SMALL:  u32 = 14;
pub const INVALID_STATE:     u32 = 15;
pub const CHECKSUM:          u32 = 16;
pub const VERSION:           u32 = 17;
pub const PATH_NOT_FOUND:    u32 = 18;
pub const ALREADY_EXISTS:    u32 = 19;
pub const END_OF_STREAM:     u32 = 20;

