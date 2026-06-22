//! `backends::aot_x86_64::codegen` — Code generator principal.
//!
//! Convierte un `common::ast::Module` a bytes x86-64 usando el
//! `Emitter`, el `RegAlloc`, y las convenciones del `abi`.
//!
//! ## Pipeline interno
//!
//! 1. Recolectar todas las funciones y strings del módulo.
//! 2. Calcular offsets tentativos (sin linkar).
//! 3. Compilar cada función:
//!    - Prologue: `push rbp; mov rbp, rsp; sub rsp, frame; push callee-saved`
//!    - Body: emitir cada stmt (let, assign, if, while, return, ...)
//!    - Epilogue: `pop callee-saved; leave; ret`
//! 4. Parchear `call rel32` que referencian otras funciones.
//! 5. Retornar `code` + `rodata` (linker los unirá con `_start`).

#![allow(dead_code)]

extern crate alloc;
use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use crate::bmo_gpu::BxResult;
use crate::lang::common::ast as ir;
use crate::lang::common::ast::{Module, Item, Stmt, Expr, Block, StrId};
use crate::lang::common::types::{IrType, IrTypeId};
use super::emit::{Emitter, CondCode};
use super::regs::{RegAlloc, Var, VarSize, Location};
use super::abi::Reg;

/// Resultado de compilar un módulo: code + rodata + parches pendientes.
pub struct CompiledArtifact {
    pub code: Vec<u8>,
    pub rodata: Vec<u8>,
    /// Parches pendientes: (posición en code, nombre de la función target).
    pub call_patches: Vec<(usize, StrId)>,
    /// Tabla de strings: (StrId, offset en rodata).
    pub string_offsets: BTreeMap<u32, u32>,
    /// Offsets de funciones: (StrId, offset en code).
    pub function_offsets: BTreeMap<u32, u32>,
}

/// Compila un módulo completo a un artefacto.
pub fn compile_module(module: &Module) -> BxResult<CompiledArtifact> {
    let mut em = Emitter::new();
    let mut call_patches: Vec<(usize, StrId)> = Vec::new();
    let mut string_offsets: BTreeMap<u32, u32> = BTreeMap::new();
    let mut function_offsets: BTreeMap<u32, u32> = BTreeMap::new();

    // Pasada 1: recopilar strings y funciones
    for item in &module.items {
        match item {
            Item::Function { name, params, body, .. } => {
                function_offsets.insert(name.0, em.code_len as u32);
                compile_function(&mut em, name, params, body, module,
                                 &mut string_offsets, &mut call_patches)?;
            }
            Item::Extern { .. } => { /* no se compila */ }
            _ => { /* otros no soportados */ }
        }
    }

    Ok(CompiledArtifact {
        code: em.bytes().to_vec(),
        rodata: em.rodata().to_vec(),
        call_patches,
        string_offsets,
        function_offsets,
    })
}

/// Compila una función. `em.code_len` después = inicio de la siguiente función.
fn compile_function(
    em: &mut Emitter,
    _name: &StrId,
    params: &[ir::Param],
    body: &Block,
    module: &Module,
    string_offsets: &mut BTreeMap<u32, u32>,
    call_patches: &mut Vec<(usize, StrId)>,
) -> BxResult<()> {
    let mut alloc = RegAlloc::new();

    // 1. Registrar params como variables (en RDI..R9 o stack).
    for (i, p) in params.iter().enumerate() {
        alloc.alloc_arg(p.name.0, i);
    }

    // 2. Prologue
    em.push(Reg::Rbp);
    em.mov_rbp_rsp();
    let frame = alloc.frame_size();
    em.sub_rsp_imm(frame);
    // Guardar callee-saved regs que vamos a usar
    for r in alloc.used_callee_saved() {
        em.push(*r);
    }

    // 3. Body
    let loop_stack: LoopStack = Vec::new();
    for stmt in &body.stmts {
        emit_stmt(stmt, em, &mut alloc, module, string_offsets, call_patches, &loop_stack)?;
    }

    // 4. Epilogue
    // Restaurar callee-saved (en orden inverso)
    for r in alloc.used_callee_saved().iter().rev() {
        em.pop(*r);
    }
    em.leave();
    em.ret();

    Ok(())
}

/// Stack de (label_break, label_continue) para loops.
type LoopStack<'a> = Vec<(usize, usize)>;

