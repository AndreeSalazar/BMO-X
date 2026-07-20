//! `convert` — conversiones entre los tipos de error del BMO ABI.
//!
//! BmoStatus ↔ BmoError ↔ ErrorCode ↔ BmoResult. Un solo punto de
//! traducción para que el resto del kernel nunca tenga que pensar en esto.

use crate::bmo_abi::fundamentals::error::BmoError;
use crate::bmo_abi::fundamentals::result::BmoResult;
use crate::bmo_abi::fundamentals::status::code::StatusFlags;
use crate::bmo_abi::fundamentals::status::BmoStatus;
use crate::bmo_abi::primitives::bx_u32;

// ─── BmoStatus → BmoError ──────────────────────────────────────────

impl From<BmoStatus> for BmoError {
    fn from(s: BmoStatus) -> Self {
        BmoError::from_status(s)
    }
}

impl From<BmoError> for BmoStatus {
    fn from(e: BmoError) -> Self {
        e.into_status()
    }
}

// ─── u32 (raw error_code) → BmoError/BmoStatus ─────────────────────

impl From<bx_u32> for BmoError {
    fn from(code: bx_u32) -> Self {
        BmoError::new(code)
    }
}

impl From<bx_u32> for BmoStatus {
    fn from(code: bx_u32) -> Self {
        BmoStatus::err(code)
    }
}

// ─── BmoResult<T, BmoStatus> / BmoResult<T, BmoError> bridges ──────

impl<T: Copy> BmoResult<T, BmoError> {
    /// Collapse into BmoStatus (losing the ok value).
    pub fn into_status(self) -> BmoStatus {
        if self.is_ok() {
            BmoStatus::OK
        } else {
            self.unwrap_err().into_status()
        }
    }
}

impl<T: Copy> BmoResult<T, BmoStatus> {
    /// Promote a BmoStatus-backed result to BmoError.
    pub fn into_error_result(self) -> BmoResult<T, BmoError> {
        if self.is_ok() {
            BmoResult::ok(self.unwrap())
        } else {
            BmoResult::err(self.unwrap_err().into())
        }
    }
}

// ─── StatusFlags helpers ───────────────────────────────────────────

impl BmoStatus {
    /// True if this status indicates a retryable operation.
    pub fn is_retryable(&self) -> bool {
        self.has_flag(StatusFlags::RETRY.bits())
    }

    /// True if the result was partial.
    pub fn is_partial(&self) -> bool {
        self.has_flag(StatusFlags::PARTIAL.bits())
    }
}

impl BmoError {
    /// True if this error indicates a retryable operation.
    pub const fn is_retryable(&self) -> bool {
        (self.flags & StatusFlags::RETRY.bits()) != 0
    }
}

// ─── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_error_roundtrip() {
        let e = BmoError::new(crate::bmo_abi::error_code::NOT_FOUND);
        let s: BmoStatus = e.into();
        let e2: BmoError = s.into();
        assert_eq!(e, e2);
    }

    #[test]
    fn ok_status_roundtrip() {
        let s = BmoStatus::OK;
        let e: BmoError = s.into();
        assert!(e.is_ok());
    }
}
