//! Traductor central de BMO Simple.
//! Coordina el Lexer, Parser, Sema y Emitter para producir bytes nativos del target.
//! Usa `CodegenBackend` trait — sin match sobre TargetEmitter.

extern crate alloc;
use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use crate::barex::{BxError, BxResult};
use super::parser::{Parser, Ast, Stmt, Expr, BinOp, Type};
use super::sema::Sema;
use super::sema::scope::{Scope, ScopeEntry};
use super::emit::TargetArch;
use super::emit::backend::CodegenBackend;

struct StringRef {
    disp_offset: usize,
    rodata_offset: usize,
}

struct LoopContext {
    break_patches: Vec<usize>,
    continue_patches: Vec<usize>,
}

struct FunctionEntry {
    name: String,
    code_offset: usize,
    param_count: usize,
}

struct CallPatch {
    call_offset: usize,
    fn_name: String,
}

struct LabelPatch {
    jmp_offset: usize,
    label_name: String,
}

pub struct Traductor {
    target: TargetArch,
    backend: Box<dyn CodegenBackend>,
    /// Raw bytes accessor for back-patching (kept separate from backend).
    raw_bytes: Vec<u8>,
    rodata: Vec<u8>,
    string_refs: Vec<StringRef>,
    loop_stack: Vec<LoopContext>,
    scope: Scope,
    frame_size: u32,
    fn_table: BTreeMap<String, FunctionEntry>,
    call_patches: Vec<CallPatch>,
    label_table: BTreeMap<String, usize>,
    label_patches: Vec<LabelPatch>,
}

impl Traductor {
    pub fn new() -> Self {
        Self::with_target(TargetArch::X86_64)
    }

    pub fn with_target(target: TargetArch) -> Self {
        let backend: Box<dyn CodegenBackend> = match target {
            TargetArch::X86_64 => Box::new(super::emit::x86_64::encoder::Emitter::new()),
            TargetArch::Aarch64 => Box::new(super::emit::aarch64::EmitterArm::new()),
            TargetArch::Riscv64 => Box::new(super::emit::riscv::EmitterRiscv::new()),
        };
        Self {
            target,
            backend,
            raw_bytes: Vec::new(),
            rodata: Vec::new(),
            string_refs: Vec::new(),
            loop_stack: Vec::new(),
            scope: Scope::default(),
            frame_size: 0,
            fn_table: BTreeMap::new(),
            call_patches: Vec::new(),
            label_table: BTreeMap::new(),
            label_patches: Vec::new(),
        }
    }

    pub fn traducir(&mut self, src: &[u8]) -> BxResult<Vec<u8>> {
        let mut parser = Parser::new(src);
        let mut ast = parser.parse().map_err(|_e| {
            crate::barex::BxError::InvalidArgument
        })?;

        // Constant folding
        super::sema::fold::Folder::fold(&mut ast);

        let sema = Sema::new();
        sema.check(&ast)?;

        self.compilar_ast(&ast)?;

        // Back-patching de strings
        let final_code_len = self.backend.here();
        for s_ref in &self.string_refs {
            self.backend.patch_string_ref(s_ref.disp_offset, s_ref.rodata_offset, final_code_len);
        }

        // Back-patching de calls forward
        for patch in &self.call_patches {
            if let Some(entry) = self.fn_table.get(&patch.fn_name) {
                let code_offset = entry.code_offset;
                self.backend.patch_rel32(patch.call_offset, patch.call_offset + 4, code_offset);
            } else {
                return Err(BxError::InvalidArgument);
            }
        }

        // Back-patching de labels forward
        for patch in &self.label_patches {
            if let Some(&target) = self.label_table.get(&patch.label_name) {
                self.backend.patch_rel32(patch.jmp_offset, patch.jmp_offset + 4, target);
            } else {
                return Err(BxError::InvalidArgument);
            }
        }

        // Concatena código + rodata
        let mut final_bytes = Vec::new();
        final_bytes.extend_from_slice(self.backend.bytes_mut());
        final_bytes.extend_from_slice(&self.rodata);

        Ok(final_bytes)
    }

    fn compilar_ast(&mut self, ast: &Ast) -> BxResult<()> {
        for item in &ast.items {
            match item {
                Stmt::Def { name, params, ret, body } => {
                    let code_offset = self.backend.here();
                    self.fn_table.insert(name.clone(), FunctionEntry {
                        name: name.clone(),
                        code_offset,
                        param_count: params.len(),
                    });
                    self.compilar_funcion(params, *ret, body)?;
                }
                _ => return Err(BxError::InvalidArgument),
            }
        }
        Ok(())
    }

