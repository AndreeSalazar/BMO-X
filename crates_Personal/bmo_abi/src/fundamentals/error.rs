//! `error` — tipo de error unificado del BMO ABI.

#![allow(dead_code)]

use crate::bmo_abi::primitives::bx_u32;
use crate::bmo_abi::status::ErrorCode;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BmoError {
    pub code: bx_u32,
    pub _pad: bx_u32,
}

impl BmoError {
    pub const fn ok() -> Self {
        Self { code: ErrorCode::OK, _pad: 0 }
    }

    pub const fn from_code(code: bx_u32) -> Self {
        Self { code, _pad: 0 }
    }

    pub const fn is_ok(&self) -> bool {
        self.code == ErrorCode::OK
    }

    pub const fn is_err(&self) -> bool {
        self.code != ErrorCode::OK
    }

    pub const fn message(&self) -> &'static str {
        ErrorCode::message(self.code)
    }
}
