//! `backends::aot_x86_64::codegen` — Code generator principal.
//!
//! Convierte un `common::ast::Module` a bytes x86-64 usando el
//! `Emitter`, el `RegAlloc`, y las convenciones del `abi`.
//!
//! v1.8.8: **skeleton funcional** — compila el pipeline pero todavía
//! no emite todas las instrucciones del common IR. Las que faltan
//! retornan `Err(BxError::Unsupported)` para que el caller sepa
//! qué features aún no están implementadas.

#![allow(dead_code)]

use crate::bmo_gpu::BxResult;
use crate::lang::common::ast::{Module, Item, Stmt, Expr, Block};
use super::emit::Emitter;
use super::abi::Reg;

/// Compila un módulo completo a bytes x86-64.
///
/// v1.8.8: retorna los bytes de la primera función que encuentre.
/// El linker (futuro) unirá múltiples funciones.
pub fn compile_module(module: &Module) -> BxResult<alloc::vec::Vec<u8>> {
    for item in &module.items {
        if let Item::Function { body, .. } = item {
            return compile_function(body);
        }
    }
    Err(crate::bmo_gpu::BxError::NotFound)
}

/// Compila una función a bytes x86-64.
pub fn compile_function(body: &Block) -> BxResult<alloc::vec::Vec<u8>> {
    let mut em = Emitter::new();

    // Prologue: rbp = rsp; rsp -= frame_size.
    em.mov_rbp_rsp();
    em.sub_rsp_imm(64); // placeholder frame

    // Body
    for stmt in &body.stmts {
        emit_stmt(stmt, &mut em)?;
    }

    // Epilogue
    em.add_rsp_imm(64);
    em.ret();

    let mut out = alloc::vec::Vec::with_capacity(em.code_len);
    out.extend_from_slice(em.bytes());
    Ok(out)
}

fn emit_stmt(stmt: &Stmt, em: &mut Emitter) -> BxResult<()> {
    match stmt {
        Stmt::Expr(e, _) => emit_expr(e, em),
        Stmt::Return(Some(e), _) => {
            emit_expr(e, em)?;
            // El valor de retorno ya está en RAX (de la última expr).
            Ok(())
        }
        Stmt::Return(None, _) => Ok(()),
        Stmt::Empty(_) => Ok(()),
        Stmt::Block(b) => {
            for s in &b.stmts { emit_stmt(s, em)?; }
            Ok(())
        }
        _ => Err(crate::bmo_gpu::BxError::Unsupported),
    }
}

fn emit_expr(expr: &Expr, em: &mut Emitter) -> BxResult<()> {
    match expr {
        Expr::IntLit { value, .. } => {
            em.mov_rax_imm64(*value as u64);
            Ok(())
        }
        Expr::BoolLit { value, .. } => {
            em.mov_rax_imm64(if *value { 1 } else { 0 });
            Ok(())
        }
        Expr::Null(_) => {
            em.mov_rax_imm64(0);
            Ok(())
        }
        _ => Err(crate::bmo_gpu::BxError::Unsupported),
    }
}
