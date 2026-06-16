//! Error types for BMOasm parser with line/column location info.

extern crate alloc;
use alloc::string::String;

/// Parser error with source location.
#[derive(Debug, Clone)]
pub struct ParseError {
    pub line: usize,
    pub col: usize,
    pub msg: &'static str,
    pub expected: Option<&'static str>,
    pub found: Option<&'static str>,
}

impl ParseError {
    pub const fn new(line: usize, col: usize, msg: &'static str) -> Self {
        Self { line, col, msg, expected: None, found: None }
    }

    pub fn with_expected(mut self, expected: &'static str) -> Self {
        self.expected = Some(expected);
        self
    }

    pub fn with_found(mut self, found: &'static str) -> Self {
        self.found = Some(found);
        self
    }

    /// Format: "line 5, col 12: expected '{', found 'ident'"
    pub fn format(&self) -> String {
        use alloc::fmt::Write;
        let mut s = String::new();
        let _ = write!(s, "line {}, col {}: {}", self.line, self.col, self.msg);
        if let Some(exp) = self.expected {
            let _ = write!(s, " (expected {})", exp);
        }
        if let Some(fnd) = self.found {
            let _ = write!(s, ", found '{}'", fnd);
        }
        s
    }
}

impl core::fmt::Display for ParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "line {}, col {}: {}", self.line, self.col, self.msg)?;
        if let Some(exp) = self.expected {
            write!(f, " (expected {})", exp)?;
        }
        if let Some(fnd) = self.found {
            write!(f, ", found '{}'", fnd)?;
        }
        Ok(())
    }
}

/// Convert ParseError to BxResult (loses location info, kept for ABI compat).
impl From<ParseError> for crate::barex::BxError {
    fn from(_: ParseError) -> Self {
        crate::barex::BxError::InvalidArgument
    }
}

pub type ParseResult<T> = core::result::Result<T, ParseError>;
