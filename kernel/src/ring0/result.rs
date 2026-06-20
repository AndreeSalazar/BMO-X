//! Kernel-wide error type.
//!
//! Every kernel API that can fail returns `KResult<T>`. The variants are
//! the minimal set needed to express the failure modes of Ring 0:
//! hardware timeouts, OOM, invalid arguments, conflicts with other drivers.
//!
//! This type is also mapped to the BMO API v2 errno (negative values) so
//! that user-space code can interpret the error without needing a separate
//! `last_error` field.

#![allow(dead_code)]

use core::fmt;

pub type KResult<T> = Result<T, KError>;

/// All kernel errors. Keep this enum small: every variant has to be
/// understood by every driver, and adding a variant is a breaking change
/// for the BMO API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KError {
    /// Out of physical or virtual memory.
    OutOfMemory,
    /// Argument outside the valid range for the operation.
    InvalidArgument,
    /// Operation took too long; hardware did not respond.
    Timeout,
    /// Generic I/O failure (e.g. bus error, device not ready).
    Io,
    /// Feature not implemented yet (e.g. driver does not support this op).
    NotSupported,
    /// Resource already in use (e.g. two drivers want the same IRQ).
    AlreadyInUse,
    /// The handle / device / resource does not exist.
    NotFound,
    /// Try again later (e.g. a mutex is held).
    Again,
    /// The hardware is in a bad state and needs a reset.
    HardwareFault,
    /// Catch-all for unknown failures. Should be rare; prefer a specific
    /// variant when possible.
    Other,
}

impl KError {
    /// Map to BMO API v2 errno. The mapping is stable: user-space can
    /// switch on the integer directly.
    pub fn errno(self) -> i64 {
        match self {
            KError::OutOfMemory     => -12,  // ENOMEM
            KError::InvalidArgument => -22,  // EINVAL
            KError::Timeout         => -110, // ETIMEDOUT
            KError::Io              => -5,   // EIO
            KError::NotSupported    => -95,  // ENOTSUP
            KError::AlreadyInUse    => -16,  // EBUSY
            KError::NotFound        => -2,   // ENOENT
            KError::Again           => -11,  // EAGAIN
            KError::HardwareFault   => -71,  // EPROTO
            KError::Other           => -1,   // EPERM
        }
    }

    /// Short human-readable name. Used by the logger.
    pub fn as_str(self) -> &'static str {
        match self {
            KError::OutOfMemory     => "out of memory",
            KError::InvalidArgument => "invalid argument",
            KError::Timeout         => "timeout",
            KError::Io              => "I/O error",
            KError::NotSupported    => "not supported",
            KError::AlreadyInUse    => "already in use",
            KError::NotFound        => "not found",
            KError::Again           => "try again",
            KError::HardwareFault   => "hardware fault",
            KError::Other           => "other error",
        }
    }
}

impl fmt::Display for KError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Convert a `bool` to `KResult<()>`: `true` -> Ok, `false` -> Io.
#[inline]
pub fn ok_or_io(cond: bool) -> KResult<()> {
    if cond { Ok(()) } else { Err(KError::Io) }
}

/// Convert `Option<T>` to `KResult<T>`: `None` -> NotFound.
#[inline]
pub fn some_or_notfound<T>(opt: Option<T>) -> KResult<T> {
    opt.ok_or(KError::NotFound)
}

/// Map a `u64` return value (where 0 means success) to `KResult<()>`.
#[inline]
pub fn ok_or_errno(rc: u64) -> KResult<()> {
    if rc == 0 { Ok(()) } else { Err(KError::Other) }
}
