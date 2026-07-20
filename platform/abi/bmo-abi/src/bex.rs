//! BEX — BMO Executable.
//!
//! A BEX file is the executable artifact consumed by BMO Ring 3.  Version 1
//! deliberately uses the existing, stable BEF1 wire layout; `bex` names the
//! executable contract, while `bef` remains the binary container and loader
//! implementation.  This lets the kernel, userspace and offline frontends
//! share one format without a flag-day header migration.

use crate::bef::{self, validator::ValidationResult};

/// File extension for native BMO executables (without the leading dot).
pub const BEX_EXTENSION: &str = "bex";

/// Human-readable name of the executable contract.
pub const BEX_FORMAT_NAME: &str = "BMO Executable";

/// BEX v1 is encoded with the canonical BEF1 header.
pub const BEX_WIRE_MAGIC: u32 = bef::BEF_MAGIC;

/// Validate a BEX image before it is mapped into a Ring 3 process.
///
/// This is intentionally structural only.  The Ring 0 loader must still
/// enforce process limits, page permissions, import policy and capabilities.
pub fn validate(bytes: &[u8]) -> ValidationResult {
    bef::validate(bytes)
}

/// Returns whether a path-like name denotes a BMO executable.
pub fn has_bex_extension(name: &str) -> bool {
    name.rsplit_once('.')
        .map(|(_, extension)| extension.eq_ignore_ascii_case(BEX_EXTENSION))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bef::writer::{BefBuilder, BefSection};
    use alloc::vec;

    #[test]
    fn bex_v1_uses_the_canonical_bef_wire_format() {
        let mut builder = BefBuilder::new();
        builder.add_section(BefSection::code(vec![0xC3]));
        let image = builder.build().unwrap();

        assert!(validate(&image).is_valid);
        assert_eq!(BEX_WIRE_MAGIC, bef::BEF_MAGIC);
    }

    #[test]
    fn recognizes_bex_extension_case_insensitively() {
        assert!(has_bex_extension("hello.bex"));
        assert!(has_bex_extension("HELLO.BEX"));
        assert!(!has_bex_extension("hello.bef"));
    }
}
