//! `version` — BmoVersion, tipo semver del BMO ABI.
//!
//! Reemplaza la práctica de C de pasar `u32 major << 16 | minor << 8 | patch`
//! con un tipo explícito que tiene operaciones de comparación semántica.

use crate::bmo_abi::primitives::bx_u32;
use crate::bmo_abi::error_code;
use crate::bmo_abi::fundamentals::status::BmoStatus;

/// Semantic version: major.minor.patch (12 bytes).
///
/// # Layout
/// ```text
/// [0..3] major: u32
/// [4..7] minor: u32
/// [8..11] patch: u32
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BmoVersion {
    pub major: bx_u32,
    pub minor: bx_u32,
    pub patch: bx_u32,
}

impl BmoVersion {
    pub const ZERO: Self = Self { major: 0, minor: 0, patch: 0 };

    pub const fn new(major: bx_u32, minor: bx_u32, patch: bx_u32) -> Self {
        Self { major, minor, patch }
    }

    /// True if this version is compatible with `required`.
    ///
    /// Compatible means: same major, and this minor >= required minor.
    pub fn is_compatible_with(&self, required: &BmoVersion) -> bool {
        self.major == required.major && self.minor >= required.minor
    }

    /// Check compatibility, returning a `BmoStatus`.
    pub fn check_compatibility(&self, required: &BmoVersion) -> BmoStatus {
        if self.is_compatible_with(required) {
            BmoStatus::OK
        } else {
            BmoStatus::err(error_code::VERSION)
        }
    }
}

impl PartialOrd for BmoVersion {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for BmoVersion {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        match self.major.cmp(&other.major) {
            core::cmp::Ordering::Equal => {}
            ord => return ord,
        }
        match self.minor.cmp(&other.minor) {
            core::cmp::Ordering::Equal => {}
            ord => return ord,
        }
        self.patch.cmp(&other.patch)
    }
}

impl core::fmt::Display for BmoVersion {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;

    #[test]
    fn version_compatible() {
        let v1 = BmoVersion::new(1, 5, 0);
        let v2 = BmoVersion::new(1, 6, 0);
        assert!(v2.is_compatible_with(&v1));
        assert!(!v1.is_compatible_with(&v2));
    }

    #[test]
    fn version_incompatible_major() {
        let v1 = BmoVersion::new(1, 0, 0);
        let v2 = BmoVersion::new(2, 0, 0);
        assert!(!v2.is_compatible_with(&v1));
    }

    #[test]
    fn version_ordering() {
        assert!(BmoVersion::new(1, 0, 0) < BmoVersion::new(2, 0, 0));
        assert!(BmoVersion::new(1, 5, 0) > BmoVersion::new(1, 0, 0));
    }

    #[test]
    fn version_display() {
        let v = BmoVersion::new(3, 2, 1);
        assert_eq!(format!("{}", v), "3.2.1");
    }

    #[test]
    fn check_compat_ok() {
        let v = BmoVersion::new(2, 1, 0);
        let req = BmoVersion::new(2, 0, 0);
        assert!(v.check_compatibility(&req).is_ok());
    }

    #[test]
    fn check_compat_err() {
        let v = BmoVersion::new(2, 0, 0);
        let req = BmoVersion::new(3, 0, 0);
        assert!(v.check_compatibility(&req).is_err());
    }
}
