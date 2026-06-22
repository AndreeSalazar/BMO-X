//! `backends::aot_x86_64::codegen` — Code generator que produce un `BmoObject`.
//!
//! v1.8.8 v2.0: en lugar de emitir `CompiledArtifact` (code + rodata + patches),
//! el codegen ahora emite un `BmoObject` completo con secciones, símbolos
//! y relocalizaciones formales. El linker v2.0 los une en un BEF final.
//!
//! ## Pipeline
//!
//! 1. compile_module() crea un BmoObjectBuilder.
//! 2. Por cada `Item::Function`, crea una sección .text con su código,
//!    registra un símbolo, y emite parches como Relocations.
//! 3. Por cada `Expr::StrLit`, agrega el string a .rodata y emite
//!    una `Relocation::RipRel32` para el LEA.
//! 4. Por cada `Expr::Call` a un símbolo no resuelto, emite
//!    `Relocation::Rel32` (call) o registra el import (BMO ABI).
//! 5. Al final, devuelve el `BmoObject` listo para el linker.

#![allow(dead_code)]

extern crate alloc;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use crate::bmo_gpu::BxResult;
use crate::lang::common::ast as ir;
use crate::lang::common::ast::{Module, Item, Stmt, Expr, Block, StrId};
use crate::lang::common::types::{IrType, IrTypeId};
use super::emit::{Emitter, CondCode};
use super::regs::{RegAlloc, Var, VarSize};
use super::abi::Reg;
use crate::lang::bef::{
    BmoObject, BmoObjectBuilder, ObjectSection, ObjectArch, SectionKind,
    SectionFlags, Symbol, SymbolBinding, SymbolType, Relocation, RelocationKind,
};

/// Compila un módulo a un BmoObject (v2.0).
pub fn compile_module(module: &Module) -> BxResult<BmoObject> {
    let mut b = BmoObjectBuilder::new(module.name.clone());

    // Pre-registrar strings del module.
    // (El module tiene `get_str` pero no iter; por ahora, los strings
    // se registran cuando se usan en Expr::StrLit.)

    let mut str_to_offset: BTreeMap<u32, u32> = BTreeMap::new();
    let mut str_id_to_name: BTreeMap<u32, String> = BTreeMap::new();

    // Pasada 1: crear strings que aparecerán en el código.
    for item in &module.items {
        if let Item::Function { body, .. } = item {
            // Por ahora no pre-coleccionamos strings.
            let _ = body;
        }
    }

    // Crear sección .text.
    let text_idx = b.create_section(SectionKind::Text, ".text");
    let rodata_idx = b.create_section(SectionKind::Rodata, ".rodata");
    let reloc_idx = b.create_section(SectionKind::Reloc, ".reloc");
    let symtab_idx = b.create_section(SectionKind::Symtab, ".symtab");
    let strtab_idx = b.create_section(SectionKind::Strtab, ".strtab");
    let meta_idx = b.create_section(SectionKind::Meta, ".meta");
    let imports_idx = b.create_section(SectionKind::Imports, ".imports");
    let _ = (reloc_idx, symtab_idx, strtab_idx, meta_idx, imports_idx);

    // Importar _start (entry point del runtime).
    b.import("_start", Some("c_min::start"));
    // Importar _exit (llamado por _start al final).
    b.import("_exit", Some("c_min::exit"));

    // Compilar cada función.
    for item in &module.items {
        if let Item::Function { name, params, body, .. } = item {
            let func_offset = b.obj.sections[text_idx].data.len() as u32;
            let fn_name = module.get_str(*name).to_string();
            b.define(&fn_name, text_idx, func_offset, SymbolType::Function);
            b.register_str(name.0, &fn_name);
            compile_function(
                &mut b.obj, &mut b.str_cache, &mut str_to_offset,
                text_idx, rodata_idx, reloc_idx,
                name, params, body, module,
            )?;
        }
    }

    // Serializar metadata ABI en .meta.
    b.append_to_section(meta_idx, &b.obj.abi_version.0.to_le_bytes());
    b.append_to_section(meta_idx, &b.obj.abi_version.1.to_le_bytes());
    b.append_to_section(meta_idx, &b.obj.capabilities.to_le_bytes());

    Ok(b.build())
}

/// Recolecta los StrId de los Expr::StrLit en un body.
fn collect_strings(body: &Block, out: &mut BTreeMap<u32, String>) {
    for s in &body.stmts {
        collect_strings_stmt(s, out);
    }
}

