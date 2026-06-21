//! BMO AOT Compiler — x86-64 native code generation.
//!
//! Compiles BMO AST directly to x86-64 machine code using a stack-machine model.
//! Output is raw x86-64 bytes that can be executed via iretq.

#![allow(dead_code)]

use super::parser::ast::{Ast, Stmt, Expr, BinOp, UnaryOp};

const CODE_BUF_SIZE: usize = 64 * 1024;
const MAX_LOCALS: usize = 64;

pub struct NativeCompiler {
    code: [u8; CODE_BUF_SIZE],
    code_len: usize,
    locals: LocalTable,
}

struct LocalTable {
    names: [u64; MAX_LOCALS],
    name_lens: [u8; MAX_LOCALS],
    offsets: [i32; MAX_LOCALS],
    count: usize,
    stack_size: i32,
}

impl LocalTable {
    const fn new() -> Self {
        Self {
            names: [0; MAX_LOCALS],
            name_lens: [0; MAX_LOCALS],
            offsets: [0; MAX_LOCALS],
            count: 0,
            stack_size: 0,
        }
    }

    fn declare(&mut self, name: &str) -> i32 {
        let bytes = name.as_bytes();
        let key = fnv1a(bytes);
        for i in 0..self.count {
            if self.names[i] == key && self.name_lens[i] == bytes.len() as u8 {
                return self.offsets[i];
            }
        }
        if self.count >= MAX_LOCALS { return 0; }
        self.stack_size -= 8;
        let idx = self.count;
        self.names[idx] = key;
        self.name_lens[idx] = bytes.len() as u8;
        self.offsets[idx] = self.stack_size;
        self.count += 1;
        self.stack_size
    }

    fn lookup(&self, name: &str) -> Option<i32> {
        let bytes = name.as_bytes();
        let key = fnv1a(bytes);
        for i in 0..self.count {
            if self.names[i] == key && self.name_lens[i] == bytes.len() as u8 {
                return Some(self.offsets[i]);
            }
        }
        None
    }
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

impl NativeCompiler {
    pub fn new() -> Self {
        Self {
            code: [0; CODE_BUF_SIZE],
            code_len: 0,
            locals: LocalTable::new(),
        }
    }

    fn emit(&mut self, byte: u8) {
        if self.code_len < CODE_BUF_SIZE {
            self.code[self.code_len] = byte;
            self.code_len += 1;
        }
    }

    fn emit32(&mut self, val: u32) {
        let b = val.to_le_bytes();
        self.emit(b[0]); self.emit(b[1]); self.emit(b[2]); self.emit(b[3]);
    }

    fn emit64(&mut self, val: u64) {
        let b = val.to_le_bytes();
        for &byte in &b { self.emit(byte); }
    }

    fn push_rax(&mut self) { self.emit(0x50); }
    fn pop_rax(&mut self) { self.emit(0x58); }
    fn pop_rcx(&mut self) { self.emit(0x59); }
    fn pop_rdx(&mut self) { self.emit(0x5A); }
    fn pop_rsi(&mut self) { self.emit(0x5E); }
    fn pop_rdi(&mut self) { self.emit(0x5F); }
    fn pop_r8(&mut self) { self.emit(0x41); self.emit(0x58); }
    fn pop_r9(&mut self) { self.emit(0x41); self.emit(0x59); }

    fn mov_rax_imm64(&mut self, val: u64) {
        self.emit(0x48); self.emit(0xB8); self.emit64(val);
    }

    fn mov_rax_rbp_off(&mut self, offset: i32) {
        if offset >= -128 && offset <= 127 {
            self.emit(0x48); self.emit(0x8B); self.emit(0x45); self.emit(offset as u8);
        } else {
            self.emit(0x48); self.emit(0x8B); self.emit(0x85); self.emit32(offset as u32);
        }
    }

    fn mov_rbp_off_rax(&mut self, offset: i32) {
        if offset >= -128 && offset <= 127 {
            self.emit(0x48); self.emit(0x89); self.emit(0x45); self.emit(offset as u8);
        } else {
            self.emit(0x48); self.emit(0x89); self.emit(0x85); self.emit32(offset as u32);
        }
    }

    fn add_rax_rcx(&mut self) { self.emit(0x48); self.emit(0x01); self.emit(0xC8); }
    fn sub_rcx_rax(&mut self) { self.emit(0x48); self.emit(0x29); self.emit(0xC1); }
    fn imul_rax_rcx(&mut self) { self.emit(0x48); self.emit(0xF7); self.emit(0xE9); }

