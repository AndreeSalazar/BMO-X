//! `backends::aot_x86_64::regs` — Register Allocator.
//!
//! Asigna variables locales del common IR a registros x86-64 o a
//! slots del stack.
//!
//! ## Algoritmo (v1.8.8)
//!
//! - **Linear scan simplificado**: las variables se asignan en orden
//!   de aparición.
//! - Primero se usan los registros callee-saved (`R12..R15`, `RBX`).
//! - Cuando se acaban, se hace spill al stack (offsets negativos
//!   desde `RBP`).
//! - Los argumentos se asignan a `RDI..R9` por orden.

#![allow(dead_code)]

use super::abi::Reg;

const MAX_VARS: usize = 64;

/// Tabla de variables y su ubicación.
pub struct RegAlloc {
    /// Slot en el stack para cada variable (offset negativo desde RBP).
    stack: [i32; MAX_VARS],
    /// Registro asignado a cada variable (None = stack).
    regs: [Option<Reg>; MAX_VARS],
    /// Frame size actual.
    frame_size: i32,
}

impl RegAlloc {
    pub const fn new() -> Self {
        Self {
            stack: [0; MAX_VARS],
            regs: [None; MAX_VARS],
            frame_size: 0,
        }
    }

    /// Frame size después de alloc.
    pub fn frame_size(&self) -> i32 { self.frame_size }

    /// Ubicación de una variable.
    pub fn location(&self, var: Var) -> Location {
        if let Some(r) = self.regs[var.0] {
            Location::Reg(r)
        } else {
            Location::Stack(self.stack[var.0])
        }
    }
}

/// Identificador opaco de variable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Var(pub usize);

impl Var {
    pub const fn new(idx: usize) -> Self { Self(idx) }
}

/// Dónde vive una variable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Location {
    /// En un registro.
    Reg(Reg),
    /// En el stack (offset negativo desde RBP).
    Stack(i32),
}

impl Location {
    /// `true` si está en un registro.
    pub fn is_reg(self) -> bool { matches!(self, Self::Reg(_)) }
}