    fn compilar_funcion(
        &mut self,
        params: &[(String, Type)],
        _ret: Type,
        body: &[Stmt],
    ) -> BxResult<()> {
        self.scope = Scope::default();
        self.frame_size = 0;

        let param_space = (params.len() * 8) as u32;
        self.frame_size = param_space;

        let prologue_offset = self.backend.emit_prologue();

        for (i, (pname, pty)) in params.iter().enumerate() {
            if i < self.backend.arg_reg_count() {
                let offset = -((i as i32 + 1) * 8);
                if let Some(arg_r) = self.backend.arg_reg(i) {
                    self.backend.mov_reg_reg(self.backend.acc_reg(), arg_r);
                    self.backend.store_var(offset);
                }
                self.scope.push(ScopeEntry {
                    name: pname.clone(),
                    ty: *pty,
                    frame_offset: -((i as i32 + 1) * 8),
                });
            }
        }

        self.compilar_body(body)?;

        self.backend.patch_frame_size(prologue_offset, self.frame_size);
        self.backend.emit_epilogue();

        Ok(())
    }

    fn compilar_body(&mut self, body: &[Stmt]) -> BxResult<()> {
        for stmt in body {
            match stmt {
                Stmt::RegAssign { reg, value } => {
                    let dst_reg = self.backend.parse_reg(reg).ok_or(BxError::InvalidArgument)?;
                    match value {
                        Expr::LitInt(imm) => {
                            self.backend.mov_acc_imm(*imm);
                            self.backend.mov_reg_acc(dst_reg);
                        }
                        Expr::LitStr(s) => {
                            let rodata_offset = self.rodata.len();
                            self.rodata.extend_from_slice(s.as_bytes());
                            self.rodata.push(0);
                            // LEA placeholder — use raw bytes for now
                            let disp_offset = match self.target {
                                TargetArch::X86_64 => {
                                    // lea reg, [rip+0]
                                    let mut rex = 0x48u8;
                                    if dst_reg >= 8 { rex |= 0x04; }
                                    let bytes = self.backend.bytes_mut();
                                    bytes.push(rex);
                                    bytes.push(0x8D);
                                    let modrm = 0x05 | ((dst_reg as u8 & 0x07) << 3);
                                    bytes.push(modrm);
                                    let off = bytes.len();
                                    bytes.extend_from_slice(&[0, 0, 0, 0]);
                                    off
                                }
                                _ => return Err(BxError::Unsupported),
                            };
                            self.string_refs.push(StringRef { disp_offset, rodata_offset });
                        }
                        Expr::Reg(src_reg_name) => {
                            let src_reg = self.backend.parse_reg(src_reg_name).ok_or(BxError::InvalidArgument)?;
                            self.backend.mov_reg_reg(dst_reg, src_reg);
                        }
                        _ => {
                            self.codegen_expr(value)?;
                            self.backend.mov_reg_acc(dst_reg);
                        }
                    }
                }
                Stmt::Let { name, ty: _, value } => {
                    self.codegen_expr(value)?;
                    let offset = -(self.frame_size as i32) - 8;
                    self.frame_size += 8;
                    self.backend.store_var(offset);
                    self.scope.push(ScopeEntry {
                        name: name.clone(),
                        ty: Type::Num,
                        frame_offset: offset,
                    });
                }
                Stmt::Retorna(expr_opt) => {
                    if let Some(expr) = expr_opt {
                        self.codegen_expr(expr)?;
                        // Ensure result is in ret_reg
                        let ret_r = self.backend.ret_reg();
                        let acc_r = self.backend.acc_reg();
                        if ret_r != acc_r {
                            self.backend.mov_reg_reg(ret_r, acc_r);
                        }
                    }
                    self.backend.emit_epilogue();
                }
                Stmt::Emit(raw_bytes) => {
                    self.backend.emit_bytes(raw_bytes);
                }
                Stmt::ExprStmt(Expr::Reg(r_name)) => {
                    if let Some(bytes) = self.backend.intrinsic_bytes(r_name) {
                        self.backend.emit_bytes(bytes);
                    } else {
                        return Err(BxError::InvalidArgument);
                    }
                }
                Stmt::Si { cond, then_body, else_body } => {
                    self.compilar_si(cond, then_body, else_body.as_deref())?;
                }
                Stmt::Mientras { cond, body } => {
                    self.compilar_mientras(cond, body)?;
                }
                Stmt::Rompe => {
                    self.compilar_rompe()?;
                }
                Stmt::Continua => {
                    self.compilar_continua()?;
                }
                Stmt::ExprStmt(expr) => {
                    self.codegen_expr(expr)?;
                }
                Stmt::FnForward { .. } => {}
                Stmt::Match { expr, arms, default } => {
                    self.compilar_match(expr, arms, default.as_deref())?;
                }
                Stmt::Para { var, desde, hasta, paso, body } => {
                    self.compilar_para(var, desde, hasta, paso.as_ref(), body)?;
                }
                Stmt::Bucle(body) => {
                    self.compilar_bucle(body)?;
                }
                Stmt::Etiqueta(name) => {
                    self.compilar_etiqueta(name)?;
                }
                Stmt::Salto(name) => {
                    self.compilar_salto(name)?;
                }
                Stmt::Libre(expr) => {
                    // libre(ptr) → free memory allocated by aloc
                    // ptr points to user data (base + 8), metadata is at ptr - 8
                    self.codegen_expr(expr)?;
                    let bytes = self.backend.bytes_mut();
                    // sub rax, 8 (point to metadata/size)
                    bytes.extend_from_slice(&[0x48, 0x83, 0xE8, 0x08]); // sub rax, 8
                    // mov rdi, rax (first arg: base address)
                    bytes.extend_from_slice(&[0x48, 0x89, 0xC7]); // mov rdi, rax
                    // mov rsi, [rax] (second arg: size in bytes)
                    bytes.extend_from_slice(&[0x48, 0x8B, 0x30]); // mov rsi, [rax]
                    // Round size to pages: add rsi, 4095; shr rsi, 12
                    bytes.extend_from_slice(&[0x48, 0x81, 0xC6, 0xFF, 0x0F, 0x00, 0x00]); // add rsi, 4095
                    bytes.extend_from_slice(&[0x48, 0xC1, 0xEE, 0x0C]); // shr rsi, 12
                    // call [rip+disp32] → free_pages
                    bytes.push(0xFF);
                    bytes.push(0x15);
                    let func_ptr_offset = self.rodata.len();
                    let func_addr = crate::arch::page_alloc::free_pages as *const () as usize as u64;
                    self.rodata.extend_from_slice(&func_addr.to_le_bytes());
                    let disp_offset = bytes.len();
                    bytes.extend_from_slice(&[0, 0, 0, 0]);
                    self.string_refs.push(StringRef { disp_offset, rodata_offset: func_ptr_offset });
                }
                _ => return Err(BxError::Unsupported),
            }
        }
        Ok(())
    }

