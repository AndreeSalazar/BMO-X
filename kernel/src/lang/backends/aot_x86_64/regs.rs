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
//!
//! ## Layout del frame
//!
//! ```text
//! [higher addr]
//!   saved RBP          <- RBP points here
//!   [callee-saved regs]
//!   [local slots ...]  <- RBP - slot_offset
//! [lower addr]
//! ```

#![allow(dead_code)]

use super::abi::Reg;
use super::emit::Emitter;

const MAX_VARS: usize = 256;

/// Identificador opaco de variable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Var(pub u32);

impl Var {
    pub const INVALID: Self = Self(u32::MAX);
}

/// Dónde vive una variable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Location {
    /// En un registro.
    Reg(Reg),
    /// En el stack: offset negativo desde RBP.
    Stack(i32),
}

impl Location {
    pub fn is_reg(self) -> bool { matches!(self, Self::Reg(_)) }
}

/// Tipo de variable (para saber cuánto ocupa en stack).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VarSize {
    Byte = 1,
    Word = 2,
    Dword = 4,
    Qword = 8,
}

impl VarSize {
    pub fn bytes(self) -> i32 { self as i32 }
}

/// Entrada de variable.
#[derive(Clone, Copy, Debug)]
struct VarEntry {
    name: u32,        // StrId (para debug)
    size: VarSize,
    loc: Location,
}

/// Tabla de variables y su ubicación.
pub struct RegAlloc {
    vars: [Option<VarEntry>; MAX_VARS],
    n_vars: usize,
    next_reg_idx: usize,
    next_stack_offset: i32,
    /// Frame size ya calculado.
    frame_size: i32,
}

/// Registros callee-saved que podemos usar para variables locales.
const ALLOC_REGS: [Reg; 5] = [Reg::R12, Reg::R13, Reg::R14, Reg::R15, Reg::Rbx];

impl RegAlloc {
    pub const fn new() -> Self {
        Self {
            vars: [None; MAX_VARS],
            n_vars: 0,
            next_reg_idx: 0,
            next_stack_offset: 0,
            frame_size: 0,
        }
    }

    /// Reserva un slot para una variable local. Devuelve el `Var`.
    pub fn alloc(&mut self, name: u32, size: VarSize) -> Var {
        let loc = if self.next_reg_idx < ALLOC_REGS.len() {
            // Asignar a un registro callee-saved.
            let r = ALLOC_REGS[self.next_reg_idx];
            self.next_reg_idx += 1;
            Location::Reg(r)
        } else {
            // Spill al stack.
            let off = -(self.next_stack_offset + size.bytes());
            self.next_stack_offset += size.bytes();
            // Alinear a 8 bytes.
            self.next_stack_offset = (self.next_stack_offset + 7) & !7;
            Location::Stack(off)
        };
        let var = Var(self.n_vars as u32);
        self.vars[self.n_vars] = Some(VarEntry { name, size, loc });
        self.n_vars += 1;
        var
    }

    /// Reserva un slot para un argumento (RDI, RSI, RDX, RCX, R8, R9).
    pub fn alloc_arg(&mut self, name: u32, idx: usize) -> Var {
        let loc = match idx {
            0 => Location::Reg(Reg::Rdi),
            1 => Location::Reg(Reg::Rsi),
            2 => Location::Reg(Reg::Rdx),
            3 => Location::Reg(Reg::Rcx),
            4 => Location::Reg(Reg::R8),
            5 => Location::Reg(Reg::R9),
            _ => {
                // Stack arg: [rbp + 16 + 8*(idx-6)]
                let off = 16 + 8 * (idx as i32 - 6);
                Location::Stack(off)
            }
        };
        let var = Var(self.n_vars as u32);
        self.vars[self.n_vars] = Some(VarEntry { name, size: VarSize::Qword, loc });
        self.n_vars += 1;
        var
    }

    pub fn location(&self, var: Var) -> Location {
        if var.0 as usize >= self.n_vars { return Location::Stack(0); }
        self.vars[var.0 as usize].map(|e| e.loc).unwrap_or(Location::Stack(0))
    }

    /// Calcula el frame size después de alloc (debe llamarse antes del prologue).
    pub fn frame_size(&self) -> i32 {
        // 16 bytes de RBP + locals + alignment a 16
        let bytes = 16 + self.next_stack_offset;
        (bytes + 15) & !15
    }

    /// Lista de registros callee-saved que necesitamos guardar/restaurar.
    pub fn used_callee_saved(&self) -> &[Reg] {
        &ALLOC_REGS[..self.next_reg_idx.min(ALLOC_REGS.len())]
    }

    /// Emite `mov dst, var` (load de la variable a un registro).
    pub fn emit_load(&self, em: &mut Emitter, var: Var, dst: Reg) {
        match self.location(var) {
            Location::Reg(r) => {
                if r != dst {
                    em.mov_rr(dst, r);
                }
            }
            Location::Stack(off) => {
                em.mov_rm(dst, Reg::Rbp, off);
            }
        }
    }

    /// Emite `mov var, src` (store de un registro a la variable).
    pub fn emit_store(&self, em: &mut Emitter, var: Var, src: Reg) {
        match self.location(var) {
            Location::Reg(r) => {
                if r != src {
                    em.mov_rr(r, src);
                }
            }
            Location::Stack(off) => {
                em.mov_mr(Reg::Rbp, off, src);
            }
        }
    }

    /// Emite el address de una variable (para `&x` o `lea`).
    pub fn emit_addr(&self, em: &mut Emitter, var: Var, dst: Reg) {
        match self.location(var) {
            Location::Reg(_) => {
                // No se puede tomar address de un registro spill. Por ahora
                // emitimos un error silencioso (no se hace nada).
                // TODO: forzar spill a stack si se pide address.
                em.xor_rr(dst, dst);
            }
            Location::Stack(off) => {
                em.lea(dst, Reg::Rbp, off);
            }
        }
    }
}
