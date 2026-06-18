//! Traductor central de BMO Simple v0.3.0.
//! Pipeline modular: parse → fold → sema → traducir.
//! Soporta: incluye, cuando, atomico/volatil/acquire/release, barr.
//! Usa `CodegenBackend` trait — sin match sobre TargetEmitter.

extern crate alloc;
use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use crate::barex::{BxError, BxResult};
use super::parser::{Parser, Ast, Stmt, Expr, BinOp, Type};
use super::parser::ast::{CpuFlag, MemOrder};
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

/// Type layout entry: one per field. The first element is the field
/// name, the second is the byte offset from the start of the struct.
struct FieldLayout {
    name: String,
    offset: i32,
    size: u8,
}

pub struct Traductor {
    target: TargetArch,
    backend: Box<dyn CodegenBackend>,
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
    /// File registry: path → source bytes (for incluye).
    file_registry: BTreeMap<String, Vec<u8>>,
    /// Type layouts: type_name → field layouts in declaration order.
    /// Used by `Expr::Field` and `Stmt::FieldAssign` to compute offsets.
    type_layouts: BTreeMap<String, Vec<FieldLayout>>,
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
            file_registry: BTreeMap::new(),
            type_layouts: BTreeMap::new(),
        }
    }

    /// Register a file for multi-file compilation.
    pub fn register_file(&mut self, path: &str, source: Vec<u8>) {
        self.file_registry.insert(String::from(path), source);
    }

    pub fn traducir(&mut self, src: &[u8]) -> BxResult<Vec<u8>> {
        let mut parser = Parser::new(src);
        let mut ast = parser.parse().map_err(|_e| BxError::InvalidArgument)?;
        self.traducir_ast(&mut ast)
    }

    /// Compile a pre-parsed AST directly. This is the no-roundtrip
    /// entry point used by the ÑEXO codegen, which produces an AST
    /// directly without going through text serialization.
    pub fn traducir_ast(&mut self, ast: &mut Ast) -> BxResult<Vec<u8>> {
        // Phase 1: incluye — merge included files
        self.process_incluye(ast)?;

        // Phase 1.5: compute type layouts from TypeDecl
        self.compute_type_layouts(ast);

        // Phase 2: constant folding
        super::sema::fold::Folder::fold(ast);

        // Phase 3: semantic check
        let sema = Sema::new();
        sema.check(ast)?;

        // Phase 4: dead code elimination
        super::sema::dce::Dce::eliminate(ast);

        // Phase 5: optimization (inline, unused let elimination)
        super::sema::opt::Optimizer::optimize(ast);

        // Phase 6: codegen
        self.compilar_ast(ast)?;

        // Phase 6: back-patching
        self.backpatch()?;

        // Phase 7: assemble final output
        let mut final_bytes = Vec::new();
        final_bytes.extend_from_slice(self.backend.bytes_mut());
        final_bytes.extend_from_slice(&self.rodata);

        Ok(final_bytes)
    }

    fn process_incluye(&mut self, ast: &mut Ast) -> BxResult<()> {
        let old_items = core::mem::take(&mut ast.items);
        let mut new_items = Vec::new();
        for item in old_items {
            if let Stmt::Incluye(ref path) = item {
                if let Some(source) = self.file_registry.get(path) {
                    let source_clone = source.clone();
                    let mut parser = Parser::new(&source_clone);
                    let included_ast = parser.parse().map_err(|_e| BxError::InvalidArgument)?;
                    for item in included_ast.items {
                        new_items.push(item);
                    }
                }
            } else {
                new_items.push(item);
            }
        }
        ast.items = new_items;
        Ok(())
    }

    /// Walk the AST and record every `TypeDecl` as a layout in
    /// `type_layouts`. Field offsets are assigned in declaration
    /// order, aligned to 8 bytes (the natural Num alignment on x86_64).
    fn compute_type_layouts(&mut self, ast: &Ast) {
        for item in &ast.items {
            if let Stmt::TypeDecl { name, kind: _, fields } = item {
                let mut layout = Vec::new();
                let mut offset: i32 = 0;
                for (fname, fty) in fields {
                    let size = fty.size() as i32;
                    // Align to 8 bytes (Num boundary)
                    if offset % 8 != 0 {
                        offset += 8 - (offset % 8);
                    }
                    layout.push(FieldLayout {
                        name: fname.clone(),
                        offset,
                        size: size as u8,
                    });
                    offset += size;
                }
                self.type_layouts.insert(name.clone(), layout);
            }
        }
    }

    /// Resolve a field offset for a given type. Returns the byte
    /// offset of the field, or `None` if the type or field is unknown.
    fn field_offset(&self, type_name: &str, field_name: &str) -> Option<i32> {
        self.type_layouts
            .get(type_name)
            .and_then(|layout| layout.iter().find(|f| f.name == field_name))
            .map(|f| f.offset)
    }

    /// Look up the byte offset of a field on an object expression.
    ///
    /// The object is expected to be either a direct identifier in scope
    /// (whose declared type is `Type::Struct(name)`) or a chain of
    /// field accesses. For now only the first case is supported.
    fn lookup_field_offset(&self, obj: &Expr, field_name: &str) -> i32 {
        if let Expr::Ident(name) = obj {
            if let Some(entry) = self.scope.lookup(name) {
                if let Type::Struct(type_name) = &entry.ty {
                    if let Some(off) = self.field_offset(type_name, field_name) {
                        return off;
                    }
                }
            }
        }
        // Field-of-field or unknown: 0 is safe (treat as identity).
        0
    }

    fn backpatch(&mut self) -> BxResult<()> {
        let final_code_len = self.backend.here();
        for s_ref in &self.string_refs {
            self.backend.patch_string_ref(s_ref.disp_offset, s_ref.rodata_offset, final_code_len);
        }
        for patch in &self.call_patches {
            if let Some(entry) = self.fn_table.get(&patch.fn_name) {
                let code_offset = entry.code_offset;
                self.backend.patch_rel32(patch.call_offset, patch.call_offset + 4, code_offset);
            } else {
                return Err(BxError::InvalidArgument);
            }
        }
        for patch in &self.label_patches {
            if let Some(&target) = self.label_table.get(&patch.label_name) {
                self.backend.patch_rel32(patch.jmp_offset, patch.jmp_offset + 4, target);
            } else {
                return Err(BxError::InvalidArgument);
            }
        }
        Ok(())
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
                    self.compilar_funcion(params, ret.clone(), body)?;
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
                    ty: pty.clone(),
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
                            let disp_offset = match self.target {
                                TargetArch::X86_64 => {
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
                Stmt::Store { name, ty: _, value } => {
                    // Rebind: re-emit into the existing slot if known, else new
                    let existing_offset = self.scope.lookup(name).map(|e| e.frame_offset);
                    self.codegen_expr(value)?;
                    if let Some(off) = existing_offset {
                        self.backend.store_var(off);
                    } else {
                        // Implicit declaration on first store (e.g. loop counter)
                        let offset = -(self.frame_size as i32) - 8;
                        self.frame_size += 8;
                        self.backend.store_var(offset);
                        self.scope.push(ScopeEntry {
                            name: name.clone(),
                            ty: Type::Num,
                            frame_offset: offset,
                        });
                    }
                }
                Stmt::CallStmt { name, args } => {
                    for (i, arg) in args.iter().enumerate() {
                        if i >= self.backend.arg_reg_count() { return Err(BxError::InvalidArgument); }
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
                        self.call_patches.push(CallPatch { call_offset, fn_name: name.clone() });
                    }
                }
                Stmt::TypeDecl { .. } => {
                    // TypeDecl is metadata, not codegen target yet
                }
                Stmt::FieldAssign { obj, field, value } => {
                    // obj.field = value
                    // 1. Compute obj base into rax
                    self.codegen_expr(obj)?;
                    // 2. Find the type of obj to get the field offset
                    let offset = self.lookup_field_offset(obj, field);
                    // 3. Compute rax = rax + offset, save in rcx
                    {
                        let bytes = self.backend.bytes_mut();
                        if offset != 0 {
                            // add rax, offset (sign-extended imm32)
                            bytes.push(0x48);
                            bytes.push(0x05);
                            bytes.extend_from_slice(&(offset as i32).to_le_bytes());
                        }
                        bytes.extend_from_slice(&[0x48, 0x89, 0xC1]); // mov rcx, rax
                    }
                    // 4. Load value into rax
                    self.codegen_expr(value)?;
                    // 5. Store rax at [rcx]
                    {
                        let bytes = self.backend.bytes_mut();
                        bytes.extend_from_slice(&[0x48, 0x89, 0x01]); // mov [rcx], rax
                    }
                }
                Stmt::IndexAssign { obj, idx, value } => {
                    // obj[idx] = value
                    // Emit the address calculation in raw bytes to avoid
                    // borrow conflicts with codegen_expr.
                    self.codegen_expr(obj)?;
                    self.backend.push_acc();
                    self.codegen_expr(idx)?;
                    self.backend.push_acc();
                    // Stack now: [..., obj, idx]
                    // Compute: pop idx, shl 3, add with obj, store value at [rax]
                    {
                        let bytes = self.backend.bytes_mut();
                        bytes.push(0x59); // pop rcx (= idx)
                        bytes.extend_from_slice(&[0x48, 0xC1, 0xE1, 0x03]); // shl rcx, 3
                        bytes.push(0x5A); // pop rdx (= obj)
                        bytes.extend_from_slice(&[0x48, 0x01, 0xCA]); // add rdx, rcx
                    }
                    // rdx = obj + idx*8
                    self.codegen_expr(value)?;
                    // rax = value, store at [rdx]
                    {
                        let bytes = self.backend.bytes_mut();
                        bytes.extend_from_slice(&[0x48, 0x89, 0x02]); // mov [rdx], rax
                    }
                }
                Stmt::Retorna(expr_opt) => {
                    if let Some(expr) = expr_opt {
                        self.codegen_expr(expr)?;
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
                Stmt::Rompe => { self.compilar_rompe()?; }
                Stmt::Continua => { self.compilar_continua()?; }
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
                    self.compilar_libre(expr)?;
                }
                Stmt::Barr => {
                    self.compilar_barr()?;
                }
                Stmt::Cuando { flag, body } => {
                    self.compilar_cuando(flag, body, None)?;
                }
                Stmt::CuandoSino { flag, then_body, else_body } => {
                    self.compilar_cuando(flag, then_body, else_body.as_deref())?;
                }
                Stmt::Atomico(body) => {
                    self.compilar_atomico(body)?;
                }
                Stmt::Volatil(expr) => {
                    self.compilar_volatil(expr)?;
                }
                Stmt::Incluye(_) => {}
                Stmt::Def { .. } => {
                    // Nested function definitions not supported at this level
                }
            }
        }
        Ok(())
    }

    // ── cuando (CPU flags) ─────────────────────────────────────────

    fn compilar_cuando(
        &mut self,
        flag: &CpuFlag,
        then_body: &[Stmt],
        else_body: Option<&[Stmt]>,
    ) -> BxResult<()> {
        // Emit a "test" for the flag, then conditional branch
        match self.target {
            TargetArch::X86_64 => {
                match flag {
                    CpuFlag::Zf => {
                        // TEST eax, eax; JZ else_body
                        let bytes = self.backend.bytes_mut();
                        bytes.extend_from_slice(&[0x85, 0xC0]); // test eax, eax
                    }
                    CpuFlag::Cf => {
                        // STC; JB else_body (carry set = unsigned overflow)
                        let bytes = self.backend.bytes_mut();
                        bytes.extend_from_slice(&[0xF9]); // STC
                    }
                    CpuFlag::Sf => {
                        // TEST eax, eax; JNS else_body
                        let bytes = self.backend.bytes_mut();
                        bytes.extend_from_slice(&[0x85, 0xC0]); // test eax, eax
                    }
                    CpuFlag::Of => {
                        // JO else_body (needs prior arithmetic to set OF)
                        let bytes = self.backend.bytes_mut();
                        bytes.extend_from_slice(&[0x85, 0xC0]); // test eax, eax
                    }
                    _ => {
                        let bytes = self.backend.bytes_mut();
                        bytes.extend_from_slice(&[0x85, 0xC0]); // test eax, eax
                    }
                }
                let jelse = if else_body.is_some() {
                    let jelse = self.backend.je_rel32();
                    // Patch: JZ → JNZ (for zf), or use proper Jcc
                    Some(jelse)
                } else {
                    None
                };

                self.compilar_body(then_body)?;

                if let Some(eb) = else_body {
                    let jend = self.backend.jmp_rel32();
                    let else_start = self.backend.here();
                    if let Some(je) = jelse {
                        self.backend.patch_rel32(je, je + 4, else_start);
                    }
                    self.compilar_body(eb)?;
                    let end = self.backend.here();
                    self.backend.patch_rel32(jend, jend + 4, end);
                } else {
                    let end = self.backend.here();
                    if let Some(je) = jelse {
                        self.backend.patch_rel32(je, je + 4, end);
                    }
                }
            }
            _ => {
                // Non-x86: just emit the body (flag check is architecture-specific)
                self.compilar_body(then_body)?;
            }
        }
        Ok(())
    }

    // ── atomico ────────────────────────────────────────────────────

    fn compilar_atomico(&mut self, body: &[Stmt]) -> BxResult<()> {
        match self.target {
            TargetArch::X86_64 => {
                // LOCK prefix (0xF0) emitted before each instruction in the body
                // For simplicity, we emit LOCK before the first instruction
                let bytes = self.backend.bytes_mut();
                bytes.push(0xF0); // LOCK prefix
                self.compilar_body(body)?;
            }
            _ => {
                // ARM: LDREX/STREX patterns — for now just emit body
                self.compilar_body(body)?;
            }
        }
        Ok(())
    }

    // ── volatil ────────────────────────────────────────────────────

    fn compilar_volatil(&mut self, expr: &Expr) -> BxResult<()> {
        self.codegen_expr(expr)?;
        // Volatile: add MFENCE before to prevent reordering
        if self.target == TargetArch::X86_64 {
            if let Some(bytes) = self.backend.intrinsic_bytes("mfence") {
                self.backend.emit_bytes(bytes);
            }
        }
        Ok(())
    }

    // ── barr (memory barrier) ──────────────────────────────────────

    fn compilar_barr(&mut self) -> BxResult<()> {
        match self.target {
            TargetArch::X86_64 => {
                if let Some(bytes) = self.backend.intrinsic_bytes("mfence") {
                    self.backend.emit_bytes(bytes);
                }
            }
            TargetArch::Aarch64 => {
                // DMB ISH
                let inst: u32 = 0xD5033BBF;
                self.backend.bytes_mut().extend_from_slice(&inst.to_le_bytes());
            }
            TargetArch::Riscv64 => {
                // FENCE rw, rww
                let inst: u32 = 0x0FF0000F;
                self.backend.bytes_mut().extend_from_slice(&inst.to_le_bytes());
            }
        }
        Ok(())
    }

    // ── libre ──────────────────────────────────────────────────────

    fn compilar_libre(&mut self, expr: &Expr) -> BxResult<()> {
        self.codegen_expr(expr)?;
        let bytes = self.backend.bytes_mut();
        bytes.extend_from_slice(&[0x48, 0x83, 0xE8, 0x08]); // sub rax, 8
        bytes.extend_from_slice(&[0x48, 0x89, 0xC7]); // mov rdi, rax
        bytes.extend_from_slice(&[0x48, 0x8B, 0x30]); // mov rsi, [rax]
        bytes.extend_from_slice(&[0x48, 0x81, 0xC6, 0xFF, 0x0F, 0x00, 0x00]); // add rsi, 4095
        bytes.extend_from_slice(&[0x48, 0xC1, 0xEE, 0x0C]); // shr rsi, 12
        bytes.push(0xFF);
        bytes.push(0x15);
        let func_ptr_offset = self.rodata.len();
        let func_addr = crate::arch::page_alloc::free_pages as *const () as usize as u64;
        self.rodata.extend_from_slice(&func_addr.to_le_bytes());
        let disp_offset = bytes.len();
        bytes.extend_from_slice(&[0, 0, 0, 0]);
        self.string_refs.push(StringRef { disp_offset, rodata_offset: func_ptr_offset });
        Ok(())
    }

    // ── Control flow ───────────────────────────────────────────────

    fn compilar_si(
        &mut self, cond: &Expr, then_body: &[Stmt], else_body: Option<&[Stmt]>,
    ) -> BxResult<()> {
        self.codegen_expr(cond)?;
        self.backend.test_acc();
        let jelse = if else_body.is_some() { Some(self.backend.je_rel32()) } else { None };
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
        self.loop_stack.push(LoopContext { break_patches: Vec::new(), continue_patches: Vec::new() });
        self.compilar_body(body)?;
        let ctx = self.loop_stack.pop().unwrap();
        for patch in &ctx.continue_patches { self.backend.patch_rel32(*patch, *patch + 4, loop_start); }
        let jback = self.backend.jmp_rel32();
        self.backend.patch_rel32(jback, jback + 4, loop_start);
        let loop_end = self.backend.here();
        self.backend.patch_rel32(jend, jend + 4, loop_end);
        for patch in &ctx.break_patches { self.backend.patch_rel32(*patch, *patch + 4, loop_end); }
        Ok(())
    }

    fn compilar_rompe(&mut self) -> BxResult<()> {
        let jmp = self.backend.jmp_rel32();
        self.loop_stack.last_mut().ok_or(BxError::InvalidArgument)?.break_patches.push(jmp);
        Ok(())
    }

    fn compilar_continua(&mut self) -> BxResult<()> {
        let jmp = self.backend.jmp_rel32();
        self.loop_stack.last_mut().ok_or(BxError::InvalidArgument)?.continue_patches.push(jmp);
        Ok(())
    }

    fn compilar_match(
        &mut self, expr: &Expr, arms: &[(Expr, Vec<Stmt>)], default: Option<&[Stmt]>,
    ) -> BxResult<()> {
        let mut end_patches: Vec<usize> = Vec::new();
        for (_pattern, body) in arms {
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
        if let Some(def) = default { self.compilar_body(def)?; }
        let end = self.backend.here();
        for patch in &end_patches { self.backend.patch_rel32(*patch, *patch + 4, end); }
        Ok(())
    }

    fn compilar_para(
        &mut self, var: &str, desde: &Expr, hasta: &Expr, paso: Option<&Expr>, body: &[Stmt],
    ) -> BxResult<()> {
        self.codegen_expr(hasta)?;
        let hasta_offset = -(self.frame_size as i32) - 8;
        self.frame_size += 8;
        self.backend.store_var(hasta_offset);

        let paso_offset = -(self.frame_size as i32) - 8;
        self.frame_size += 8;
        if let Some(p) = paso { self.codegen_expr(p)?; } else { self.backend.mov_acc_imm(1); }
        self.backend.store_var(paso_offset);

        self.codegen_expr(desde)?;
        let var_offset = -(self.frame_size as i32) - 8;
        self.frame_size += 8;
        self.backend.store_var(var_offset);
        self.scope.push(ScopeEntry { name: alloc::string::ToString::to_string(var), ty: Type::Num, frame_offset: var_offset });

        let loop_start = self.backend.here();
        self.backend.load_var(var_offset);
        let scratch = self.backend.scratch_reg();
        self.backend.mov_reg_acc(scratch);
        self.backend.load_var(hasta_offset);
        self.backend.cmp_lt_acc(scratch);
        self.backend.test_acc();
        let jend = self.backend.je_rel32();

        self.loop_stack.push(LoopContext { break_patches: Vec::new(), continue_patches: Vec::new() });
        self.compilar_body(body)?;
        let ctx = self.loop_stack.pop().unwrap();

        for patch in &ctx.continue_patches { self.backend.patch_rel32(*patch, *patch + 4, loop_start); }

        self.backend.load_var(var_offset);
        self.backend.mov_reg_acc(scratch);
        self.backend.load_var(paso_offset);
        self.backend.add_acc(scratch);
        self.backend.store_var(var_offset);

        let jback = self.backend.jmp_rel32();
        self.backend.patch_rel32(jback, jback + 4, loop_start);
        let loop_end = self.backend.here();
        self.backend.patch_rel32(jend, jend + 4, loop_end);
        for patch in &ctx.break_patches { self.backend.patch_rel32(*patch, *patch + 4, loop_end); }
        Ok(())
    }

    fn compilar_bucle(&mut self, body: &[Stmt]) -> BxResult<()> {
        let loop_start = self.backend.here();
        self.loop_stack.push(LoopContext { break_patches: Vec::new(), continue_patches: Vec::new() });
        self.compilar_body(body)?;
        let ctx = self.loop_stack.pop().unwrap();
        let jback = self.backend.jmp_rel32();
        self.backend.patch_rel32(jback, jback + 4, loop_start);
        let loop_end = self.backend.here();
        for patch in &ctx.break_patches { self.backend.patch_rel32(*patch, *patch + 4, loop_end); }
        for patch in &ctx.continue_patches { self.backend.patch_rel32(*patch, *patch + 4, loop_start); }
        Ok(())
    }

    fn compilar_etiqueta(&mut self, name: &str) -> BxResult<()> {
        let offset = self.backend.here();
        self.label_table.insert(alloc::string::ToString::to_string(name), offset);
        Ok(())
    }

    fn compilar_salto(&mut self, name: &str) -> BxResult<()> {
        let jmp_offset = self.backend.jmp_rel32();
        if let Some(&target) = self.label_table.get(name) {
            self.backend.patch_rel32(jmp_offset, jmp_offset + 4, target);
        } else {
            self.label_patches.push(LabelPatch { jmp_offset, label_name: alloc::string::ToString::to_string(name) });
        }
        Ok(())
    }

    // ── Expression codegen ─────────────────────────────────────────

    fn codegen_expr(&mut self, expr: &Expr) -> BxResult<()> {
        match expr {
            Expr::LitInt(imm) => { self.backend.mov_acc_imm(*imm); }
            Expr::LitByte(b) => {
                self.backend.zero_acc();
                let bytes = self.backend.bytes_mut();
                bytes.push(0xB0);
                bytes.push(*b);
            }
            Expr::Reg(r_name) => {
                let r = self.backend.parse_reg(r_name).ok_or(BxError::InvalidArgument)?;
                if r != self.backend.acc_reg() { self.backend.mov_acc_reg(r); }
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
                        self.backend.cmp_lt_acc(scratch);
                        self.backend.test_acc();
                        self.backend.sete_acc();
                    }
                    BinOp::MenIg => {
                        self.backend.cmp_gt_acc(scratch);
                        self.backend.test_acc();
                        self.backend.sete_acc();
                    }
                    BinOp::Difer => {
                        self.backend.cmp_eq_acc(scratch);
                        self.backend.test_acc();
                        self.backend.sete_acc();
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
                self.codegen_expr(size_expr)?;
                let bytes = self.backend.bytes_mut();
                bytes.push(0x50);
                bytes.extend_from_slice(&[0x48, 0x05, 0xFF, 0x0F, 0x00, 0x00]);
                bytes.extend_from_slice(&[0x48, 0xC1, 0xE8, 0x0C]);
                bytes.extend_from_slice(&[0x48, 0xFF, 0xC0]);
                bytes.extend_from_slice(&[0x48, 0x89, 0xC7]);
                bytes.push(0xFF);
                bytes.push(0x15);
                let func_ptr_offset = self.rodata.len();
                let func_addr = crate::arch::page_alloc::alloc_pages_contiguous as *const () as usize as u64;
                self.rodata.extend_from_slice(&func_addr.to_le_bytes());
                let disp_offset = bytes.len();
                bytes.extend_from_slice(&[0, 0, 0, 0]);
                self.string_refs.push(StringRef { disp_offset, rodata_offset: func_ptr_offset });
                bytes.extend_from_slice(&[0x59]);
                bytes.extend_from_slice(&[0x48, 0x89, 0x08]);
                bytes.extend_from_slice(&[0x48, 0x83, 0xC0, 0x08]);
            }
            Expr::Call { name, args } => {
                for (i, arg) in args.iter().enumerate() {
                    if i >= self.backend.arg_reg_count() { return Err(BxError::InvalidArgument); }
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
                    self.call_patches.push(CallPatch { call_offset, fn_name: name.clone() });
                }
            }
            Expr::LitNulo => { self.backend.zero_acc(); }
            Expr::LitStr(_) => { self.backend.zero_acc(); }
            Expr::Flag(flag) => {
                self.codegen_flag_read(flag)?;
            }
            Expr::MemOrder(mo, inner) => {
                self.codegen_memorder(*mo, inner)?;
            }
            Expr::Field { obj, name } => {
                // Struct field read: load obj base, add field offset, load 8 bytes
                self.codegen_expr(obj)?;
                let offset = self.lookup_field_offset(obj, name);
                {
                    let bytes = self.backend.bytes_mut();
                    if offset != 0 {
                        // add rax, offset (sign-extended imm32)
                        bytes.push(0x48);
                        bytes.push(0x05);
                        bytes.extend_from_slice(&(offset as i32).to_le_bytes());
                    }
                    // mov rax, [rax]
                    bytes.extend_from_slice(&[0x48, 0x8B, 0x00]);
                }
            }
            Expr::Index { obj, idx } => {
                // Array index read: load obj, compute obj+idx*8, load
                self.codegen_expr(obj)?;
                self.backend.push_acc();
                self.codegen_expr(idx)?;
                self.backend.push_acc();
                // Stack: [..., obj, idx]
                {
                    let bytes = self.backend.bytes_mut();
                    bytes.push(0x59); // pop rcx (= idx)
                    bytes.extend_from_slice(&[0x48, 0xC1, 0xE1, 0x03]); // shl rcx, 3
                    bytes.push(0x5A); // pop rdx (= obj)
                    bytes.extend_from_slice(&[0x48, 0x01, 0xCA]); // add rdx, rcx
                    bytes.extend_from_slice(&[0x48, 0x8B, 0x02]); // mov rax, [rdx]
                }
            }
            Expr::AddrOf(inner) => {
                // &x → load frame address of x
                if let Expr::Ident(name) = &**inner {
                    if let Some(entry) = self.scope.lookup(name) {
                        // rax = rbp + frame_offset (lea)
                        let bytes = self.backend.bytes_mut();
                        bytes.push(0x48);
                        bytes.push(0x8D);
                        bytes.push(0x85);
                        bytes.extend_from_slice(&(entry.frame_offset as u32).to_le_bytes());
                    } else {
                        // Unknown — emit 0
                        self.backend.zero_acc();
                    }
                } else {
                    // Not an ident — fall through to value
                    self.codegen_expr(inner)?;
                }
            }
            Expr::Deref(inner) => {
                // *x → load 8 bytes from address
                self.codegen_expr(inner)?;
                let bytes = self.backend.bytes_mut();
                bytes.push(0x48);
                bytes.push(0x8B);
                bytes.push(0x00);
            }
            Expr::Cast(inner, _ty) => {
                // For now, cast is identity (no-op)
                self.codegen_expr(inner)?;
            }
        }
        Ok(())
    }

    fn codegen_flag_read(&mut self, flag: &CpuFlag) -> BxResult<()> {
        match self.target {
            TargetArch::X86_64 => {
                self.backend.zero_acc();
                let setcc = match flag {
                    CpuFlag::Cf => 0x92u8,
                    CpuFlag::Zf => 0x94u8,
                    CpuFlag::Sf => 0x98u8,
                    CpuFlag::Of => 0x90u8,
                    CpuFlag::Pf => 0x9Au8,
                    CpuFlag::Df => 0x9Cu8,
                };
                let bytes = self.backend.bytes_mut();
                bytes.push(0x0F);
                bytes.push(setcc);
                bytes.push(0xC0);
            }
            _ => {
                self.backend.zero_acc();
            }
        }
        Ok(())
    }

    fn codegen_memorder(&mut self, mo: MemOrder, inner: &Expr) -> BxResult<()> {
        match mo {
            MemOrder::Volatil => {
                self.codegen_expr(inner)?;
                if self.target == TargetArch::X86_64 {
                    if let Some(b) = self.backend.intrinsic_bytes("mfence") {
                        self.backend.emit_bytes(b);
                    }
                }
            }
            MemOrder::Acquire => {
                self.codegen_expr(inner)?;
                match self.target {
                    TargetArch::X86_64 => {
                        if let Some(b) = self.backend.intrinsic_bytes("lfence") {
                            self.backend.emit_bytes(b);
                        }
                    }
                    TargetArch::Aarch64 => {
                        let inst: u32 = 0xD50339DF; // LDAR
                        self.backend.bytes_mut().extend_from_slice(&inst.to_le_bytes());
                    }
                    TargetArch::Riscv64 => {
                        let inst: u32 = 0x0000000F; // FENCE r,rw
                        self.backend.bytes_mut().extend_from_slice(&inst.to_le_bytes());
                    }
                }
            }
            MemOrder::Release => {
                self.codegen_expr(inner)?;
                match self.target {
                    TargetArch::X86_64 => {
                        if let Some(b) = self.backend.intrinsic_bytes("sfence") {
                            self.backend.emit_bytes(b);
                        }
                    }
                    TargetArch::Aarch64 => {
                        let inst: u32 = 0x0800009F; // STLR
                        self.backend.bytes_mut().extend_from_slice(&inst.to_le_bytes());
                    }
                    TargetArch::Riscv64 => {
                        let inst: u32 = 0x0000000F; // FENCE rw,w
                        self.backend.bytes_mut().extend_from_slice(&inst.to_le_bytes());
                    }
                }
            }
            MemOrder::Relaxed => {
                self.codegen_expr(inner)?;
            }
            MemOrder::Fence => {
                self.compilar_barr()?;
            }
        }
        Ok(())
    }
}

impl Default for Traductor {
    fn default() -> Self { Self::new() }
}