    // ── Control flow codegen ───────────────────────────────────────

    fn compilar_si(
        &mut self,
        cond: &Expr,
        then_body: &[Stmt],
        else_body: Option<&[Stmt]>,
    ) -> BxResult<()> {
        self.codegen_expr(cond)?;
        self.backend.test_acc();

        let jelse = if else_body.is_some() {
            Some(self.backend.je_rel32())
        } else {
            None
        };

        self.compilar_body(then_body)?;

        if let Some(eb) = else_body {
            let jend = self.backend.jmp_rel32();
            let else_start = self.backend.here();
            self.backend.patch_rel32(jelse.unwrap(), jelse.unwrap() + 4, else_start);
            self.compilar_body(eb)?;
            let end = self.backend.here();
            self.backend.patch_rel32(jend, jend + 4, end);
        } else {
            let end = self.backend.here();
            self.backend.patch_rel32(jelse.unwrap(), jelse.unwrap() + 4, end);
        }
        Ok(())
    }

    fn compilar_mientras(&mut self, cond: &Expr, body: &[Stmt]) -> BxResult<()> {
        let loop_start = self.backend.here();
        self.codegen_expr(cond)?;
        self.backend.test_acc();
        let jend = self.backend.je_rel32();

        self.loop_stack.push(LoopContext {
            break_patches: Vec::new(),
            continue_patches: Vec::new(),
        });
        self.compilar_body(body)?;
        let ctx = self.loop_stack.pop().unwrap();

        for patch in &ctx.continue_patches {
            self.backend.patch_rel32(*patch, *patch + 4, loop_start);
        }
        let jback = self.backend.jmp_rel32();
        self.backend.patch_rel32(jback, jback + 4, loop_start);
        let loop_end = self.backend.here();
        self.backend.patch_rel32(jend, jend + 4, loop_end);
        for patch in &ctx.break_patches {
            self.backend.patch_rel32(*patch, *patch + 4, loop_end);
        }
        Ok(())
    }