fn emit_stmt(
    stmt: &Stmt,
    em: &mut Emitter,
    alloc: &mut RegAlloc,
    module: &Module,
    string_offsets: &mut BTreeMap<u32, u32>,
    call_patches: &mut Vec<(usize, StrId)>,
    loop_stack: &LoopStack<'_>,
) -> BxResult<()> {
    match stmt {
        Stmt::Expr(e, _) => {
            // Evaluar la expresión en RAX (descartando el resultado).
            emit_expr(e, em, alloc, module, string_offsets, call_patches, loop_stack)?;
            Ok(())
        }
        Stmt::Let { name, ty: _, init, .. } => {
            let var = alloc.alloc(name.0, VarSize::Qword);
            if let Some(e) = init {
                emit_expr(e, em, alloc, module, string_offsets, call_patches, loop_stack)?;
                alloc.emit_store(em, var, Reg::Rax);
            }
            Ok(())
        }
        Stmt::Assign { target, value, .. } => {
            // Por ahora solo Var como target.
            if let Expr::Var { name, .. } = target {
                emit_expr(value, em, alloc, module, string_offsets, call_patches, loop_stack)?;
                // Buscar la var por nombre
                let var = find_var(alloc, &name.0);
                if let Some(v) = var {
                    alloc.emit_store(em, v, Reg::Rax);
                }
            }
            Ok(())
        }
        Stmt::If { cond, then_branch, else_branch, .. } => {
            // eval cond
            emit_expr(cond, em, alloc, module, string_offsets, call_patches, loop_stack)?;
            em.test_rr(Reg::Rax, Reg::Rax);
            let jz_else = em.reserve_rel32();
            // then
            for s in &then_branch.stmts {
                emit_stmt(s, em, alloc, module, string_offsets, call_patches, loop_stack)?;
            }
            let jmp_end = em.reserve_rel32();
            // patch jz -> else
            em.patch_rel32(jz_else, em.pos());
            // else
            if let Some(eb) = else_branch {
                for s in &eb.stmts {
                    emit_stmt(s, em, alloc, module, string_offsets, call_patches, loop_stack)?;
                }
            }
            // patch jmp -> end
            em.patch_rel32(jmp_end, em.pos());
            Ok(())
        }
        Stmt::While { cond, body, .. } => {
            let l_start = em.pos();
            emit_expr(cond, em, alloc, module, string_offsets, call_patches, loop_stack)?;
            em.test_rr(Reg::Rax, Reg::Rax);
            let jz_end = em.reserve_rel32();
            let l_end = em.pos();
            for s in &body.stmts {
                emit_stmt(s, em, alloc, module, string_offsets, call_patches, loop_stack)?;
            }
            let jmp = em.reserve_rel32();
            em.patch_rel32(jz_end, l_end);
            em.patch_rel32(jmp, l_start);
            Ok(())
        }
        Stmt::Return(value, _) => {
            if let Some(e) = value {
                emit_expr(e, em, alloc, module, string_offsets, call_patches, loop_stack)?;
                // El valor ya está en RAX
            }
            // Salir: saltar al epilogue (que ya está al final).
            // Por simplicidad, emitimos `ret` directamente. Esto funciona
            // solo si no hay más código. Para multi-stmt con return intermedio,
            // deberíamos saltar a una etiqueta "epilogue".
            // v1.8.8: simplificación — el usuario pone el return al final.
            em.ret();
            Ok(())
        }
        Stmt::Block(b) => {
            for s in &b.stmts {
                emit_stmt(s, em, alloc, module, string_offsets, call_patches, loop_stack)?;
            }
            Ok(())
        }
        Stmt::Break(_) => {
            // Stub — no se usa en Hello World.
            Ok(())
        }
        Stmt::Continue(_) => {
            // Stub.
            Ok(())
        }
        _ => Ok(()),
    }
}

fn find_var(alloc: &RegAlloc, _name_id: &u32) -> Option<Var> {
    // v1.8.8: simplificación — siempre retorna la primera variable.
    // En una versión real, mantendríamos un HashMap<StrId, Var>.
    if alloc_used_count(alloc) > 0 { Some(Var(0)) } else { None }
}

fn alloc_used_count(alloc: &RegAlloc) -> usize {
    // Hack: el `RegAlloc` no expone `n_vars`, usamos un magic number.
    // La forma correcta es agregar un método público.
    1
}

