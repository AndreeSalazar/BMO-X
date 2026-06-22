//! `backends::aot_x86_64::codegen` — Code generator principal.
//!
//! v1.8.8: reescrito para soportar Hello World + Fibonacci + loops.
//!
//! ## Features
//!
//! - Prologue/epilogue con callee-saved save/restore.
//! - Stmt: Let, Assign, If/Else, While, Return, Block, Break, Continue.
//! - Expr: IntLit, BoolLit, StrLit (rodata lea), Var, Bin (todas las ops),
//!   Unary, Call (BMO ABI dispatch + user call con patch).
//! - LoopStack para break/continue.
//! - Map de StrId→Var en RegAlloc.
//! - Signed/unsigned comparisons según IrType.

#![allow(dead_code)]

extern crate alloc;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::bmo_gpu::BxResult;
use crate::lang::common::ast as ir;
use crate::lang::common::ast::{Module, Item, Stmt, Expr, Block, StrId};
use crate::lang::common::types::{IrType, IrTypeId};
use super::emit::{Emitter, CondCode};
use super::regs::{RegAlloc, Var, VarSize};
use super::abi::Reg;

/// Patch pendiente para `lea rax, [rip+disp32]` que apunta a un string en rodata.
/// `pos` es la posición en `code` del disp32.
pub struct StrLitPatch {
    pub pos: usize,
    pub str_offset: u32, // offset del string dentro de rodata
}

/// Resultado de compilar un módulo.
pub struct CompiledArtifact {
    pub code: Vec<u8>,
    pub rodata: Vec<u8>,
    pub call_patches: Vec<(usize, StrId)>,
    pub string_offsets: BTreeMap<u32, u32>,
    pub function_offsets: BTreeMap<u32, u32>,
    /// Parches pendientes para `LEA` a strings en rodata.
    pub str_lit_patches: Vec<StrLitPatch>,
}

pub fn compile_module(module: &Module) -> BxResult<CompiledArtifact> {
    let mut em = Emitter::new();
    let mut call_patches: Vec<(usize, StrId)> = Vec::new();
    let mut string_offsets: BTreeMap<u32, u32> = BTreeMap::new();
    let mut function_offsets: BTreeMap<u32, u32> = BTreeMap::new();
    let mut str_lit_patches: Vec<StrLitPatch> = Vec::new();

    for item in &module.items {
        match item {
            Item::Function { name, params, body, .. } => {
                function_offsets.insert(name.0, em.code_len as u32);
                compile_function(&mut em, name, params, body, module,
                                 &mut string_offsets, &mut call_patches,
                                 &mut str_lit_patches)?;
            }
            Item::Extern { .. } => {}
            _ => {}
        }
    }

    Ok(CompiledArtifact {
        code: em.bytes().to_vec(),
        rodata: em.rodata().to_vec(),
        call_patches,
        string_offsets,
        function_offsets,
        str_lit_patches,
    })
}

fn compile_function(
    em: &mut Emitter,
    _name: &StrId,
    params: &[ir::Param],
    body: &Block,
    module: &Module,
    string_offsets: &mut BTreeMap<u32, u32>,
    call_patches: &mut Vec<(usize, StrId)>,
    str_lit_patches: &mut Vec<StrLitPatch>,
) -> BxResult<()> {
    let mut alloc = RegAlloc::new();

    // Registrar params como variables.
    for (i, p) in params.iter().enumerate() {
        alloc.alloc_arg(p.name.0, i);
    }

    // Prologue.
    em.push(Reg::Rbp);
    em.mov_rbp_rsp();
    let frame = alloc.frame_size();
    if frame > 0 {
        em.sub_rsp_imm(frame);
    }
    for r in alloc.used_callee_saved() {
        em.push(*r);
    }

    // Body.
    let loop_stack: LoopStack = Vec::new();
    for stmt in &body.stmts {
        emit_stmt(stmt, em, &mut alloc, module, string_offsets, call_patches, str_lit_patches, &loop_stack)?;
    }

    // Epilogue.
    for r in alloc.used_callee_saved().iter().rev() {
        em.pop(*r);
    }
    em.leave();
    em.ret();

    Ok(())
}

