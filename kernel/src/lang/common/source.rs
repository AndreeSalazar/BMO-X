//! `lang::common::source` — Source location, span, file id.
//!
//! Tipos compartidos por todos los frontends. Cada token, nodo, o error
//! lleva su `Span` para reportar posición en el código fuente.

#![allow(dead_code)]

use core::fmt;

/// Posición en el código fuente: `byte_offset` desde el inicio del archivo
/// + `line` + `column` (1-based).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Pos {
    pub byte_offset: u32,
    pub line: u32,
    pub column: u32,
}

impl Pos {
    pub const ZERO: Self = Self { byte_offset: 0, line: 1, column: 1 };

    pub const fn new(byte_offset: u32, line: u32, column: u32) -> Self {
        Self { byte_offset, line, column }
    }
}

impl fmt::Display for Pos {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

/// Rango en el código fuente: `[start, end)`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: Pos,
    pub end: Pos,
}

impl Span {
    pub const fn new(start: Pos, end: Pos) -> Self {
        Self { start, end }
    }

    pub const fn point(p: Pos) -> Self {
        Self { start: p, end: p }
    }

    /// Span vacío al inicio del archivo.
    pub const ZERO: Self = Self { start: Pos::ZERO, end: Pos::ZERO };

    /// `true` si el span es un punto (start == end).
    pub const fn is_point(self) -> bool {
        self.start.byte_offset == self.end.byte_offset
    }

    /// Largo en bytes.
    pub const fn len(self) -> u32 {
        self.end.byte_offset - self.start.byte_offset
    }

    /// Une dos spans en uno que cubre ambos.
    pub fn join(self, other: Span) -> Span {
        Span {
            start: if self.start.byte_offset < other.start.byte_offset {
                self.start
            } else {
                other.start
            },
            end: if self.end.byte_offset > other.end.byte_offset {
                self.end
            } else {
                other.end
            },
        }
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_point() {
            write!(f, "{}", self.start)
        } else {
            write!(f, "{}-{}", self.start, self.end)
        }
    }
}

/// Identifica un archivo fuente. Permite que el compilador reporte
/// `path:line:col` correctamente cuando hay múltiples archivos.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FileId(pub u32);

impl FileId {
    pub const SYNTHETIC: Self = Self(0);
    pub const INVALID: Self = Self(u32::MAX);
}