    fn compilar_rompe(&mut self) -> BxResult<()> {
        let jmp = self.backend.jmp_rel32();
        let ctx = self.loop_stack.last_mut().ok_or(BxError::InvalidArgument)?;
        ctx.break_patches.push(jmp);
        Ok(())
    }

    fn compilar_continua(&mut self) -> BxResult<()> {
        let jmp = self.backend.jmp_rel32();
        let ctx = self.loop_stack.last_mut().ok_or(BxError::InvalidArgument)?;
        ctx.continue_patches.push(jmp);
        Ok(())
    }

    // ── match/caso ─────────────────────────────────────────────────

    fn compilar_match(
        &mut self,
        expr: &Expr,
        arms: &[(Expr, Vec<Stmt>)],
        default: Option<&[Stmt]>,
    ) -> BxResult<()> {
        let mut end_patches: Vec<usize> = Vec::new();

        for (_i, (_pattern, body)) in arms.iter().enumerate() {
            // Re-evaluate match expr → scratch, eval pattern → acc, compare
            self.codegen_expr(expr)?;
            let scratch = self.backend.scratch_reg();
            self.backend.mov_reg_acc(scratch);
            self.codegen_expr(_pattern)?;
            self.backend.cmp_eq_acc(scratch);
            self.backend.test_acc();
            let jbody = self.backend.je_rel32();
            let jnext = self.backend.jmp_rel32();
            let body_start = self.backend.here();
            self.backend.patch_rel32(jbody, jbody + 4, body_start);
            self.compilar_body(body)?;
            end_patches.push(self.backend.jmp_rel32());
            let next_start = self.backend.here();
            self.backend.patch_rel32(jnext, jnext + 4, next_start);
        }

        if let Some(def) = default {
            self.compilar_body(def)?;
        }

        let end = self.backend.here();
        for patch in &end_patches {
            self.backend.patch_rel32(*patch, *patch + 4, end);
        }

        Ok(())
    }

    // ── para/desde/hasta/paso ──────────────────────────────────────

    fn compilar_para(
        &mut self,
        var: &str,
        desde: &Expr,
        hasta: &Expr,
        paso: Option<&Expr>,
        body: &[Stmt],
    ) -> BxResult<()> {
        // Evaluate hasta once
        self.codegen_expr(hasta)?;
        let hasta_offset = -(self.frame_size as i32) - 8;
        self.frame_size += 8;
        self.backend.store_var(hasta_offset);

        // Evaluate step once (default 1)
        let paso_offset = -(self.frame_size as i32) - 8;
        self.frame_size += 8;
        if let Some(p) = paso {
            self.codegen_expr(p)?;
        } else {
            self.backend.mov_acc_imm(1);
        }
        self.backend.store_var(paso_offset);

        // Initialize loop variable
        self.codegen_expr(desde)?;
        let var_offset = -(self.frame_size as i32) - 8;
        self.frame_size += 8;
        self.backend.store_var(var_offset);
        self.scope.push(ScopeEntry {
            name: alloc::string::ToString::to_string(var),
            ty: Type::Num,
            frame_offset: var_offset,
        });

        let loop_start = self.backend.here();

        // Loop condition: if var < hasta → continue, else exit
        self.backend.load_var(var_offset);
        let scratch = self.backend.scratch_reg();
        self.backend.mov_reg_acc(scratch);
        self.backend.load_var(hasta_offset);
        // acc = hasta, scratch = var
        // cmp_lt_acc(scratch) → acc = (scratch < acc) ? 1 : 0 = (var < hasta) ? 1 : 0
        self.backend.cmp_lt_acc(scratch);
        self.backend.test_acc();
        let jend = self.backend.je_rel32();

        self.loop_stack.push(LoopContext {
            break_patches: Vec::new(),
            continue_patches: Vec::new(),
        });
        self.compilar_body(body)?;
        let ctx = self.loop_stack.pop().unwrap();

        for patch in &ctx.continue_patches {
            self.backend.patch_rel32(*patch, *patch + 4, loop_start);
        }

        // Increment variable by step
        self.backend.load_var(var_offset);
        self.backend.mov_reg_acc(scratch);
        self.backend.load_var(paso_offset);
        // acc = step, scratch = var
        self.backend.add_acc(scratch);
        self.backend.store_var(var_offset);

        let jback = self.backend.jmp_rel32();
        self.backend.patch_rel32(jback, jback + 4, loop_start);
        let loop_end = self.backend.here();
        self.backend.patch_rel32(jend, jend + 4, loop_end);
        for patch in &ctx.break_patches {
            self.backend.patch_rel32(*patch, *patch + 4, loop_end);
        }

        Ok(())
    }