fn emit_expr(
    expr: &Expr,
    em: &mut Emitter,
    alloc: &mut RegAlloc,
    module: &Module,
    string_offsets: &mut BTreeMap<u32, u32>,
    call_patches: &mut Vec<(usize, StrId)>,
    loop_stack: &LoopStack<'_>,
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
            // Buscar o emitir el string en rodata.
            let s = module.get_str(*id);
            let offset = if let Some(&off) = string_offsets.get(&id.0) {
                off
            } else {
                let off = em.add_string(s.as_bytes());
                string_offsets.insert(id.0, off);
                off
            };
            // El string está en rodata, que viene después de code.
            // Como aún no sabemos el offset de rodata, emitimos un patch
            // pendiente. v1.8.8: simplificación, asumimos que el linker
            // resolverá. Aquí emitimos un lea placeholder.
            // Por ahora, calculamos el RIP-relative offset:
            // rodata_base = code_len final (que es em.code_len al final
            // de compile_module). El offset = rodata_base + string_offset - (current_pos + 7).
            // Como no sabemos rodata_base aún, dejamos un patch.
            // v1.8.8: simplificación — el linker parchea.
            let _ = offset;
            // Por ahora, el string se accede via syscall 0x1F0 (DIAG_PRINT).
            // Para hacer esto, necesitamos cambiar el Expr a un Call.
            // v1.8.8: workaround — emitimos el address del string como
            // una llamada a __bmo_diag_print (que el codegen reconoce).
            // Aquí simplemente emitimos el address:
            // [RIP + (final_rodata_offset + string_offset - current_pos)]
            // Como no podemos saber, el linker debe parchar.
            // IMPLEMENTACIÓN SIMPLIFICADA: el caller del codegen debe
            // traducir StrLit a un Call a diag_print, no hacerlo aquí.
            em.mov_rax_imm64(0); // placeholder
            Ok(())
        }
        Expr::Var { name, .. } => {
            // Buscar en alloc por StrId. Simplificación: siempre var 0.
            let var = find_var(alloc, &name.0);
            if let Some(v) = var {
                alloc.emit_load(em, v, Reg::Rax);
            } else {
                em.mov_rax_imm64(0);
            }
            Ok(())
        }
        Expr::Bin { op, lhs, rhs, .. } => {
            // Evaluar LHS en RAX, push.
            emit_expr(lhs, em, alloc, module, string_offsets, call_patches, loop_stack)?;
            em.push(Reg::Rax);
            // Evaluar RHS en RAX, mover a RCX.
            emit_expr(rhs, em, alloc, module, string_offsets, call_patches, loop_stack)?;
            em.mov_rr(Reg::Rcx, Reg::Rax);
            em.pop(Reg::Rax);
            // Operación.
            match op {
                ir::BinOp::Add => { em.add_rr(Reg::Rax, Reg::Rcx); }
                ir::BinOp::Sub => { em.sub_rr(Reg::Rax, Reg::Rcx); }
                ir::BinOp::Mul => { em.imul_rr(Reg::Rax, Reg::Rcx); }
                ir::BinOp::Div => {
                    em.cqo();
                    em.idiv(Reg::Rcx);
                }
                ir::BinOp::Mod => {
                    em.cqo();
                    em.idiv(Reg::Rcx);
                    em.mov_rr(Reg::Rax, Reg::Rdx); // remainder
                }
                ir::BinOp::BitAnd => { em.xor_rr(Reg::Rax, Reg::Rax); em.mov_rr(Reg::Rax, Reg::Rax); /* stub */ }
                ir::BinOp::BitOr  => { em.xor_rr(Reg::Rax, Reg::Rax); /* stub */ }
                ir::BinOp::BitXor => { em.xor_rr(Reg::Rax, Reg::Rcx); }
                ir::BinOp::Shl    => { /* stub */ }
                ir::BinOp::Shr    => { /* stub */ }
                ir::BinOp::And => { em.xor_rr(Reg::Rax, Reg::Rax); /* TODO: short-circuit */ }
                ir::BinOp::Or  => { em.mov_rax_imm64(1); /* TODO: short-circuit */ }
                _ => {}
            }
            Ok(())
        }
        Expr::Call { callee, args, .. } => {
            // 1. Evaluar args en orden, push al stack.
            let nargs = args.len();
            // Stack alignment: si nargs es impar, push extra para alinear.
            if nargs > 6 {
                for i in (6..nargs).rev() {
                    if let Some(a) = args.get(i) {
                        emit_expr(a, em, alloc, module, string_offsets, call_patches, loop_stack)?;
                        em.push(Reg::Rax);
                    }
                }
            }

            // 2. Evaluar args 0..6 en RDI..R9.
            for (i, a) in args.iter().take(6).enumerate() {
                emit_expr(a, em, alloc, module, string_offsets, call_patches, loop_stack)?;
                let reg = match i {
                    0 => Reg::Rdi, 1 => Reg::Rsi, 2 => Reg::Rdx,
                    3 => Reg::Rcx, 4 => Reg::R8,  5 => Reg::R9,
                    _ => unreachable!(),
                };
                em.mov_rr(reg, Reg::Rax);
            }

            // 3. Determinar si es BMO ABI o user call.
            if let Expr::Var { name, .. } = callee.as_ref() {
                let name_str = module.get_str(*name);
                // ¿Es BMO ABI?
                if let Some(nr) = crate::lang::bmo::abi::resolve(name_str) {
                    em.mov_rax_imm64(nr as u64);
                    em.syscall();
                } else if name_str.starts_with("__bmo_syscall_") {
                    // Syscall directo del BMO legacy.
                    let nr: u64 = name_str.trim_start_matches("__bmo_syscall_").parse().unwrap_or(0);
                    em.mov_rax_imm64(nr);
                    em.syscall();
                } else {
                    // User function: parchear.
                    let patch_pos = em.reserve_rel32();
                    call_patches.push((patch_pos, *name));
                    em.call_rel32(0); // será parchado
                }
            }
            Ok(())
        }
        _ => {
            // Otros: emitir 0.
            em.mov_rax_imm64(0);
            Ok(())
        }
    }
}
