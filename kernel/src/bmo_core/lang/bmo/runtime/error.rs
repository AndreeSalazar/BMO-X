//! ÑEXO Runtime — Tipos de error.

#![allow(dead_code)]

/// Error codes for ÑEXO runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Error {
    Ok = 0,
    OutOfMemory = 1,
    InvalidArgument = 2,
    NotFound = 3,
    PermissionDenied = 4,
    AlreadyExists = 5,
    IoError = 6,
    Timeout = 7,
    WouldBlock = 8,
    BadHandle = 9,
    BufferTooSmall = 10,
    NotSupported = 11,
    ProcessLimit = 12,
    ThreadLimit = 13,
    StackOverflow = 14,
    DivisionByZero = 15,
}

/// Result type for ÑEXO runtime operations.
pub type Result<T> = core::result::Result<T, Error>;

impl Error {
    /// Convert to a human-readable message.
    pub fn message(&self) -> &'static str {
        match self {
            Error::Ok => "success",
            Error::OutOfMemory => "out of memory",
            Error::InvalidArgument => "invalid argument",
            Error::NotFound => "not found",
            Error::PermissionDenied => "permission denied",
            Error::AlreadyExists => "already exists",
            Error::IoError => "I/O error",
            Error::Timeout => "timeout",
            Error::WouldBlock => "would block",
            Error::BadHandle => "bad handle",
            Error::BufferTooSmall => "buffer too small",
            Error::NotSupported => "not supported",
            Error::ProcessLimit => "process limit reached",
            Error::ThreadLimit => "thread limit reached",
            Error::StackOverflow => "stack overflow",
            Error::DivisionByZero => "division by zero",
        }
    }

    /// Convert from kernel BxError.
    pub fn from_bx(err: crate::bmo_gpu::BxError) -> Self {
        match err {
            crate::bmo_gpu::BxError::OutOfMemory => Error::OutOfMemory,
            crate::bmo_gpu::BxError::InvalidArgument => Error::InvalidArgument,
            crate::bmo_gpu::BxError::NotFound => Error::NotFound,
            crate::bmo_gpu::BxError::PermissionDenied => Error::PermissionDenied,
            crate::bmo_gpu::BxError::AlreadyExists => Error::AlreadyExists,
            crate::bmo_gpu::BxError::IoError => Error::IoError,
            crate::bmo_gpu::BxError::Timeout => Error::Timeout,
            crate::bmo_gpu::BxError::BadHandle => Error::BadHandle,
            crate::bmo_gpu::BxError::BufferTooSmall => Error::BufferTooSmall,
            crate::bmo_gpu::BxError::NotImplemented | crate::bmo_gpu::BxError::Unsupported => Error::NotSupported,
            crate::bmo_gpu::BxError::DeviceLost => Error::IoError,
            crate::bmo_gpu::BxError::NotInitialized => Error::InvalidArgument,
        }
    }
}