fn collect_strings_stmt(stmt: &Stmt, out: &mut BTreeMap<u32, String>) {
    match stmt {
        Stmt::Expr(e, _) | Stmt::Return(Some(e), _) => collect_strings_expr(e, out),
        Stmt::Let { init: Some(e), .. } | Stmt::Assign { value: e, .. } => collect_strings_expr(e, out),
        Stmt::If { cond, then_branch, else_branch, .. } => {
            collect_strings_expr(cond, out);
            collect_strings(then_branch, out);
            if let Some(b) = else_branch { collect_strings(b, out); }
        }
        Stmt::While { cond, body, .. } => {
            collect_strings_expr(cond, out);
            collect_strings(body, out);
        }
        Stmt::Block(b) => collect_strings(b, out),
        _ => {}
    }
}

fn collect_strings_expr(expr: &Expr, out: &mut BTreeMap<u32, String>) {
    match expr {
        Expr::StrLit { id, .. } => { out.insert(id.0, alloc::format!("str_{}", id.0)); }
        Expr::Bin { lhs, rhs, .. } => {
            collect_strings_expr(lhs, out);
            collect_strings_expr(rhs, out);
        }
        Expr::Unary { expr, .. } => collect_strings_expr(expr, out),
        Expr::Call { callee, args, .. } => {
            collect_strings_expr(callee, out);
            for a in args { collect_strings_expr(a, out); }
        }
        _ => {}
    }
}

fn compile_function(
    obj: &mut BmoObject,
    str_cache: &mut BTreeMap<u32, String>,
    str_to_offset: &mut BTreeMap<u32, u32>,
    text_idx: usize,
    rodata_idx: usize,
    reloc_idx: usize,
    _name: &StrId,
    params: &[ir::Param],
    body: &Block,
    module: &Module,
) -> BxResult<()> {
    let mut em = Emitter::new();
    let mut alloc = RegAlloc::new();

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
    let text_offset = obj.sections[text_idx].data.len();
    for stmt in &body.stmts {
        emit_stmt(stmt, &mut em, &mut alloc, module, obj, str_cache,
                  str_to_offset, text_idx, rodata_idx, reloc_idx, text_offset, &loop_stack)?;
    }

    // Epilogue.
    for r in alloc.used_callee_saved().iter().rev() {
        em.pop(*r);
    }
    em.leave();
    em.ret();

    // Append a la sección .text.
    obj.sections[text_idx].data.extend_from_slice(em.bytes());

    Ok(())
}

type LoopStack = Vec<(usize, usize)>;

