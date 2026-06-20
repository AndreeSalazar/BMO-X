//! `ErrorCode` — códigos numéricos canónicos del BMO ABI.
//!
//! Mapean 1-a-1 con `crate::bmo_gpu::BxError`. Esta tabla es la fuente de
//! verdad para FFI con apps que no usen el enum Rust.

use crate::bmo_core::bmo_abi::primitives::bx_u32;

pub mod error_code {
    use super::bx_u32;

    pub const OK:                bx_u32 = 0;
    pub const OUT_OF_MEMORY:     bx_u32 = 1;
    pub const INVALID_ARGUMENT:  bx_u32 = 2;
    pub const NOT_INITIALIZED:   bx_u32 = 3;
    pub const DEVICE_LOST:       bx_u32 = 4;
    pub const NOT_IMPLEMENTED:   bx_u32 = 5;
    pub const UNSUPPORTED:       bx_u32 = 6;
    pub const TIMEOUT:           bx_u32 = 7;
    pub const IO_ERROR:          bx_u32 = 8;
    pub const PERMISSION_DENIED: bx_u32 = 9;
    pub const ALREADY_EXISTS:    bx_u32 = 10;
    pub const NOT_FOUND:         bx_u32 = 11;
    pub const BAD_HANDLE:        bx_u32 = 12;
    pub const BUFFER_TOO_SMALL:  bx_u32 = 13;
    pub const WOULD_BLOCK:       bx_u32 = 14;
    pub const CANCELLED:         bx_u32 = 15;
    pub const CONNECTION_RESET:  bx_u32 = 16;
    pub const CONNECTION_REFUSED: bx_u32 = 17;
    pub const ADDR_IN_USE:       bx_u32 = 18;

    /// Devuelve un texto humano para depuración. Cero asignación.
    pub const fn message(code: bx_u32) -> &'static str {
        match code {
            OK => "ok",
            OUT_OF_MEMORY => "out of memory",
            INVALID_ARGUMENT => "invalid argument",
            NOT_INITIALIZED => "not initialized",
            DEVICE_LOST => "device lost",
            NOT_IMPLEMENTED => "not implemented",
            UNSUPPORTED => "unsupported",
            TIMEOUT => "timeout",
            IO_ERROR => "io error",
            PERMISSION_DENIED => "permission denied",
            ALREADY_EXISTS => "already exists",
            NOT_FOUND => "not found",
            BAD_HANDLE => "bad handle",
            BUFFER_TOO_SMALL => "buffer too small",
            WOULD_BLOCK => "would block",
            CANCELLED => "cancelled",
            CONNECTION_RESET => "connection reset",
            CONNECTION_REFUSED => "connection refused",
            ADDR_IN_USE => "address in use",
            _ => "unknown error",
        }
    }
}
