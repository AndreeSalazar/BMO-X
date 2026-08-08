//! COBOL dialect library -- the BMO toolchain ingredient.
//!
//! COBOL never lives in Ring 0: it is a *library* of dialects at the
//! toolchain layer. Every dialect lowers to the same BMO ABI v2 surface
//! (BEF -> linker -> BEX), the way LLVM frontends lower to one IR -- except
//! the BMO pipeline carries no patches: one canonical contract, enforced
//! by `bmo-abi` validation at every step.
//!
//! A `Dialect` only changes what the *parser* accepts (reserved words,
//! source format, extensions). Codegen and the ABI surface are shared by
//! all dialects by construction.

/// Source reference format accepted by the scanner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceFormat {
    /// Columns 8..72, sequence area 1..6, indicator column 7 (ANSI).
    Fixed,
    /// Free-form source (COBOL 2002 / GnuCOBOL `-free`).
    Free,
    /// Accept either, decided per line (GnuCOBOL `-F auto`).
    Auto,
}

/// A COBOL dialect the BMO frontend can parse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    /// ANSI X3.23 / ISO 1989 -- the portable baseline. Default.
    Ansi85,
    /// ISO/IEC 1989:2002 (free-form source, inline comments).
    Cobol2002,
    /// GnuCOBOL extensions (the `extern/gnucobol-rs` bridge speaks this).
    GnuCobol,
    /// IBM Enterprise COBOL (COMP-3 packed decimal, EBCDIC pictures).
    IbmEnterprise,
    /// Micro Focus (Net Express / Visual COBOL surface syntax).
    MicroFocus,
    /// ACUCOBOL-GT (screen section extensions).
    AcuCobol,
}

impl Dialect {
    pub const DEFAULT: Self = Self::Ansi85;

    /// Canonical lowercase name (matches GnuCOBOL `-std` spellings where
    /// one exists, so build scripts can pass values straight through).
    pub const fn name(self) -> &'static str {
        match self {
            Self::Ansi85 => "cobol85",
            Self::Cobol2002 => "cobol2002",
            Self::GnuCobol => "default",
            Self::IbmEnterprise => "ibm",
            Self::MicroFocus => "mf",
            Self::AcuCobol => "acu",
        }
    }

    /// Parse a dialect name (`-std=` style). `None` for unknown names.
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "cobol85" | "ansi" | "ansi85" => Some(Self::Ansi85),
            "cobol2002" | "2002" => Some(Self::Cobol2002),
            "default" | "gnucobol" | "gnu" => Some(Self::GnuCobol),
            "ibm" | "enterprise" => Some(Self::IbmEnterprise),
            "mf" | "microfocus" => Some(Self::MicroFocus),
            "acu" | "acucobol" => Some(Self::AcuCobol),
            _ => None,
        }
    }

    /// Source format this dialect defaults to.
    pub const fn source_format(self) -> SourceFormat {
        match self {
            Self::Ansi85 | Self::IbmEnterprise => SourceFormat::Fixed,
            Self::Cobol2002 => SourceFormat::Free,
            Self::GnuCobol | Self::MicroFocus | Self::AcuCobol => SourceFormat::Auto,
        }
    }

    /// Whether the dialect admits `*>` inline comments.
    pub const fn allows_inline_comments(self) -> bool {
        !matches!(self, Self::Ansi85)
    }

    /// Whether the dialect admits underscores in user-defined words
    /// (GnuCOBOL / Micro Focus extension).
    pub const fn allows_underscore_in_words(self) -> bool {
        matches!(self, Self::GnuCobol | Self::MicroFocus | Self::AcuCobol)
    }
}

/// Parser-facing dialect configuration, derived from a `Dialect`.
#[derive(Debug, Clone, Copy)]
pub struct DialectConfig {
    pub dialect: Dialect,
    pub source_format: SourceFormat,
    pub inline_comments: bool,
    pub underscore_in_words: bool,
}

impl DialectConfig {
    pub const fn of(dialect: Dialect) -> Self {
        Self {
            dialect,
            source_format: dialect.source_format(),
            inline_comments: dialect.allows_inline_comments(),
            underscore_in_words: dialect.allows_underscore_in_words(),
        }
    }
}

impl Default for DialectConfig {
    fn default() -> Self {
        Self::of(Dialect::DEFAULT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_dialect_round_trips_through_its_name() {
        for d in [
            Dialect::Ansi85,
            Dialect::Cobol2002,
            Dialect::GnuCobol,
            Dialect::IbmEnterprise,
            Dialect::MicroFocus,
            Dialect::AcuCobol,
        ] {
            assert_eq!(Dialect::from_name(d.name()), Some(d));
        }
    }

    #[test]
    fn ansi85_is_the_strict_baseline() {
        let cfg = DialectConfig::of(Dialect::Ansi85);
        assert_eq!(cfg.source_format, SourceFormat::Fixed);
        assert!(!cfg.inline_comments);
        assert!(!cfg.underscore_in_words);
    }
}
