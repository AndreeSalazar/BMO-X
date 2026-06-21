extern crate alloc;
use alloc::vec::Vec;

use crate::bmo_gpu::BxResult;
use super::emit::{Emitter, self};
use super::parser::ast::{Ast, Stmt, Expr, BinOp, UnaryOp};

pub struct Codegen {
    current_module: alloc::vec::Vec<alloc::string::String>,
}

impl Codegen {
    pub fn new() -> Self {
        Self { current_module: Vec::new() }
    }

    pub fn emit(&mut self, ast: &Ast) -> BxResult<Vec<u8>> {
        let mut em = Emitter::new();
        for item in &ast.items {
            emit_stmt(&mut em, item)?;
        }
        em.emit_byte(emit::HALT);
        Ok(em.into_code())
    }
}

impl Default for Codegen {
    fn default() -> Self { Self::new() }
}

fn emit_stmt(em: &mut Emitter, stmt: &Stmt) -> BxResult<()> {
    match stmt {
        Stmt::Let { name, value, .. } => {
            if let Some(val) = value {
                emit_expr(em, val)?;
            } else {
                em.emit_byte(emit::PUSH_IMM64);
                em.emit_imm64(0);
            }
            em.emit_local_index(emit::STORE_LOCAL, name);
        }
        Stmt::ExprStmt(expr) => {
            emit_expr(em, expr)?;
        }
        Stmt::Return(expr) => {
            if let Some(val) = expr {
                emit_expr(em, val)?;
            }
            em.emit_byte(emit::RET);
        }
        Stmt::If { cond, then_body, else_body } => {
            emit_expr(em, cond)?;
            let jump_if_false_off = em.current_offset();
            em.emit_byte(emit::JMP_IF_FALSE);
            em.emit_imm16(0);
            for s in then_body {
                emit_stmt(em, s)?;
            }
            if let Some(else_stmts) = else_body {
                let end_off = em.current_offset();
                em.emit_byte(emit::JMP);
                em.emit_imm16(0);
                let then_end = (em.current_offset() - jump_if_false_off - 3) as i16;
                em.patch_jump(jump_if_false_off + 1, then_end);
                for s in else_stmts {
                    emit_stmt(em, s)?;
                }
                let full_end = (em.current_offset() - end_off - 3) as i16;
                em.patch_jump(end_off + 1, full_end);
            } else {
                let offset = (em.current_offset() - jump_if_false_off - 3) as i16;
                em.patch_jump(jump_if_false_off + 1, offset);
            }
        }
        Stmt::While { cond, body } => {
            let start = em.current_offset();
            emit_expr(em, cond)?;
            let jump_off = em.current_offset();
            em.emit_byte(emit::JMP_IF_FALSE);
            em.emit_imm16(0);
            for s in body {
                emit_stmt(em, s)?;
            }
            em.emit_byte(emit::JMP);
            let back = -(((em.current_offset() - start) + 2) as i16);
            em.emit_imm16(back);
            let fwd = (em.current_offset() - jump_off - 3) as i16;
            em.patch_jump(jump_off + 1, fwd);
        }
        Stmt::Block(stmts) => {
            for s in stmts {
                emit_stmt(em, s)?;
            }
        }
        Stmt::Assign(name, expr) => {
            emit_expr(em, expr)?;
            em.emit_local_index(emit::STORE_LOCAL, name);
        }
        Stmt::Syscall { nr, args } => {
            for a in args {
                emit_expr(em, a)?;
            }
            em.emit_byte(emit::SYS_CALL);
            em.emit_byte(*nr as u8);
        }
        Stmt::Emit(bytes) => {
            for b in bytes {
                em.emit_byte(*b);
            }
        }
        _ => {}
    }
    Ok(())
}

fn emit_expr(em: &mut Emitter, expr: &Expr) -> BxResult<()> {
    match expr {
        Expr::LitInt(n) => {
            em.emit_byte(emit::PUSH_IMM64);
            em.emit_imm64(*n);
        }
        Expr::LitBool(b) => {
            em.emit_byte(emit::PUSH_IMM64);
            em.emit_imm64(if *b { 1 } else { 0 });
        }
        Expr::Binary(op, left, right) => {
            emit_expr(em, left)?;
            emit_expr(em, right)?;
            emit_binop(em, *op);
        }
        Expr::Unary(op, inner) => {
            emit_expr(em, inner)?;
            match op {
                UnaryOp::Neg => em.emit_byte(emit::NEG),
                UnaryOp::Not => em.emit_byte(emit::NOT),
                _ => {}
            }
        }
        Expr::Ident(name) => {
            em.emit_local_index(emit::LOAD_LOCAL, name);
        }
        Expr::Call(name, args) => {
            for a in args {
                emit_expr(em, a)?;
            }
            em.emit_local_index(emit::CALL, name);
        }
        Expr::Syscall(nr, args) => {
            for a in args {
                emit_expr(em, a)?;
            }
            em.emit_byte(emit::SYS_CALL);
            em.emit_byte(*nr as u8);
        }
        Expr::Block(stmts) => {
            for s in stmts {
                emit_stmt(em, s)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn emit_binop(em: &mut Emitter, op: BinOp) {
    let code = match op {
        BinOp::Add => emit::ADD,
        BinOp::Sub => emit::SUB,
        BinOp::Mul => emit::MUL,
        BinOp::Div => emit::DIV,
        BinOp::Mod => emit::MOD,
        BinOp::And => emit::AND,
        BinOp::Or => emit::OR,
        BinOp::Xor => emit::XOR,
        BinOp::Shl => emit::SHL,
        BinOp::Shr => emit::SHR,
        BinOp::Eq => emit::EQ,
        BinOp::Ne => emit::NE,
        BinOp::Lt => emit::LT,
        BinOp::Gt => emit::GT,
        BinOp::Le => emit::LE,
        BinOp::Ge => emit::GE,
        BinOp::Land => emit::LAND,
        BinOp::Lor => emit::LOR,
    };
    em.emit_byte(code);
}