fn emit_stmt(
    stmt: &Stmt,
    em: &mut Emitter,
    alloc: &mut RegAlloc,
    module: &Module,
    obj: &mut BmoObject,
    str_cache: &mut BTreeMap<u32, String>,
    str_to_offset: &mut BTreeMap<u32, u32>,
    text_idx: usize,
    rodata_idx: usize,
    reloc_idx: usize,
    text_offset: usize,
    loop_stack: &LoopStack,
) -> BxResult<()> {
    match stmt {
        Stmt::Expr(e, _) => {
            emit_expr(e, em, alloc, module, obj, str_cache, str_to_offset,
                      text_idx, rodata_idx, reloc_idx, text_offset, loop_stack)?;
            Ok(())
        }
        Stmt::Let { name, ty: _, init, .. } => {
            let var = alloc.alloc(name.0, VarSize::Qword);
            if let Some(e) = init {
                emit_expr(e, em, alloc, module, obj, str_cache, str_to_offset,
                          text_idx, rodata_idx, reloc_idx, text_offset, loop_stack)?;
                alloc.emit_store(em, var, Reg::Rax);
            }
            Ok(())
        }
        Stmt::Assign { target, value, .. } => {
            if let Expr::Var { name, .. } = target {
                emit_expr(value, em, alloc, module, obj, str_cache, str_to_offset,
                          text_idx, rodata_idx, reloc_idx, text_offset, loop_stack)?;
                if let Some(v) = alloc.find_by_name(name.0) {
                    alloc.emit_store(em, v, Reg::Rax);
                }
            }
            Ok(())
        }
        Stmt::If { cond, then_branch, else_branch, .. } => {
            emit_expr(cond, em, alloc, module, obj, str_cache, str_to_offset,
                      text_idx, rodata_idx, reloc_idx, text_offset, loop_stack)?;
            em.test_rr(Reg::Rax, Reg::Rax);
            let jz_else = em.reserve_rel32();
            for s in &then_branch.stmts {
                emit_stmt(s, em, alloc, module, obj, str_cache, str_to_offset,
                          text_idx, rodata_idx, reloc_idx, text_offset, loop_stack)?;
            }
            let jmp_end = em.reserve_rel32();
            em.patch_rel32(jz_else, em.pos());
            if let Some(eb) = else_branch {
                for s in &eb.stmts {
                    emit_stmt(s, em, alloc, module, obj, str_cache, str_to_offset,
                              text_idx, rodata_idx, reloc_idx, text_offset, loop_stack)?;
                }
            }
            em.patch_rel32(jmp_end, em.pos());
            Ok(())
        }
        Stmt::While { cond, body, .. } => {
            let l_start = em.pos();
            emit_expr(cond, em, alloc, module, obj, str_cache, str_to_offset,
                      text_idx, rodata_idx, reloc_idx, text_offset, loop_stack)?;
            em.test_rr(Reg::Rax, Reg::Rax);
            let jz_end = em.reserve_rel32();
            let l_end = em.pos();
            let l_break_pos = em.reserve_rel32();
            let mut nested_stack = loop_stack.clone();
            nested_stack.push((l_break_pos, l_start));
            for s in &body.stmts {
                emit_stmt(s, em, alloc, module, obj, str_cache, str_to_offset,
                          text_idx, rodata_idx, reloc_idx, text_offset, &nested_stack)?;
            }
            let jmp = em.reserve_rel32();
            em.patch_rel32(jz_end, l_end);
            em.patch_rel32(l_break_pos, em.pos());
            em.patch_rel32(jmp, l_start);
            Ok(())
        }
        Stmt::Return(value, _) => {
            if let Some(e) = value {
                emit_expr(e, em, alloc, module, obj, str_cache, str_to_offset,
                          text_idx, rodata_idx, reloc_idx, text_offset, loop_stack)?;
            }
            em.ret();
            Ok(())
        }
        Stmt::Block(b) => {
            for s in &b.stmts {
                emit_stmt(s, em, alloc, module, obj, str_cache, str_to_offset,
                          text_idx, rodata_idx, reloc_idx, text_offset, loop_stack)?;
            }
            Ok(())
        }
        Stmt::Break(_) => {
            if let Some(&(l_break, _)) = loop_stack.last() {
                em.jmp_rel32(0);
                let pos = em.code_len - 4;
                let target = obj.sections[text_idx].data.len() + l_break;
                let rel = (target as isize - (obj.sections[text_idx].data.len() + pos) as isize - 4) as i32;
                let cm = em.code_mut();
                cm[pos..pos+4].copy_from_slice(&rel.to_le_bytes());
            }
            Ok(())
        }
        Stmt::Continue(_) => {
            if let Some(&(_, l_continue)) = loop_stack.last() {
                em.jmp_rel32(0);
                let pos = em.code_len - 4;
                let target = obj.sections[text_idx].data.len() + l_continue;
                let rel = (target as isize - (obj.sections[text_idx].data.len() + pos) as isize - 4) as i32;
                let cm = em.code_mut();
                cm[pos..pos+4].copy_from_slice(&rel.to_le_bytes());
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
    obj: &mut BmoObject,
    str_cache: &mut BTreeMap<u32, String>,
    str_to_offset: &mut BTreeMap<u32, u32>,
    text_idx: usize,
    rodata_idx: usize,
    reloc_idx: usize,
    text_offset: usize,
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
            let s = module.get_str(*id);
            let str_offset = if let Some(&off) = str_to_offset.get(&id.0) {
                off
            } else {
                let off = obj.sections[rodata_idx].data.len() as u32;
                obj.sections[rodata_idx].data.extend_from_slice(s.as_bytes());
                obj.sections[rodata_idx].data.push(0);
                str_to_offset.insert(id.0, off);
                off
            };
            let str_name = alloc::format!(".L.str.{}", id.0);
            str_cache.insert(id.0, str_name.clone());
            obj.define_symbol(&str_name, rodata_idx, str_offset, SymbolType::Object);
            em.rex(true, Reg::Rax, Reg::Rax, Reg::Rax);
            em.cb(0x8D);
            em.modrm_rip(Reg::Rax);
            let disp_pos = text_offset + em.code_len - 4;
            obj.relocations.push(Relocation {
                kind: RelocationKind::RipRel32,
                section: text_idx,
                offset: disp_pos as u32,
                symbol: str_name,
                addend: 0,
                size: 4,
            });
            em.cs(&[0; 4]);
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
            emit_expr(lhs, em, alloc, module, obj, str_cache, str_to_offset,
                      text_idx, rodata_idx, reloc_idx, text_offset, loop_stack)?;
            em.push(Reg::Rax);
            emit_expr(rhs, em, alloc, module, obj, str_cache, str_to_offset,
                      text_idx, rodata_idx, reloc_idx, text_offset, loop_stack)?;
            em.mov_rr(Reg::Rcx, Reg::Rax);
            em.pop(Reg::Rax);

            match op {
                ir::BinOp::Add => em.add_rr(Reg::Rax, Reg::Rcx),
                ir::BinOp::Sub => em.sub_rr(Reg::Rax, Reg::Rcx),
                ir::BinOp::Mul => em.imul_rr(Reg::Rax, Reg::Rcx),
                ir::BinOp::Div => { em.cqo(); em.idiv(Reg::Rcx); }
                ir::BinOp::Mod => { em.cqo(); em.idiv(Reg::Rcx); em.mov_rr(Reg::Rax, Reg::Rdx); }
                ir::BinOp::BitAnd => { em.xor_rr(Reg::Rax, Reg::Rcx); em.xor_rr(Reg::Rax, Reg::Rcx); em.and_rr(Reg::Rax, Reg::Rcx); }
                ir::BinOp::BitOr => { em.or_rr(Reg::Rax, Reg::Rcx); }
                ir::BinOp::BitXor => em.xor_rr(Reg::Rax, Reg::Rcx),
                ir::BinOp::Shl => em.shl_cl(),
                ir::BinOp::Shr => em.shr_cl(),
                ir::BinOp::And => {
                    em.test_rr(Reg::Rcx, Reg::Rcx); em.setcc_al(CondCode::Ne); em.movzx_byte(Reg::Rax);
                    em.push(Reg::Rax);
                    em.test_rr(Reg::Rax, Reg::Rax); em.setcc_al(CondCode::Ne); em.movzx_byte(Reg::Rax);
                    em.pop(Reg::Rcx);
                    em.test_rr(Reg::Rax, Reg::Rax); em.setcc_al(CondCode::Ne); em.movzx_byte(Reg::Rax);
                    em.test_rr(Reg::Rcx, Reg::Rcx); em.setcc_al(CondCode::Ne); em.movzx_byte(Reg::Rax);
                }
                ir::BinOp::Or => {
                    em.test_rr(Reg::Rax, Reg::Rax); em.setcc_al(CondCode::Ne); em.movzx_byte(Reg::Rax);
                    em.push(Reg::Rax);
                    em.test_rr(Reg::Rcx, Reg::Rcx); em.setcc_al(CondCode::Ne); em.movzx_byte(Reg::Rax);
                    em.pop(Reg::Rcx);
                    em.test_rr(Reg::Rax, Reg::Rax); em.setcc_al(CondCode::Ne); em.movzx_byte(Reg::Rax);
                    em.test_rr(Reg::Rcx, Reg::Rcx); em.setcc_al(CondCode::Ne); em.movzx_byte(Reg::Rax);
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
            emit_expr(expr, em, alloc, module, obj, str_cache, str_to_offset,
                      text_idx, rodata_idx, reloc_idx, text_offset, loop_stack)?;
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
                        emit_expr(a, em, alloc, module, obj, str_cache, str_to_offset,
                                  text_idx, rodata_idx, reloc_idx, text_offset, loop_stack)?;
                        em.push(Reg::Rax);
                    }
                }
            }
            for (i, a) in args.iter().take(6).enumerate() {
                emit_expr(a, em, alloc, module, obj, str_cache, str_to_offset,
                          text_idx, rodata_idx, reloc_idx, text_offset, loop_stack)?;
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
                    let name_owned = name_str.to_string();
                    obj.import_symbol(&name_owned, None);
                    em.call_rel32(0);
                    let disp_pos = text_offset + em.code_len - 4;
                    obj.relocations.push(Relocation {
                        kind: RelocationKind::Rel32,
                        section: text_idx,
                        offset: disp_pos as u32,
                        symbol: name_owned,
                        addend: 0,
                        size: 4,
                    });
                }
            }
            Ok(())
        }
        _ => { em.mov_rax_imm64(0); Ok(()) }
    }
}

fn emit_cmp(em: &mut Emitter, cc: CondCode) {
    em.cmp_rr(Reg::Rax, Reg::Rcx);
    em.setcc_al(cc);
    em.movzx_byte(Reg::Rax);
}
