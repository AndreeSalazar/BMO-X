//! `lang::common::diagnostics` — Errores, warnings y notas de compilación.
//!
//! Todos los frontends reportan diagnósticos a través de este módulo.
//! Cada diagnóstico tiene severidad, mensaje, y `Span` de origen.

#![allow(dead_code)]

use super::source::Span;
use core::fmt;

/// Severidad de un diagnóstico.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// Información (no afecta el build).
    Note,
    /// Advertencia (no bloquea el build).
    Warning,
    /// Error (bloquea el build).
    Error,
}

impl Severity {
    pub fn prefix(self) -> &'static str {
        match self {
            Self::Note => "note",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.prefix())
    }
}

/// Código de error canónico del compilador. Cada frontend puede agregar
/// sus propios códigos, pero estos son comunes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DiagCode {
    /// Error de sintaxis genérico.
    SyntaxError,
    /// Token inesperado.
    UnexpectedToken,
    /// Token esperado pero no encontrado.
    ExpectedToken,
    /// Identificador no declarado.
    UndefinedSymbol,
    /// Símbolo redeclarado.
    RedefinedSymbol,
    /// Tipos incompatibles.
    TypeMismatch,
    /// Tipo incorrecto para la operación.
    InvalidType,
    /// Llamada a función con número incorrecto de argumentos.
    WrongArgCount,
    /// División por cero (constante).
    DivisionByZero,
    /// Index fuera de rango (constante).
    OutOfBounds,
    /// Referencia a un label/goto que no existe.
    UndefinedLabel,
    /// #include no encontrado.
    IncludeNotFound,
    /// #define recursivo o circular.
    MacroRecursion,
    /// Caracter UTF-8 inválido.
    InvalidUtf8,
    /// Literal numérico malformado.
    InvalidNumber,
    /// Caracter de escape inválido en string/char.
    InvalidEscape,
    /// Caracter no soportado por el lenguaje.
    UnsupportedFeature,
    /// Otro error (mensaje arbitrario).
    Other,
}

impl DiagCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SyntaxError => "E0001",
            Self::UnexpectedToken => "E0002",
            Self::ExpectedToken => "E0003",
            Self::UndefinedSymbol => "E0004",
            Self::RedefinedSymbol => "E0005",
            Self::TypeMismatch => "E0006",
            Self::InvalidType => "E0007",
            Self::WrongArgCount => "E0008",
            Self::DivisionByZero => "E0009",
            Self::OutOfBounds => "E0010",
            Self::UndefinedLabel => "E0011",
            Self::IncludeNotFound => "E0012",
            Self::MacroRecursion => "E0013",
            Self::InvalidUtf8 => "E0014",
            Self::InvalidNumber => "E0015",
            Self::InvalidEscape => "E0016",
            Self::UnsupportedFeature => "E0017",
            Self::Other => "E9999",
        }
    }
}

/// Un diagnóstico individual.
#[derive(Clone, Debug)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: DiagCode,
    pub message: String,
    pub span: Span,
}

impl Diagnostic {
    pub fn error(code: DiagCode, message: impl Into<String>, span: Span) -> Self {
        Self { severity: Severity::Error, code, message: message.into(), span }
    }
    pub fn warning(code: DiagCode, message: impl Into<String>, span: Span) -> Self {
        Self { severity: Severity::Warning, code, message: message.into(), span }
    }
    pub fn note(message: impl Into<String>, span: Span) -> Self {
        Self { severity: Severity::Note, code: DiagCode::Other, message: message.into(), span }
    }

    pub fn is_error(&self) -> bool { self.severity == Severity::Error }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}: {} ({})", self.span, self.severity, self.message, self.code.as_str())
    }
}

/// Acumulador de diagnósticos. Los compiladores crean uno, lo llenan
/// durante lex/parse/sema, y al final preguntan `has_errors()`.
#[derive(Clone, Debug, Default)]
pub struct Diagnostics {
    items: Vec<Diagnostic>,
}

impl Diagnostics {
    pub const fn new() -> Self { Self { items: Vec::new() } }

    pub fn push(&mut self, d: Diagnostic) { self.items.push(d); }

    pub fn error(&mut self, code: DiagCode, msg: impl Into<String>, span: Span) {
        self.push(Diagnostic::error(code, msg, span));
    }
    pub fn warning(&mut self, code: DiagCode, msg: impl Into<String>, span: Span) {
        self.push(Diagnostic::warning(code, msg, span));
    }

    pub fn has_errors(&self) -> bool {
        self.items.iter().any(|d| d.is_error())
    }
    pub fn len(&self) -> usize { self.items.len() }
    pub fn is_empty(&self) -> bool { self.items.is_empty() }

    pub fn iter(&self) -> core::slice::Iter<'_, Diagnostic> { self.items.iter() }

    /// Convierte los diagnósticos a un solo String (uno por línea).
    pub fn to_string_lossy(&self) -> String {
        let mut s = String::new();
        for d in &self.items {
            s.push_str(&d.to_string());
            s.push('\n');
        }
        s
    }
}