    fn xor_rdx_rdx(&mut self) { self.emit(0x48); self.emit(0x31); self.emit(0xD2); }
    fn cqo(&mut self) { self.emit(0x48); self.emit(0x99); }
    fn idiv_rcx(&mut self) { self.emit(0x48); self.emit(0xF7); self.emit(0xF9); }
    fn and_rax_rcx(&mut self) { self.emit(0x48); self.emit(0x21); self.emit(0xC8); }
    fn or_rax_rcx(&mut self) { self.emit(0x48); self.emit(0x09); self.emit(0xC8); }
    fn xor_rax_rcx(&mut self) { self.emit(0x48); self.emit(0x31); self.emit(0xC8); }
    fn shl_rax_cl(&mut self) { self.emit(0x48); self.emit(0xD3); self.emit(0xE0); }
    fn shr_rax_cl(&mut self) { self.emit(0x48); self.emit(0xD3); self.emit(0xE8); }

    fn cmp_rax_rcx(&mut self) { self.emit(0x48); self.emit(0x39); self.emit(0xC8); }
    fn sete_al(&mut self) { self.emit(0x0F); self.emit(0x94); self.emit(0xC0); }
    fn setne_al(&mut self) { self.emit(0x0F); self.emit(0x95); self.emit(0xC0); }
    fn setl_al(&mut self) { self.emit(0x0F); self.emit(0x9C); self.emit(0xC0); }
    fn setg_al(&mut self) { self.emit(0x0F); self.emit(0x9F); self.emit(0xC0); }
    fn setle_al(&mut self) { self.emit(0x0F); self.emit(0x9E); self.emit(0xC0); }
    fn setge_al(&mut self) { self.emit(0x0F); self.emit(0x9D); self.emit(0xC0); }
    fn movzx_rax_al(&mut self) { self.emit(0x48); self.emit(0x0F); self.emit(0xB6); self.emit(0xC0); }
    fn neg_rax(&mut self) { self.emit(0x48); self.emit(0xF7); self.emit(0xD8); }
    fn test_rax_rax(&mut self) { self.emit(0x48); self.emit(0x85); self.emit(0xC0); }
    fn test_rcx_rcx(&mut self) { self.emit(0x48); self.emit(0x85); self.emit(0xC9); }

    fn sub_rsp_imm32(&mut self, imm: i32) {
        if imm >= -128 && imm <= 127 {
            self.emit(0x48); self.emit(0x83); self.emit(0xEC); self.emit(imm as u8);
        } else {
            self.emit(0x48); self.emit(0x81); self.emit(0xEC); self.emit32(imm as u32);
        }
    }

    fn syscall_nr(&mut self, nr: u64) {
        self.mov_rax_imm64(nr);
        self.emit(0x0F); self.emit(0x05); // syscall
        self.push_rax();
    }

    pub fn compile(&mut self, ast: &Ast) -> NativeCompilerResult {
        self.prologue();
        for item in &ast.items {
            self.compile_stmt(item);
        }
        self.epilogue();
        self.patch_prologue();

        let mut code = alloc::vec![0u8; self.code_len];
        code.copy_from_slice(&self.code[..self.code_len]);
        NativeCompilerResult { code, entry_offset: 0 }
    }

    fn prologue(&mut self) {
        self.emit(0x55); // push rbp
        self.emit(0x48); self.emit(0x89); self.emit(0xE5); // mov rbp, rsp
        self.sub_rsp_imm32(0); // placeholder patched later
    }

    fn patch_prologue(&mut self) {
        let frame_size = (-self.locals.stack_size + 15) & !15;
        let fb = frame_size as u32;
        self.code[4] = (fb & 0xFF) as u8;
        self.code[5] = ((fb >> 8) & 0xFF) as u8;
        self.code[6] = ((fb >> 16) & 0xFF) as u8;
        self.code[7] = ((fb >> 24) & 0xFF) as u8;
    }

    fn epilogue(&mut self) {
        self.mov_rax_imm64(60); // sys_exit
        self.pop_rdi();
        self.emit(0x0F); self.emit(0x05); // syscall
        self.emit(0x90); self.emit(0x90); self.emit(0x90); // nop padding
    }