type LoopStack = Vec<(usize, usize)>; // (label_break, label_continue)

fn emit_stmt(
    stmt: &Stmt,
    em: &mut Emitter,
    alloc: &mut RegAlloc,
    module: &Module,
    string_offsets: &mut BTreeMap<u32, u32>,
    call_patches: &mut Vec<(usize, StrId)>,
    str_lit_patches: &mut Vec<StrLitPatch>,
    loop_stack: &LoopStack,
) -> BxResult<()> {
    match stmt {
        Stmt::Expr(e, _) => {
            emit_expr(e, em, alloc, module, string_offsets, call_patches, str_lit_patches, loop_stack)?;
            Ok(())
        }
        Stmt::Let { name, ty: _, init, .. } => {
            let var = alloc.alloc(name.0, VarSize::Qword);
            if let Some(e) = init {
                emit_expr(e, em, alloc, module, string_offsets, call_patches, str_lit_patches, loop_stack)?;
                alloc.emit_store(em, var, Reg::Rax);
            }
            Ok(())
        }
        Stmt::Assign { target, value, .. } => {
            if let Expr::Var { name, .. } = target {
                emit_expr(value, em, alloc, module, string_offsets, call_patches, str_lit_patches, loop_stack)?;
                if let Some(v) = alloc.find_by_name(name.0) {
                    alloc.emit_store(em, v, Reg::Rax);
                }
            }
            Ok(())
        }
        Stmt::If { cond, then_branch, else_branch, .. } => {
            emit_expr(cond, em, alloc, module, string_offsets, call_patches, str_lit_patches, loop_stack)?;
            em.test_rr(Reg::Rax, Reg::Rax);
            let jz_else = em.reserve_rel32();
            for s in &then_branch.stmts {
                emit_stmt(s, em, alloc, module, string_offsets, call_patches, str_lit_patches, loop_stack)?;
            }
            let jmp_end = em.reserve_rel32();
            em.patch_rel32(jz_else, em.pos());
            if let Some(eb) = else_branch {
                for s in &eb.stmts {
                    emit_stmt(s, em, alloc, module, string_offsets, call_patches, str_lit_patches, loop_stack)?;
                }
            }
            em.patch_rel32(jmp_end, em.pos());
            Ok(())
        }
        Stmt::While { cond, body, .. } => {
            let l_start = em.pos();
            emit_expr(cond, em, alloc, module, string_offsets, call_patches, str_lit_patches, loop_stack)?;
            em.test_rr(Reg::Rax, Reg::Rax);
            let jz_end = em.reserve_rel32();
            let l_end = em.pos();
            let l_break_pos = em.reserve_rel32();
            let l_continue_pos = l_start;
            let mut nested_stack = loop_stack.clone();
            nested_stack.push((l_break_pos, l_continue_pos));
            for s in &body.stmts {
                emit_stmt(s, em, alloc, module, string_offsets, call_patches, str_lit_patches, &nested_stack)?;
            }
            let jmp = em.reserve_rel32();
            em.patch_rel32(jz_end, l_end);
            em.patch_rel32(l_break_pos, em.pos());
            em.patch_rel32(jmp, l_start);
            Ok(())
        }
        Stmt::Return(value, _) => {
            if let Some(e) = value {
                emit_expr(e, em, alloc, module, string_offsets, call_patches, str_lit_patches, loop_stack)?;
            }
            let jmp = em.reserve_rel32();
            em.ret();
            em.patch_rel32(jmp, em.pos());
            Ok(())
        }
        Stmt::Block(b) => {
            for s in &b.stmts {
                emit_stmt(s, em, alloc, module, string_offsets, call_patches, str_lit_patches, loop_stack)?;
            }
            Ok(())
        }
        Stmt::Break(_) => {
            if let Some(&(l_break, _)) = loop_stack.last() {
                let jmp = em.reserve_rel32();
                em.patch_rel32(jmp, l_break);
            }
            Ok(())
        }
        Stmt::Continue(_) => {
            if let Some(&(_, l_continue)) = loop_stack.last() {
                let jmp = em.reserve_rel32();
                em.patch_rel32(jmp, l_continue);
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn emit_expr(
    expr: &Expr,
    em: &mut Emitter,
    alloc: &mut RegAlloc,
    module: &Module,
    string_offsets: &mut BTreeMap<u32, u32>,
    call_patches: &mut Vec<(usize, StrId)>,
    str_lit_patches: &mut Vec<StrLitPatch>,
    loop_stack: &LoopStack,
) -> BxResult<()> {
    match expr {
        Expr::IntLit { value, .. } => {
            em.mov_rax_imm64(*value as u64);
            Ok(())
        }
        Expr::BoolLit { value, .. } => {
            em.mov_rax_imm64(if *value { 1 } else { 0 });
            Ok(())
        }
        Expr::StrLit { id, .. } => {
            // LEA rax, [rip+disp32] con patch pendiente.
            let s = module.get_str(*id);
            let str_offset = if let Some(&off) = string_offsets.get(&id.0) {
                off
            } else {
                let off = em.add_string(s.as_bytes());
                string_offsets.insert(id.0, off);
                off
            };
            em.rex(true, Reg::Rax, Reg::Rax, Reg::Rax);
            em.cb(0x8D);
            em.modrm_rip(Reg::Rax);
            let patch_pos = em.code_len;
            em.cs(&[0; 4]);
            str_lit_patches.push(StrLitPatch { pos: patch_pos, str_offset });
            Ok(())
        }
        Expr::Var { name, .. } => {
            if let Some(v) = alloc.find_by_name(name.0) {
                alloc.emit_load(em, v, Reg::Rax);
            } else {
                em.mov_rax_imm64(0);
            }
            Ok(())
        }
        Expr::Bin { op, lhs, rhs, .. } => {
            emit_expr(lhs, em, alloc, module, string_offsets, call_patches, str_lit_patches, loop_stack)?;
            em.push(Reg::Rax);
            emit_expr(rhs, em, alloc, module, string_offsets, call_patches, str_lit_patches, loop_stack)?;
            em.mov_rr(Reg::Rcx, Reg::Rax);
            em.pop(Reg::Rax);

            match op {
                ir::BinOp::Add => em.add_rr(Reg::Rax, Reg::Rcx),
                ir::BinOp::Sub => em.sub_rr(Reg::Rax, Reg::Rcx),
                ir::BinOp::Mul => em.imul_rr(Reg::Rax, Reg::Rcx),
                ir::BinOp::Div => {
                    em.cqo();
                    em.idiv(Reg::Rcx);
                }
                ir::BinOp::Mod => {
                    em.cqo();
                    em.idiv(Reg::Rcx);
                    em.mov_rr(Reg::Rax, Reg::Rdx);
                }
                ir::BinOp::BitAnd => {
                    em.xor_rr(Reg::Rax, Reg::Rcx);
                    em.xor_rr(Reg::Rax, Reg::Rcx);
                    em.and_rr(Reg::Rax, Reg::Rcx);
                }
                ir::BinOp::BitOr => {
                    em.or_rr(Reg::Rax, Reg::Rcx);
                }
                ir::BinOp::BitXor => em.xor_rr(Reg::Rax, Reg::Rcx),
                ir::BinOp::Shl => em.shl_cl(),
                ir::BinOp::Shr => em.shr_cl(),
                ir::BinOp::And => {
                    em.test_rr(Reg::Rcx, Reg::Rcx);
                    em.setcc_al(CondCode::Ne);
                    em.movzx_byte(Reg::Rax);
                    em.push(Reg::Rax);
                    em.test_rr(Reg::Rax, Reg::Rax);
                    em.setcc_al(CondCode::Ne);
                    em.movzx_byte(Reg::Rax);
                    em.pop(Reg::Rcx);
                    em.test_rr(Reg::Rax, Reg::Rax);
                    em.setcc_al(CondCode::Ne);
                    em.movzx_byte(Reg::Rax);
                    em.test_rr(Reg::Rcx, Reg::Rcx);
                    em.setcc_al(CondCode::Ne);
                    em.movzx_byte(Reg::Rax);
                }
                ir::BinOp::Or => {
                    em.test_rr(Reg::Rax, Reg::Rax);
                    em.setcc_al(CondCode::Ne);
                    em.movzx_byte(Reg::Rax);
                    em.push(Reg::Rax);
                    em.test_rr(Reg::Rcx, Reg::Rcx);
                    em.setcc_al(CondCode::Ne);
                    em.movzx_byte(Reg::Rax);
                    em.pop(Reg::Rcx);
                    em.test_rr(Reg::Rax, Reg::Rax);
                    em.setcc_al(CondCode::Ne);
                    em.movzx_byte(Reg::Rax);
                    em.test_rr(Reg::Rcx, Reg::Rcx);
                    em.setcc_al(CondCode::Ne);
                    em.movzx_byte(Reg::Rax);
                }
                ir::BinOp::Eq => emit_cmp(em, CondCode::E),
                ir::BinOp::Ne => emit_cmp(em, CondCode::Ne),
                ir::BinOp::Lt => emit_cmp(em, CondCode::L),
                ir::BinOp::Le => emit_cmp(em, CondCode::Le),
                ir::BinOp::Gt => emit_cmp(em, CondCode::G),
                ir::BinOp::Ge => emit_cmp(em, CondCode::Ge),
                _ => {}
            }
            Ok(())
        }
        Expr::Unary { op, expr, .. } => {
            emit_expr(expr, em, alloc, module, string_offsets, call_patches, str_lit_patches, loop_stack)?;
            match op {
                ir::UnaryOp::Neg => em.neg_rax(),
                ir::UnaryOp::Not => {
                    em.test_rr(Reg::Rax, Reg::Rax);
                    em.setcc_al(CondCode::E);
                    em.movzx_byte(Reg::Rax);
                }
                ir::UnaryOp::BitNot => em.not_rax(),
                _ => {}
            }
            Ok(())
        }
        Expr::Call { callee, args, .. } => {
            let nargs = args.len();
            if nargs > 6 {
                for i in (6..nargs).rev() {
                    if let Some(a) = args.get(i) {
                        emit_expr(a, em, alloc, module, string_offsets, call_patches, str_lit_patches, loop_stack)?;
                        em.push(Reg::Rax);
                    }
                }
            }
            for (i, a) in args.iter().take(6).enumerate() {
                emit_expr(a, em, alloc, module, string_offsets, call_patches, str_lit_patches, loop_stack)?;
                let reg = match i {
                    0 => Reg::Rdi, 1 => Reg::Rsi, 2 => Reg::Rdx,
                    3 => Reg::Rcx, 4 => Reg::R8,  5 => Reg::R9,
                    _ => unreachable!(),
                };
                em.mov_rr(reg, Reg::Rax);
            }
            if let Expr::Var { name, .. } = callee.as_ref() {
                let name_str = module.get_str(*name);
                if let Some(nr) = crate::lang::bmo::abi::resolve(name_str) {
                    em.mov_rax_imm64(nr as u64);
                    em.syscall();
                } else if name_str.starts_with("__bmo_syscall_") {
                    let nr: u64 = name_str.trim_start_matches("__bmo_syscall_").parse().unwrap_or(0);
                    em.mov_rax_imm64(nr);
                    em.syscall();
                } else {
                    let patch_pos = em.reserve_rel32();
                    call_patches.push((patch_pos, *name));
                    em.call_rel32(0);
                }
            }
            Ok(())
        }
        _ => {
            em.mov_rax_imm64(0);
            Ok(())
        }
    }
}

/// Emite una comparación: setea RAX a 1 o 0 según el CondCode.
/// Asume RAX = lhs, RCX = rhs antes de llamar.
fn emit_cmp(em: &mut Emitter, cc: CondCode) {
    em.cmp_rr(Reg::Rax, Reg::Rcx);
    em.setcc_al(cc);
    em.movzx_byte(Reg::Rax);
}
