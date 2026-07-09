//! Language standard profiles — embedded definitions for C, C++, COBOL.
//!
//! Each profile defines which language features, type rules, and predefined
//! macros are active for a given standard version. Frontends query these at
//! compile time via `--std=c11`, `--std=c++17`, etc.
//!
//! Data is compiled-in (no filesystem dependency), backed by TOML files in
//! `Semantic_ASM/standards/` which serve as the human-readable source of truth.

/// A language standard profile.
#[derive(Debug, Clone)]
pub struct StandardProfile {
    /// ISO standard number, e.g. "ISO/IEC 9899:2011"
    pub iso_number: &'static str,
    /// Year of publication.
    pub year: u16,
    /// Short name: "C89", "C99", "C++17", "COBOL-85"
    pub short_name: &'static str,
    /// Language: "c", "cpp", "cobol"
    pub language: &'static str,
    /// Feature flags: name → enabled/disabled.
    pub features: &'static [(&'static str, bool)],
    /// Predefined macros: name → value.
    pub macros: &'static [(&'static str, i64)],
    /// Parent standard (for inheritance).
    pub parent: Option<&'static str>,
}

impl StandardProfile {
    /// Check if a feature is enabled.
    pub fn has(&self, feature: &str) -> bool {
        self.features.iter().any(|(k, v)| *k == feature && *v)
    }

    /// Get a predefined macro value. Returns None if not defined.
    pub fn macro_value(&self, name: &str) -> Option<i64> {
        self.macros.iter().find(|(k, _)| *k == name).map(|(_, v)| *v)
    }

    /// Check if this standard inherits from another.
    pub fn inherits_from(&self, parent: &str) -> bool {
        self.parent == Some(parent)
    }
}

pub mod c;
pub mod cpp;
pub mod cobol;

/// Find a standard profile by language and version.
pub fn find(language: &str, version: &str) -> Option<&'static StandardProfile> {
    match language {
        "c" => c::find(version),
        "cpp" | "c++" => cpp::find(version),
        "cobol" => cobol::find(version),
        _ => None,
    }
}

/// All available C standards.
pub fn c_standards() -> &'static [&'static StandardProfile] {
    c::ALL
}

/// All available C++ standards.
pub fn cpp_standards() -> &'static [&'static StandardProfile] {
    cpp::ALL
}

/// All available COBOL standards.
pub fn cobol_standards() -> &'static [&'static StandardProfile] {
    cobol::ALL
}