    fn compile_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let { name, value, .. } => {
                if let Some(val) = value {
                    self.compile_expr(val);
                } else {
                    self.mov_rax_imm64(0);
                    self.push_rax();
                }
                let off = self.locals.declare(name);
                self.pop_rax();
                self.mov_rbp_off_rax(off);
            }
            Stmt::Mut { name, value, .. } => {
                self.compile_expr(value);
                let off = self.locals.declare(name);
                self.pop_rax();
                self.mov_rbp_off_rax(off);
            }
            Stmt::Assign(name, expr) => {
                self.compile_expr(expr);
                if let Some(off) = self.locals.lookup(name) {
                    self.pop_rax();
                    self.mov_rbp_off_rax(off);
                }
            }
            Stmt::Return(val) => {
                if let Some(v) = val {
                    self.compile_expr(v);
                } else {
                    self.mov_rax_imm64(0);
                    self.push_rax();
                }
                self.pop_rax();
                self.emit(0xC9); // leave
                self.emit(0xC3); // ret
            }
            Stmt::ExprStmt(expr) => {
                self.compile_expr(expr);
                self.pop_rax();
            }
            Stmt::If { cond, then_body, else_body } => {
                self.compile_expr(cond);
                self.pop_rax();
                self.test_rax_rax();
                let patch_jz = self.code_len;
                self.emit(0x74); self.emit(0x00); // jz placeholder (rel8)
                for s in then_body {
                    self.compile_stmt(s);
                }
                if let Some(els) = else_body {
                    let patch_jmp = self.code_len;
                    self.emit(0xE9); self.emit32(0); // jmp placeholder (rel32)
                    let else_start = self.code_len;
                    let disp = (else_start as isize - (patch_jz as isize + 2)) as i8;
                    self.code[patch_jz + 1] = disp as u8;
                    for s in els {
                        self.compile_stmt(s);
                    }
                    let after_else = self.code_len;
                    let d = (after_else as isize - (patch_jmp as isize + 5)) as i32;
                    self.code[patch_jmp + 1..patch_jmp + 5].copy_from_slice(&d.to_le_bytes());
                } else {
                    let after = self.code_len;
                    let disp = (after as isize - (patch_jz as isize + 2)) as i8;
                    self.code[patch_jz + 1] = disp as u8;
                }
            }
            Stmt::While { cond, body } => {
                let loop_start = self.code_len;
                self.compile_expr(cond);
                self.pop_rax();
                self.test_rax_rax();
                let patch_jz = self.code_len;
                self.emit(0x0F); self.emit(0x84); self.emit32(0); // jz rel32
                for s in body {
                    self.compile_stmt(s);
                }
                self.jmp_rel32(loop_start);
                let after = self.code_len;
                let d = (after as isize - (patch_jz as isize + 6)) as i32;
                self.code[patch_jz + 2..patch_jz + 6].copy_from_slice(&d.to_le_bytes());
            }
            Stmt::Block(stmts) => {
                for s in stmts {
                    self.compile_stmt(s);
                }
            }
            Stmt::Syscall { nr, args } => {
                for a in args.iter().rev() {
                    self.compile_expr(a);
                }
                self.syscall_regs(*nr, args.len());
            }
            Stmt::FnDecl { name: _, params: _, body, .. } => {
                let saved = LocalTable::new();
                let old_locals = core::mem::replace(&mut self.locals, saved);
                let fn_start = self.code_len;
                self.emit(0x55);
                self.emit(0x48); self.emit(0x89); self.emit(0xE5);
                self.sub_rsp_imm32(0);
                for s in body {
                    self.compile_stmt(s);
                }
                self.mov_rax_imm64(0);
                self.push_rax();
                self.pop_rax();
                self.emit(0xC9);
                self.emit(0xC3);
                let fs = (-self.locals.stack_size + 15) & !15;
                let fb = fs as u32;
                self.code[fn_start + 4] = (fb & 0xFF) as u8;
                self.code[fn_start + 5] = ((fb >> 8) & 0xFF) as u8;
                self.code[fn_start + 6] = ((fb >> 16) & 0xFF) as u8;
                self.code[fn_start + 7] = ((fb >> 24) & 0xFF) as u8;
                let _ = core::mem::replace(&mut self.locals, old_locals);
            }
            Stmt::For { var, start, end, body } => {
                self.compile_expr(start);
                let off = self.locals.declare(var);
                self.pop_rax();
                self.mov_rbp_off_rax(off);
                let loop_start = self.code_len;
                self.mov_rax_rbp_off(off);
                self.push_rax();
                self.compile_expr(end);
                self.pop_rax();
                self.pop_rcx();
                self.cmp_rax_rcx();
                let patch_jge = self.code_len;
                self.emit(0x0F); self.emit(0x8D); self.emit32(0); // jge rel32
                for s in body {
                    self.compile_stmt(s);
                }
                self.mov_rax_rbp_off(off);
                self.push_rax();
                self.mov_rax_imm64(1);
                self.pop_rcx();
                self.pop_rax();
                self.emit(0x48); self.emit(0x01); self.emit(0xC8); // add rax, rcx
                self.push_rax();
                self.pop_rax();
                self.mov_rbp_off_rax(off);
                self.jmp_rel32(loop_start);
                let after = self.code_len;
                let d = (after as isize - (patch_jge as isize + 6)) as i32;
                self.code[patch_jge + 2..patch_jge + 6].copy_from_slice(&d.to_le_bytes());
            }
            Stmt::Break => {}
            Stmt::Continue => {}
            Stmt::StructDecl { .. } => {}
            Stmt::EnumDecl { .. } => {}
            Stmt::ImplDecl { .. } => {}
            Stmt::Emit(bytes) => {
                for &b in bytes {
                    self.emit(b);
                }
            }
            Stmt::Aloc { size } => {
                self.compile_expr(size);
                self.pop_rax();
                self.push_rax();
            }
            Stmt::Libre(_) => {}
            Stmt::Module { items, .. } => {
                for s in items {
                    self.compile_stmt(s);
                }
            }
            Stmt::Use { .. } => {}
            Stmt::UseGlob { .. } => {}
            Stmt::Pub { inner } => self.compile_stmt(inner),
            Stmt::Extern { .. } => {}
        }
    }

    fn compile_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::LitInt(n) => {
                self.mov_rax_imm64(*n);
                self.push_rax();
            }
            Expr::LitBool(b) => {
                self.mov_rax_imm64(if *b { 1 } else { 0 });
                self.push_rax();
            }
            Expr::LitStr(_) => {
                self.mov_rax_imm64(0); // null ptr placeholder
                self.push_rax();
            }
            Expr::Ident(name) => {
                if let Some(off) = self.locals.lookup(name) {
                    self.mov_rax_rbp_off(off);
                    self.push_rax();
                } else {
                    self.mov_rax_imm64(0);
                    self.push_rax();
                }
            }
            Expr::Binary(op, left, right) => {
                self.compile_expr(right);
                self.compile_expr(left);
                self.pop_rax();
                self.pop_rcx();
                match op {
                    BinOp::Add => self.add_rax_rcx(),
                    BinOp::Sub => self.sub_rcx_rax(),
                    BinOp::Mul => self.imul_rax_rcx(),
                    BinOp::Div => { self.cqo(); self.idiv_rcx(); }
                    BinOp::Mod => { self.cqo(); self.idiv_rcx(); self.mov_rdx_rax(); }
                    BinOp::And => self.and_rax_rcx(),
                    BinOp::Or => self.or_rax_rcx(),
                    BinOp::Xor => self.xor_rax_rcx(),
                    BinOp::Shl => { self.mov_rcx_into_cl(); self.shl_rax_cl(); }
                    BinOp::Shr => { self.mov_rcx_into_cl(); self.shr_rax_cl(); }
                    BinOp::Eq => { self.cmp_rax_rcx(); self.sete_al(); self.movzx_rax_al(); }
                    BinOp::Ne => { self.cmp_rax_rcx(); self.setne_al(); self.movzx_rax_al(); }
                    BinOp::Lt => { self.cmp_rax_rcx(); self.setl_al(); self.movzx_rax_al(); }
                    BinOp::Gt => { self.cmp_rax_rcx(); self.setg_al(); self.movzx_rax_al(); }
                    BinOp::Le => { self.cmp_rax_rcx(); self.setle_al(); self.movzx_rax_al(); }
                    BinOp::Ge => { self.cmp_rax_rcx(); self.setge_al(); self.movzx_rax_al(); }
                    BinOp::Land => {
                        self.test_rax_rax(); self.setne_al(); self.movzx_rax_al();
                        self.push_rax();
                        self.test_rcx_rcx(); self.setne_al(); self.movzx_rax_al();
                        self.pop_rcx();
                        self.and_rax_rcx();
                    }
                    BinOp::Lor => {
                        self.test_rax_rax(); self.setne_al(); self.movzx_rax_al();
                        self.push_rax();
                        self.test_rcx_rcx(); self.setne_al(); self.movzx_rax_al();
                        self.pop_rcx();
                        self.or_rax_rcx();
                    }
                }
                self.push_rax();
            }
            Expr::Unary(op, inner) => {
                self.compile_expr(inner);
                self.pop_rax();
                match op {
                    UnaryOp::Neg => self.neg_rax(),
                    UnaryOp::Not => { self.test_rax_rax(); self.sete_al(); self.movzx_rax_al(); }
                    UnaryOp::Deref | UnaryOp::Ref => {}
                }
                self.push_rax();
            }
            Expr::Syscall(nr, args) => {
                for a in args.iter().rev() {
                    self.compile_expr(a);
                }
                self.syscall_regs(*nr, args.len());
            }
            Expr::Call(name, args) => {
                for a in args.iter().rev() {
                    self.compile_expr(a);
                }
                let _ = name;
                for _ in 0..args.len().min(6) {
                    match args.len() - 1 {
                        0 => self.pop_rdi(),
                        1 => self.pop_rsi(),
                        2 => self.pop_rdx(),
                        3 => self.pop_rcx(),
                        4 => self.pop_r8(),
                        5 => self.pop_r9(),
                        _ => self.pop_rax(),
                    }
                }
                self.mov_rax_imm64(0);
                self.emit(0xFF); self.emit(0xD0); // call rax
                self.push_rax();
            }
            Expr::Emit(bytes) => {
                for &b in bytes {
                    self.emit(b);
                }
            }
            Expr::Block(stmts) => {
                for s in stmts {
                    self.compile_stmt(s);
                }
            }
            Expr::QualifiedCall(_, args) => {
                for a in args.iter().rev() {
                    self.compile_expr(a);
                }
                for _ in 0..args.len().min(6) {
                    match args.len() - 1 {
                        0 => self.pop_rdi(),
                        1 => self.pop_rsi(),
                        2 => self.pop_rdx(),
                        3 => self.pop_rcx(),
                        4 => self.pop_r8(),
                        5 => self.pop_r9(),
                        _ => self.pop_rax(),
                    }
                }
                self.mov_rax_imm64(0);
                self.emit(0xFF); self.emit(0xD0);
                self.push_rax();
            }
            Expr::QualifiedPath(_) => {
                self.mov_rax_imm64(0);
                self.push_rax();
            }
            Expr::MethodCall(_, _, _) => {
                self.mov_rax_imm64(0);
                self.push_rax();
            }
            Expr::Field(_, _) => {
                self.mov_rax_imm64(0);
                self.push_rax();
            }
            Expr::Index(_, _) => {
                self.mov_rax_imm64(0);
                self.push_rax();
            }
            Expr::Aloc(_) => {
                self.mov_rax_imm64(0);
                self.push_rax();
            }
            Expr::Libre(_) => {}
            Expr::Reg(_) => {
                self.mov_rax_imm64(0);
                self.push_rax();
            }
            Expr::LitFloat(_) => {
                self.mov_rax_imm64(0);
                self.push_rax();
            }
            Expr::LitByte(b) => {
                self.mov_rax_imm64(*b as u64);
                self.push_rax();
            }
            Expr::LitNull => {
                self.mov_rax_imm64(0);
                self.push_rax();
            }
        }
    }

    fn syscall_regs(&mut self, nr: u64, arg_count: usize) {
        for i in (0..arg_count).rev() {
            match i {
                0 => self.pop_rdi(),
                1 => self.pop_rsi(),
                2 => self.pop_rdx(),
                3 => self.pop_rcx(),
                4 => self.pop_r8(),
                5 => self.pop_r9(),
                _ => self.pop_rax(),
            }
        }
        self.syscall_nr(nr);
    }

    fn jmp_rel32(&mut self, target: usize) {
        self.emit(0xE9);
        let disp = target as i64 - (self.code_len as i64 + 4);
        self.emit32(disp as u32);
    }

    fn mov_rdx_rax(&mut self) { self.emit(0x48); self.emit(0x89); self.emit(0xD0); }
    fn mov_rcx_into_cl(&mut self) { self.emit(0x48); self.emit(0x89); self.emit(0xC8); }
    fn not_rax(&mut self) { self.emit(0x48); self.emit(0xF7); self.emit(0xD0); }

}

impl Default for NativeCompiler {
    fn default() -> Self { Self::new() }
}

pub struct NativeCompilerResult {
    pub code: alloc::vec::Vec<u8>,
    pub entry_offset: usize,
}
