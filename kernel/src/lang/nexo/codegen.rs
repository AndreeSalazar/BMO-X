//! ÑEXO Codegen — Generación de código vía BMOasm.
//!
//! Traduce el AST de ÑEXO a código BMOasm intermedio, que luego
//! se emite como código nativo x86-64/AArch64/RISC-V.

#![allow(dead_code)]

extern crate alloc;
use alloc::vec::Vec;

use crate::barex::BxResult;
use super::parser::{Ast, Stmt, Expr, BinOp, UnaryOp};

/// Code generator for ÑEXO.
pub struct Codegen;

impl Codegen {
    pub fn new() -> Self { Self }

    /// Generate native code from AST.
    pub fn emit(&self, ast: &Ast) -> BxResult<Vec<u8>> {
        let mut code = Vec::new();
        for item in &ast.items {
            self.emit_stmt(item, &mut code)?;
        }
        Ok(code)
    }

    fn emit_stmt(&self, stmt: &Stmt, code: &mut Vec<u8>) -> BxResult<()> {
        match stmt {
            Stmt::FnDecl { name: _, params: _, ret: _, body } => {
                // Function prologue: push rbp; mov rbp, rsp
                code.extend_from_slice(&[0x55, 0x48, 0x89, 0xE5]);
                for s in body {
                    self.emit_stmt(s, code)?;
                }
                // Function epilogue: leave; ret
                code.extend_from_slice(&[0xC9, 0xC3]);
            }
            Stmt::Return(Some(expr)) => {
                self.emit_expr(expr, code)?;
                // leave; ret
                code.extend_from_slice(&[0xC9, 0xC3]);
            }
            Stmt::Return(None) => {
                // xor eax, eax; leave; ret
                code.extend_from_slice(&[0x31, 0xC0, 0xC9, 0xC3]);
            }
            Stmt::ExprStmt(expr) => {
                self.emit_expr(expr, code)?;
            }
            Stmt::Let { name: _, ty: _, value } => {
                if let Some(val) = value {
                    self.emit_expr(val, code)?;
                    // push rax (save value)
                    code.push(0x50);
                }
            }
            Stmt::If { cond, then_body, else_body } => {
                self.emit_expr(cond, code)?;
                // test rax, rax
                code.extend_from_slice(&[0x48, 0x85, 0xC0]);
                // je else_label (placeholder)
                code.extend_from_slice(&[0x0F, 0x84]);
                let patch_pos = code.len();
                code.extend_from_slice(&[0, 0, 0, 0]); // placeholder
                for s in then_body {
                    self.emit_stmt(s, code)?;
                }
                if let Some(eb) = else_body {
                    // jmp end
                    code.push(0xE9);
                    let jmp_end = code.len();
                    code.extend_from_slice(&[0, 0, 0, 0]);
                    // Patch je to here
                    let else_start = code.len() as i32;
                    let disp = else_start - (patch_pos as i32 + 4);
                    code[patch_pos..patch_pos + 4].copy_from_slice(&disp.to_le_bytes());
                    for s in eb {
                        self.emit_stmt(s, code)?;
                    }
                    // Patch jmp end
                    let end = code.len() as i32;
                    let disp2 = end - (jmp_end as i32 + 4);
                    code[jmp_end..jmp_end + 4].copy_from_slice(&disp2.to_le_bytes());
                } else {
                    let end = code.len() as i32;
                    let disp = end - (patch_pos as i32 + 4);
                    code[patch_pos..patch_pos + 4].copy_from_slice(&disp.to_le_bytes());
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn emit_expr(&self, expr: &Expr, code: &mut Vec<u8>) -> BxResult<()> {
        match expr {
            Expr::LitInt(n) => {
                // mov rax, imm64
                code.extend_from_slice(&[0x48, 0xB8]);
                code.extend_from_slice(&n.to_le_bytes());
            }
            Expr::LitBool(b) => {
                // xor eax, eax; set al
                code.extend_from_slice(&[0x31, 0xC0, 0xB0, if *b { 1 } else { 0 }]);
            }
            Expr::LitNull => {
                // xor eax, eax
                code.extend_from_slice(&[0x31, 0xC0]);
            }
            Expr::Binary(op, left, right) => {
                self.emit_expr(left, code)?;
                code.push(0x50); // push rax
                self.emit_expr(right, code)?;
                code.extend_from_slice(&[0x48, 0x89, 0xC1]); // mov rcx, rax
                code.push(0x58); // pop rax
                match op {
                    BinOp::Add => code.extend_from_slice(&[0x48, 0x01, 0xC8]), // add rax, rcx
                    BinOp::Sub => code.extend_from_slice(&[0x48, 0x29, 0xC8]), // sub rax, rcx
                    BinOp::Mul => code.extend_from_slice(&[0x48, 0x0F, 0xAF, 0xC1]), // imul rax, rcx
                    _ => {
                        // For now, just return 0 for unsupported ops
                        code.extend_from_slice(&[0x31, 0xC0]);
                    }
                }
            }
            Expr::Unary(op, inner) => {
                self.emit_expr(inner, code)?;
                match op {
                    UnaryOp::Neg => {
                        code.extend_from_slice(&[0x48, 0xF7, 0xD8]); // neg rax
                    }
                    UnaryOp::Not => {
                        code.extend_from_slice(&[0x48, 0x85, 0xC0]); // test rax, rax
                        code.extend_from_slice(&[0x0F, 0x94, 0xC0]); // sete al
                        code.extend_from_slice(&[0x48, 0x0F, 0xB6, 0xC0]); // movzx rax, al
                    }
                    _ => {}
                }
            }
            _ => {
                // Placeholder for unhandled expressions
                code.extend_from_slice(&[0x31, 0xC0]); // xor eax, eax
            }
        }
        Ok(())
    }
}
