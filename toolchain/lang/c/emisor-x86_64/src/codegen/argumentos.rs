//! **LOS ARGUMENTOS DE UNA LLAMADA, a donde la convencion dice.**
//!
//! [fase]     EMISION
//!
//! [aparece]  BANCO -- un argumento en el registro equivocado es un parametro
//!            con el valor de otro, y el banco lo ve en la primera fila que
//!            llame con dos argumentos distintos
//!
//! [carril]   VERDE -- y NO se elige: sale de su `[aparece]` (BANCO).
//!            Ver la tabla en toolchain/tools/fases/fases.py
//!
//! `decidir/llamada.rs` dice POR DONDE viaja cada argumento; esto los lleva.
//! Son dos pasadas separadas porque el llamante indirecto (`CallPtr`) tiene
//! que meter la direccion de la funcion entre las dos:
//!
//! ```text
//!    1. la PILA      los de la pila, de derecha a izquierda, como siempre
//!    2. REGISTROS    tres clases, en este orden:
//!                    - los que PISAN registros (una llamada dentro, una
//!                      division): se calculan en orden, se empujan, y se
//!                      sacan al reves a su registro
//!                    - los que solo tocan rax y rcx (`sin_pila`): se calculan
//!                      y se mueven con UN `mov`; el que va a rcx, el ultimo,
//!                      porque evaluar otro despues lo pisaria
//!                    - los SIMPLES --una variable, una constante-- directos,
//!                      que no tocan nada
//! ```
//!
//! [!] Orden de evaluacion: los de la pila antes que los de registro, y entre
//! los de registro los complejos antes que los simples. C no fija ninguno;
//! `f(x, x++)` es indefinido en cualquier compilador y aqui tambien.

use crate::ast::{Expr, TypeSpec};

use super::agregados;
use super::decidir::llamada::Paso;
use super::Codegen;

impl Codegen {
    /// La pasada 1: los argumentos que van por la pila. Devuelve las ranuras
    /// empujadas (para el `add rsp` de despues de la llamada).
    pub(super) fn emit_argumentos_pila(&mut self, args: &[Expr], tipos: &[TypeSpec], pasos: &[Paso]) -> u32 {
        let mut ranuras = 0u32;
        for (i, arg) in args.iter().enumerate().rev() {
            if pasos[i] != Paso::Pila {
                continue;
            }
            match tipos.get(i) {
                Some(t) if self.es_agregado(t) => {
                    let bytes = self.type_stack_size(t);
                    ranuras += agregados::ranuras(bytes);
                    self.emit_empuja_agregado(arg, bytes);
                }
                Some(t) if Self::is_float_ty(t) => {
                    ranuras += 1;
                    let estrecho = matches!(t, TypeSpec::Float);
                    self.emit_empuja_flotante(arg, estrecho);
                }
                _ => {
                    ranuras += 1;
                    self.emit_expr(arg);
                    self.code.push(0x50); // push rax
                }
            }
        }
        ranuras
    }

    /// La pasada 2: los argumentos de registro, complejos por la pila y
    /// simples directos.
    pub(super) fn emit_argumentos_registro(&mut self, args: &[Expr], pasos: &[Paso]) {
        // 2a) los que pisan registros: por la pila
        let mut pendientes: Vec<u8> = Vec::new();
        for (i, arg) in args.iter().enumerate() {
            if let Paso::Registro(reg) = pasos[i] {
                if !self.argumento_simple(arg) && !self.sin_pila(arg) {
                    self.emit_expr(arg);
                    self.code.push(0x50); // push rax
                    pendientes.push(reg);
                }
            }
        }
        for &reg in pendientes.iter().rev() {
            self.emit_pop_reg(reg);
        }
        // 2b) los que solo tocan rax y rcx: un `mov`, y el de rcx el ultimo
        let mut a_rcx: Option<&Expr> = None;
        for (i, arg) in args.iter().enumerate() {
            if let Paso::Registro(reg) = pasos[i] {
                if !self.argumento_simple(arg) && self.sin_pila(arg) {
                    if reg == 1 {
                        a_rcx = Some(arg);
                        continue;
                    }
                    self.emit_expr(arg);
                    self.emit_mov_reg_rax(reg);
                }
            }
        }
        if let Some(arg) = a_rcx {
            self.emit_expr(arg);
            self.emit_mov_reg_rax(1);
        }
        // 2c) los simples, directos
        for (i, arg) in args.iter().enumerate() {
            if let Paso::Registro(reg) = pasos[i] {
                if self.argumento_simple(arg) {
                    let hecho = self.emit_argumento_en(arg, reg);
                    debug_assert!(hecho, "un argumento simple que no se pudo cargar");
                }
            }
        }
    }

    /// `mov reg, rax`.
    fn emit_mov_reg_rax(&mut self, reg: u8) {
        self.code.extend_from_slice(&[0x48 | (reg >> 3), 0x89, 0xC0 | (reg & 7)]);
    }
}