    // ── bucle (infinite loop) ──────────────────────────────────────

    fn compilar_bucle(&mut self, body: &[Stmt]) -> BxResult<()> {
        let loop_start = self.backend.here();
        self.loop_stack.push(LoopContext {
            break_patches: Vec::new(),
            continue_patches: Vec::new(),
        });
        self.compilar_body(body)?;
        let ctx = self.loop_stack.pop().unwrap();

        let jback = self.backend.jmp_rel32();
        self.backend.patch_rel32(jback, jback + 4, loop_start);

        let loop_end = self.backend.here();
        for patch in &ctx.break_patches {
            self.backend.patch_rel32(*patch, *patch + 4, loop_end);
        }
        for patch in &ctx.continue_patches {
            self.backend.patch_rel32(*patch, *patch + 4, loop_start);
        }

        Ok(())
    }

    // ── etiqueta/salto (labels/gotos) ─────────────────────────────

    fn compilar_etiqueta(&mut self, name: &str) -> BxResult<()> {
        let offset = self.backend.here();
        self.label_table.insert(alloc::string::ToString::to_string(name), offset);
        Ok(())
    }

    fn compilar_salto(&mut self, name: &str) -> BxResult<()> {
        let jmp_offset = self.backend.jmp_rel32();
        if let Some(&target) = self.label_table.get(name) {
            // Backward reference: patch immediately
            self.backend.patch_rel32(jmp_offset, jmp_offset + 4, target);
        } else {
            // Forward reference: add to patch list
            self.label_patches.push(LabelPatch {
                jmp_offset,
                label_name: alloc::string::ToString::to_string(name),
            });
        }
        Ok(())
    }

    // ── Expression codegen ─────────────────────────────────────────

