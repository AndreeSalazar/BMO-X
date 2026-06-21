//! `convert` — Conversiones entre tipos de error del BMO ABI.
//!
//! Unifica BmoError, BmoStatus y ErrorCode en un sistema coherente.
//! Todo error puede convertirse a cualquier otro formato sin pérdida.

#![allow(dead_code)]

use crate::bmo_core::bmo_abi::primitives::bx_u32;
use crate::bmo_core::bmo_abi::status::code::BmoStatus;
use crate::bmo_core::bmo_abi::status::error::error_code;
use super::error::BmoError;

impl From<BmoError> for BmoStatus {
    fn from(e: BmoError) -> Self {
        BmoStatus::err(e.code)
    }
}

impl From<BmoStatus> for BmoError {
    fn from(s: BmoStatus) -> Self {
        BmoError::from_code(s.code)
    }
}

impl From<BmoError> for bx_u32 {
    fn from(e: BmoError) -> Self {
        e.code
    }
}

impl From<bx_u32> for BmoError {
    fn from(code: bx_u32) -> Self {
        BmoError::from_code(code)
    }
}

impl From<BmoStatus> for bx_u32 {
    fn from(s: BmoStatus) -> Self {
        s.code
    }
}

impl BmoError {
    pub fn to_status(self) -> BmoStatus {
        BmoStatus::from(self)
    }

    pub fn to_u32(self) -> bx_u32 {
        self.code
    }

    pub fn is_io_error(&self) -> bool {
        self.code == error_code::IO_ERROR
    }

    pub fn is_not_found(&self) -> bool {
        self.code == error_code::NOT_FOUND
    }

    pub fn is_permission_denied(&self) -> bool {
        self.code == error_code::PERMISSION_DENIED
    }

    pub fn is_timeout(&self) -> bool {
        self.code == error_code::TIMEOUT
    }

    pub fn is_would_block(&self) -> bool {
        self.code == error_code::WOULD_BLOCK
    }
}

impl BmoStatus {
    pub fn to_error(self) -> BmoError {
        BmoError::from(self)
    }

    pub fn into_result<T: Default>(self, value: T) -> Result<T, BmoError> {
        if self.is_ok() {
            Ok(value)
        } else {
            Err(self.to_error())
        }
    }
}

pub fn ok_status() -> BmoStatus {
    BmoStatus::OK
}

pub fn err_status(code: bx_u32) -> BmoStatus {
    BmoStatus::err(code)
}

pub fn ok_error() -> BmoError {
    BmoError::ok()
}