    fn codegen_expr(&mut self, expr: &Expr) -> BxResult<()> {
        match expr {
            Expr::LitInt(imm) => {
                self.backend.mov_acc_imm(*imm);
            }
            Expr::LitByte(b) => {
                self.backend.zero_acc();
                let bytes = self.backend.bytes_mut();
                bytes.push(0xB0);
                bytes.push(*b);
            }
            Expr::Reg(r_name) => {
                let r = self.backend.parse_reg(r_name).ok_or(BxError::InvalidArgument)?;
                if r != self.backend.acc_reg() {
                    self.backend.mov_acc_reg(r);
                }
            }
            Expr::Ident(name) => {
                let entry = self.scope.lookup(name).ok_or(BxError::InvalidArgument)?;
                self.backend.load_var(entry.frame_offset);
            }
            Expr::Bin(op, left, right) => {
                self.codegen_expr(left)?;
                self.backend.push_acc();
                self.codegen_expr(right)?;
                let scratch = self.backend.scratch_reg();
                self.backend.mov_reg_acc(scratch);
                self.backend.pop_acc();
                // acc = left, scratch = right
                match op {
                    BinOp::Suma => self.backend.add_acc(scratch),
                    BinOp::Resta => self.backend.sub_acc(scratch),
                    BinOp::Mult => self.backend.mul_acc(scratch),
                    BinOp::Div => self.backend.div_acc(scratch),
                    BinOp::Mod => self.backend.mod_acc(scratch),
                    BinOp::Y => self.backend.and_acc(scratch),
                    BinOp::O => self.backend.or_acc(scratch),
                    BinOp::Xor => self.backend.xor_acc(scratch),
                    BinOp::Shl => self.backend.shl_acc(scratch),
                    BinOp::Shr => self.backend.shr_acc(scratch),
                    BinOp::Igual => self.backend.cmp_eq_acc(scratch),
                    BinOp::Mayor => self.backend.cmp_gt_acc(scratch),
                    BinOp::Menor => self.backend.cmp_lt_acc(scratch),
                    BinOp::MayIg => {
                        // !(a < b)
                        self.backend.cmp_lt_acc(scratch);
                        self.backend.test_acc();
                        self.backend.sete_acc();
                    }
                    BinOp::MenIg => {
                        // !(a > b)
                        self.backend.cmp_gt_acc(scratch);
                        self.backend.test_acc();
                        self.backend.sete_acc();
                    }
                    BinOp::Difer => {
                        self.backend.cmp_eq_acc(scratch);
                        self.backend.test_acc();
                        self.backend.sete_acc(); // sete on NOT-equal
                    }
                }
            }
            Expr::No(inner) => {
                self.codegen_expr(inner)?;
                self.backend.test_acc();
                self.backend.zero_acc();
                self.backend.sete_acc();
            }
            Expr::Aloc(size_expr) => {
                // aloc(N) → allocate N bytes with size metadata
                // Layout: [u64 size][user data...]
                // Returns pointer to user data (base + 8)
                self.codegen_expr(size_expr)?;
                let bytes = self.backend.bytes_mut();
                // Save original size for metadata: push rax
                bytes.push(0x50); // push rax
                // Round up to pages: add rax, 4095; shr rax, 12
                bytes.extend_from_slice(&[0x48, 0x05, 0xFF, 0x0F, 0x00, 0x00]); // add rax, 4095
                bytes.extend_from_slice(&[0x48, 0xC1, 0xE8, 0x0C]); // shr rax, 12
                // Add 1 page for metadata header
                bytes.extend_from_slice(&[0x48, 0xFF, 0xC0]); // inc rax
                // mov rdi, rax (page count → first arg)
                bytes.extend_from_slice(&[0x48, 0x89, 0xC7]); // mov rdi, rax
                // call [rip+disp32] → alloc_pages_contiguous
                bytes.push(0xFF);
                bytes.push(0x15);
                let func_ptr_offset = self.rodata.len();
                let func_addr = crate::arch::page_alloc::alloc_pages_contiguous as *const () as usize as u64;
                self.rodata.extend_from_slice(&func_addr.to_le_bytes());
                let disp_offset = bytes.len();
                bytes.extend_from_slice(&[0, 0, 0, 0]);
                self.string_refs.push(StringRef { disp_offset, rodata_offset: func_ptr_offset });
                // RAX = base address. Now store metadata and return base + 8.
                // mov [rax], rax_value (but we need original size from stack)
                // pop rcx (original size)
                bytes.extend_from_slice(&[0x59]); // pop rcx
                // mov [rax], rcx (store size metadata at base)
                bytes.extend_from_slice(&[0x48, 0x89, 0x08]); // mov [rax], rcx
                // add rax, 8 (skip metadata, return user pointer)
                bytes.extend_from_slice(&[0x48, 0x83, 0xC0, 0x08]); // add rax, 8
            }
            Expr::Call { name, args } => {
                for (i, arg) in args.iter().enumerate() {
                    if i >= self.backend.arg_reg_count() {
                        return Err(BxError::InvalidArgument);
                    }
                    self.codegen_expr(arg)?;
                    if let Some(arg_r) = self.backend.arg_reg(i) {
                        self.backend.mov_reg_reg(arg_r, self.backend.acc_reg());
                    }
                }
                let call_offset = self.backend.call_rel32();
                if let Some(entry) = self.fn_table.get(name) {
                    let code_offset = entry.code_offset;
                    self.backend.patch_rel32(call_offset, call_offset + 4, code_offset);
                } else {
                    self.call_patches.push(CallPatch {
                        call_offset,
                        fn_name: name.clone(),
                    });
                }
            }
            Expr::LitNulo => {
                self.backend.zero_acc();
            }
            Expr::LitStr(_) => {
                self.backend.zero_acc();
            }
        }
        Ok(())
    }
}

impl Default for Traductor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::bmoasm::sample::SALUDO;

    #[test]
    fn test_traductor_saludo() {
        let mut trad = Traductor::new();
        let res = trad.traducir(SALUDO.as_bytes());
        assert!(res.is_ok());
        let bytes = res.unwrap();
        assert!(bytes.windows(5).any(|w| w == b"hola\0"));
    }
}
