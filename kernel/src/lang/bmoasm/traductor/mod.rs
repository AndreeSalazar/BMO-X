//! Traductor central de BMO Simple.
//! Coordina el Lexer, Parser, Sema y Emitter para producir bytes nativos del target.
//! Implementa la resolución semántica de cadenas literales y llamadas a funciones
//! con la BMO ABI (7 GPR args + 64B align + RAX status).

extern crate alloc;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use crate::barex::{BxError, BxResult};
use super::parser::{Parser, Ast, Stmt, Expr, BinOp, Type};
use super::sema::Sema;
use super::sema::scope::{Scope, ScopeEntry};
use super::emit::{TargetArch, TargetEmitter, TargetRegister, Reg64};
use super::builtin::{IntrinsicId, emit_intrinsic};

struct StringRef {
    disp_offset: usize,
    rodata_offset: usize,
}

struct LoopContext {
    break_patches: Vec<usize>,
    continue_patches: Vec<usize>,
}

/// Información de una función compilada (para back-patching de calls).
struct FunctionEntry {
    name: String,
    /// Offset de inicio del código de la función (después del prologue).
    code_offset: usize,
    /// Número de parámetros.
    param_count: usize,
}

/// Instrucciones pendientes de parchear (calls forward).
struct CallPatch {
    /// Offset del displacement32 en la instrucción call.
    call_offset: usize,
    /// Nombre de la función a llamar.
    fn_name: String,
}

/// BMO ABI register argument order.
const BMO_ARG_REGS_X86: [Reg64; 7] = [
    Reg64::Rdi, Reg64::Rsi, Reg64::Rdx,
    Reg64::R10, Reg64::R8,  Reg64::R9,
    Reg64::Rax,
];

pub struct Traductor {
    target: TargetArch,
    emitter: TargetEmitter,
    rodata: Vec<u8>,
    string_refs: Vec<StringRef>,
    loop_stack: Vec<LoopContext>,
    scope: Scope,
    frame_size: u32,
    /// Tabla de funciones compiladas (name → offset).
    fn_table: BTreeMap<String, FunctionEntry>,
    /// Patches pendientes para calls forward.
    call_patches: Vec<CallPatch>,
}

impl Traductor {
    pub fn new() -> Self {
        Self::with_target(TargetArch::X86_64)
    }

    pub fn with_target(target: TargetArch) -> Self {
        Self {
            target,
            emitter: TargetEmitter::new(target),
            rodata: Vec::new(),
            string_refs: Vec::new(),
            loop_stack: Vec::new(),
            scope: Scope::default(),
            frame_size: 0,
            fn_table: BTreeMap::new(),
            call_patches: Vec::new(),
        }
    }

    /// Traduce código fuente en español de BMO Simple a bytes nativos del target.
    pub fn traducir(&mut self, src: &[u8]) -> BxResult<Vec<u8>> {
        // 1. Parser
        let mut parser = Parser::new(src);
        let ast = parser.parse()?;

        // 2. Análisis Semántico (Sema)
        let sema = Sema::new();
        sema.check(&ast)?;

        // 3. Generación de Código (dos pasadas)
        self.compilar_ast(&ast)?;

        // 4. Back-patching de strings
        let final_code_len = self.emitter.bytes().len();
        for s_ref in &self.string_refs {
            match &mut self.emitter {
                TargetEmitter::X86_64(e) => {
                    e.patch_string_ref(s_ref.disp_offset, s_ref.rodata_offset, final_code_len);
                }
                TargetEmitter::Aarch64(e) => {
                    e.patch_string_ref(s_ref.disp_offset, s_ref.rodata_offset, final_code_len);
                }
                TargetEmitter::Riscv64(e) => {
                    e.patch_string_ref(s_ref.disp_offset, s_ref.rodata_offset, final_code_len);
                }
            }
        }

        // 5. Back-patching de calls forward
        for patch in &self.call_patches {
            if let Some(entry) = self.fn_table.get(&patch.fn_name) {
                let code_offset = entry.code_offset;
                match &mut self.emitter {
                    TargetEmitter::X86_64(e) => {
                        e.patch_rel32(patch.call_offset, patch.call_offset + 4, code_offset);
                    }
                    _ => {}
                }
            } else {
                return Err(BxError::InvalidArgument); // Undefined function
            }
        }

        // 6. Concatena código + rodata
        let mut final_bytes = Vec::new();
        final_bytes.extend_from_slice(self.emitter.bytes());
        final_bytes.extend_from_slice(&self.rodata);

        Ok(final_bytes)
    }

    fn compilar_ast(&mut self, ast: &Ast) -> BxResult<()> {
        // Primera pasada: compilar todas las funciones y registrar offsets
        for item in &ast.items {
            match item {
                Stmt::Def { name, params, ret, body } => {
                    let code_offset = match &self.emitter {
                        TargetEmitter::X86_64(e) => e.here(),
                        _ => 0,
                    };
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

        // Reserve space for parameters on stack (BMO ABI: args arrive in regs)
        let param_space = (params.len() * 8) as u32;
        self.frame_size = param_space;

        // Prologue: push rbp; mov rbp, rsp; sub rsp, imm32 (placeholder)
        let sub_rsp_offset = match &mut self.emitter {
            TargetEmitter::X86_64(e) => {
                e.push_rbp();
                e.mov_rbp_rsp();
                let off = e.here();
                e.sub_rsp_imm32(0); // placeholder
                off
            }
            _ => 0,
        };

        // Save BMO ABI argument registers into stack frame slots
        for (i, _param) in params.iter().enumerate() {
            if i < BMO_ARG_REGS_X86.len() {
                // Stack layout: [rbp-8] = param0, [rbp-16] = param1, ...
                let offset = -((i as i32 + 1) * 8);
                match &mut self.emitter {
                    TargetEmitter::X86_64(e) => {
                        // Move arg reg → RAX → stack slot
                        e.mov_reg_reg(Reg64::Rax, BMO_ARG_REGS_X86[i]);
                        if offset >= -128 && offset <= 127 {
                            e.mov_rbp_disp8_rax(offset as i8);
                        } else {
                            e.mov_rbp_disp32_rax(offset);
                        }
                    }
                    _ => {}
                }
                // Register the param in the scope for ident lookup
                self.scope.push(ScopeEntry {
                    name: _param.0.clone(),
                    ty: _param.1,
                    frame_offset: -((i as i32 + 1) * 8),
                });
            }
        }

        // Emit the body
        self.compilar_body(body)?;

        // Back-patch sub rsp, N with the real frame size
        let aligned = (self.frame_size + 15) & !15; // 16-byte align
        match &mut self.emitter {
            TargetEmitter::X86_64(e) => {
                let imm_bytes = (aligned as i32).to_le_bytes();
                e.bytes[sub_rsp_offset + 3] = imm_bytes[0];
                e.bytes[sub_rsp_offset + 4] = imm_bytes[1];
                e.bytes[sub_rsp_offset + 5] = imm_bytes[2];
                e.bytes[sub_rsp_offset + 6] = imm_bytes[3];
                // Epilogue
                e.leave();
                e.ret();
            }
            _ => {}
        }

        Ok(())
    }

    fn compilar_body(&mut self, body: &[Stmt]) -> BxResult<()> {
        for stmt in body {
            match stmt {
                Stmt::RegAssign { reg, value } => {
                    let dst_reg = TargetRegister::from_name(self.target, reg).ok_or(BxError::InvalidArgument)?;
                    // General codegen: result goes into RAX via codegen_expr_x86
                    match value {
                        Expr::LitInt(imm) => {
                            match (&mut self.emitter, dst_reg) {
                                (TargetEmitter::X86_64(e), TargetRegister::X86_64(r)) => e.mov_reg_imm64(r, *imm),
                                (TargetEmitter::Aarch64(e), TargetRegister::Aarch64(r)) => e.mov_reg_imm64(r, *imm),
                                (TargetEmitter::Riscv64(e), TargetRegister::Riscv64(r)) => e.mov_reg_imm64(r, *imm),
                                _ => return Err(BxError::InvalidArgument),
                            }
                        }
                        Expr::LitStr(s) => {
                            // Almacenar el string en rodata
                            let rodata_offset = self.rodata.len();
                            self.rodata.extend_from_slice(s.as_bytes());
                            self.rodata.push(0); // Null terminator para FFI/BMO C-strings si se requiere
                            
                            // Emitir LEA/ADR con placeholder
                            let disp_offset = match (&mut self.emitter, dst_reg) {
                                (TargetEmitter::X86_64(e), TargetRegister::X86_64(r)) => e.lea_reg_rip_placeholder(r),
                                (TargetEmitter::Aarch64(e), TargetRegister::Aarch64(r)) => e.lea_reg_rip_placeholder(r),
                                (TargetEmitter::Riscv64(e), TargetRegister::Riscv64(r)) => e.lea_reg_rip_placeholder(r),
                                _ => return Err(BxError::InvalidArgument),
                            };
                            self.string_refs.push(StringRef {
                                disp_offset,
                                rodata_offset,
                            });
                        }
                        Expr::Reg(src_reg_name) => {
                            let src_reg = TargetRegister::from_name(self.target, src_reg_name).ok_or(BxError::InvalidArgument)?;
                            match (&mut self.emitter, dst_reg, src_reg) {
                                (TargetEmitter::X86_64(e), TargetRegister::X86_64(rd), TargetRegister::X86_64(rs)) => e.mov_reg_reg(rd, rs),
                                (TargetEmitter::Aarch64(e), TargetRegister::Aarch64(rd), TargetRegister::Aarch64(rs)) => e.mov_reg_reg(rd, rs),
                                (TargetEmitter::Riscv64(e), TargetRegister::Riscv64(rd), TargetRegister::Riscv64(rs)) => e.mov_reg_reg(rd, rs),
                                _ => return Err(BxError::InvalidArgument),
                            }
                        }
                        // Ident and other expressions: codegen to RAX, then move to dst_reg
                        _ => {
                            self.codegen_expr_x86(value)?;
                            if dst_reg != TargetRegister::X86_64(Reg64::Rax) {
                                match (&mut self.emitter, dst_reg) {
                                    (TargetEmitter::X86_64(e), TargetRegister::X86_64(r)) => {
                                        e.mov_reg_reg(r, Reg64::Rax);
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
                Stmt::Let { name, ty: _, value } => {
                    // codegen value -> RAX
                    self.codegen_expr_x86(value)?;
                    // Allocate 8 bytes on stack for this variable
                    let offset = -(self.frame_size as i32) - 8;
                    self.frame_size += 8;
                    // Store RAX to [rbp + offset]
                    match &mut self.emitter {
                        TargetEmitter::X86_64(e) => {
                            if offset >= -128 && offset <= 127 {
                                e.mov_rbp_disp8_rax(offset as i8);
                            } else {
                                e.mov_rbp_disp32_rax(offset);
                            }
                        }
                        _ => return Err(BxError::Unsupported),
                    }
                    // Push to scope for future lookups
                    self.scope.push(ScopeEntry {
                        name: name.clone(),
                        ty: super::parser::ast::Type::Num,
                        frame_offset: offset,
                    });
                }
                Stmt::Retorna(expr_opt) => {
                    if let Some(expr) = expr_opt {
                        self.codegen_expr_x86(expr)?;
                        // Ensure result is in RAX for return value
                        match (&mut self.emitter, &expr) {
                            (TargetEmitter::X86_64(e), Expr::Reg(r_name)) => {
                                if let Some(r) = Reg64::from_name(r_name) {
                                    if r != Reg64::Rax {
                                        e.mov_reg_reg(Reg64::Rax, r);
                                    }
                                }
                            }
                            (TargetEmitter::Aarch64(e), Expr::Reg(r_name)) => {
                                if let Some(r) = TargetRegister::from_name(self.target, r_name) {
                                    if let TargetRegister::Aarch64(a) = r {
                                        if a != super::emit::aarch64::RegArm::X0 {
                                            e.mov_reg_reg(super::emit::aarch64::RegArm::X0, a);
                                        }
                                    }
                                }
                            }
                            (TargetEmitter::Riscv64(e), Expr::Reg(r_name)) => {
                                if let Some(r) = TargetRegister::from_name(self.target, r_name) {
                                    if let TargetRegister::Riscv64(rv) = r {
                                        if rv != super::emit::riscv::RegRiscv::A0 {
                                            e.mov_reg_reg(super::emit::riscv::RegRiscv::A0, rv);
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    match &mut self.emitter {
                        TargetEmitter::X86_64(e) => e.leave(),
                        TargetEmitter::Aarch64(e) => e.ret(),
                        TargetEmitter::Riscv64(e) => e.ret(),
                    }
                }
                Stmt::Emit(raw_bytes) => {
                    match &mut self.emitter {
                        TargetEmitter::X86_64(e) => e.emit_raw(raw_bytes),
                        TargetEmitter::Aarch64(e) => e.emit_raw(raw_bytes),
                        TargetEmitter::Riscv64(e) => e.emit_raw(raw_bytes),
                    }
                }
                Stmt::ExprStmt(Expr::Reg(r_name)) => {
                    // Intrínsecos directos
                    if r_name == "syscall" {
                        match &mut self.emitter {
                            TargetEmitter::X86_64(e) => e.syscall(),
                            TargetEmitter::Aarch64(e) => e.syscall(),
                            TargetEmitter::Riscv64(e) => e.syscall(),
                        }
                    } else if r_name == "nop" {
                        match &mut self.emitter {
                            TargetEmitter::X86_64(e) => e.nop(),
                            TargetEmitter::Aarch64(e) => e.nop(),
                            TargetEmitter::Riscv64(e) => e.nop(),
                        }
                    } else if let Some(intrinsic) = self.map_intrinsic_name(r_name) {
                        match &mut self.emitter {
                            TargetEmitter::X86_64(e) => emit_intrinsic(e, intrinsic)?,
                            _ => return Err(BxError::Unsupported),
                        }
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
                    // General expression statement (function calls, etc.)
                    self.codegen_expr_x86(expr)?;
                }
                Stmt::FnForward { .. } => {
                    // Forward declarations are processed during AST compilation
                }
                _ => return Err(BxError::Unsupported),
            }
        }
        Ok(())
    }

    fn map_intrinsic_name(&self, name: &str) -> Option<IntrinsicId> {
        match name {
            "pausa" => Some(IntrinsicId::Pausa),
            "int3" => Some(IntrinsicId::Int3),
            "hlt" => Some(IntrinsicId::Hlt),
            "cli" => Some(IntrinsicId::Cli),
            "sti" => Some(IntrinsicId::Sti),
            "rdtsc" => Some(IntrinsicId::Rdtsc),
            "cpuid" => Some(IntrinsicId::Cpuid),
            "lfence" => Some(IntrinsicId::Lfence),
            "mfence" => Some(IntrinsicId::Mfence),
            "sfence" => Some(IntrinsicId::Sfence),
            _ => None,
        }
    }

    // ── Control flow codegen (x86-64) ───────────────────────────────

    fn compilar_si(
        &mut self,
        cond: &Expr,
        then_body: &[Stmt],
        else_body: Option<&[Stmt]>,
    ) -> BxResult<()> {
        // codegen cond -> RAX
        self.codegen_expr_x86(cond)?;
        // Emit test + conditional jump
        let (jelse, jend) = {
            match &mut self.emitter {
                TargetEmitter::X86_64(e) => {
                    e.test_rax_rax();
                    if else_body.is_some() {
                        let jelse = e.je_rel32();
                        (Some(jelse), None)
                    } else {
                        (None, Some(e.je_rel32()))
                    }
                }
                _ => return Err(BxError::Unsupported),
            }
        };
        // Emit then body (borrow dropped)
        self.compilar_body(then_body)?;
        if let Some(eb) = else_body {
            // Emit jmp past else
            let jend = {
                match &mut self.emitter {
                    TargetEmitter::X86_64(e) => e.jmp_rel32(),
                    _ => return Err(BxError::Unsupported),
                }
            };
            // Patch je to else start
            {
                match &mut self.emitter {
                    TargetEmitter::X86_64(e) => {
                        let else_start = e.here();
                        e.patch_rel32(jelse.unwrap(), jelse.unwrap() + 4, else_start);
                    }
                    _ => return Err(BxError::Unsupported),
                }
            }
            // Emit else body
            self.compilar_body(eb)?;
            // Patch jmp past else
            {
                match &mut self.emitter {
                    TargetEmitter::X86_64(e) => {
                        let end = e.here();
                        e.patch_rel32(jend, jend + 4, end);
                    }
                    _ => return Err(BxError::Unsupported),
                }
            }
        } else {
            // Patch je to end
            match &mut self.emitter {
                TargetEmitter::X86_64(e) => {
                    let end = e.here();
                    e.patch_rel32(jend.unwrap(), jend.unwrap() + 4, end);
                }
                _ => return Err(BxError::Unsupported),
            }
        }
        Ok(())
    }

    fn compilar_mientras(&mut self, cond: &Expr, body: &[Stmt]) -> BxResult<()> {
        let loop_start = {
            match &mut self.emitter {
                TargetEmitter::X86_64(e) => e.here(),
                _ => return Err(BxError::Unsupported),
            }
        };
        // codegen cond -> RAX
        self.codegen_expr_x86(cond)?;
        let jend = {
            match &mut self.emitter {
                TargetEmitter::X86_64(e) => {
                    e.test_rax_rax();
                    e.je_rel32()
                }
                _ => return Err(BxError::Unsupported),
            }
        };
        self.loop_stack.push(LoopContext {
            break_patches: Vec::new(),
            continue_patches: Vec::new(),
        });
        self.compilar_body(body)?;
        let ctx = self.loop_stack.pop().unwrap();
        // continue -> jump to loop_start
        {
            match &mut self.emitter {
                TargetEmitter::X86_64(e) => {
                    for patch in &ctx.continue_patches {
                        e.patch_rel32(*patch, *patch + 4, loop_start);
                    }
                    let jback = e.jmp_rel32();
                    e.patch_rel32(jback, jback + 4, loop_start);
                    let loop_end = e.here();
                    e.patch_rel32(jend, jend + 4, loop_end);
                    for patch in &ctx.break_patches {
                        e.patch_rel32(*patch, *patch + 4, loop_end);
                    }
                }
                _ => return Err(BxError::Unsupported),
            }
        }
        Ok(())
    }

    fn compilar_rompe(&mut self) -> BxResult<()> {
        let jmp = {
            match &mut self.emitter {
                TargetEmitter::X86_64(e) => e.jmp_rel32(),
                _ => return Err(BxError::Unsupported),
            }
        };
        let ctx = self.loop_stack.last_mut().ok_or(BxError::InvalidArgument)?;
        ctx.break_patches.push(jmp);
        Ok(())
    }

    fn compilar_continua(&mut self) -> BxResult<()> {
        let jmp = {
            match &mut self.emitter {
                TargetEmitter::X86_64(e) => e.jmp_rel32(),
                _ => return Err(BxError::Unsupported),
            }
        };
        let ctx = self.loop_stack.last_mut().ok_or(BxError::InvalidArgument)?;
        ctx.continue_patches.push(jmp);
        Ok(())
    }

    /// Codegen de una expresión x86-64. Resultado en RAX.
    fn codegen_expr_x86(&mut self, expr: &Expr) -> BxResult<()> {
        match expr {
            Expr::LitInt(imm) => {
                match &mut self.emitter {
                    TargetEmitter::X86_64(e) => e.mov_reg_imm64(Reg64::Rax, *imm),
                    _ => return Err(BxError::Unsupported),
                }
            }
            Expr::LitByte(b) => {
                match &mut self.emitter {
                    TargetEmitter::X86_64(e) => {
                        e.xor_rax_rax();
                        e.bytes.extend_from_slice(&[0xB0, *b]); // mov al, imm8
                    }
                    _ => return Err(BxError::Unsupported),
                }
            }
            Expr::Reg(r_name) => {
                if let Some(r) = Reg64::from_name(r_name) {
                    if r != Reg64::Rax {
                        match &mut self.emitter {
                            TargetEmitter::X86_64(e) => e.mov_reg_reg(Reg64::Rax, r),
                            _ => return Err(BxError::Unsupported),
                        }
                    }
                } else {
                    return Err(BxError::InvalidArgument);
                }
            }
            Expr::Ident(name) => {
                // Variable reference: load from stack frame.
                let entry = self.scope.lookup(name).ok_or(BxError::InvalidArgument)?;
                let offset = entry.frame_offset;
                match &mut self.emitter {
                    TargetEmitter::X86_64(e) => {
                        if offset >= -128 && offset <= 127 {
                            e.mov_rax_rbp_disp8(offset as i8);
                        } else {
                            e.mov_rax_rbp_disp32(offset);
                        }
                    }
                    _ => return Err(BxError::Unsupported),
                }
            }
            Expr::Bin(op, left, right) => {
                // codegen left -> RAX
                self.codegen_expr_x86(left)?;
                match &mut self.emitter {
                    TargetEmitter::X86_64(e) => e.push_rax(),
                    _ => return Err(BxError::Unsupported),
                }
                // codegen right -> RAX
                self.codegen_expr_x86(right)?;
                // Move right to RCX
                match &mut self.emitter {
                    TargetEmitter::X86_64(e) => {
                        if Reg64::Rax != Reg64::Rcx {
                            e.mov_reg_reg(Reg64::Rcx, Reg64::Rax);
                        }
                        e.pop_rax();
                    }
                    _ => return Err(BxError::Unsupported),
                }
                match op {
                    BinOp::Suma => {
                        match &mut self.emitter { TargetEmitter::X86_64(e) => e.add_rax_rcx(), _ => return Err(BxError::Unsupported) }
                    }
                    BinOp::Resta => {
                        match &mut self.emitter { TargetEmitter::X86_64(e) => e.sub_rax_rcx(), _ => return Err(BxError::Unsupported) }
                    }
                    BinOp::Mult => {
                        match &mut self.emitter { TargetEmitter::X86_64(e) => e.imul_rax_rcx(), _ => return Err(BxError::Unsupported) }
                    }
                    BinOp::Div => {
                        match &mut self.emitter { TargetEmitter::X86_64(e) => e.idiv_rcx(), _ => return Err(BxError::Unsupported) }
                    }
                    BinOp::Igual => {
                        match &mut self.emitter {
                            TargetEmitter::X86_64(e) => {
                                e.cmp_reg_reg(Reg64::Rax, Reg64::Rcx);
                                e.xor_rax_rax();
                                e.sete_al();
                                e.movzx_rax_al();
                            }
                            _ => return Err(BxError::Unsupported),
                        }
                    }
                    BinOp::Mayor => {
                        match &mut self.emitter {
                            TargetEmitter::X86_64(e) => {
                                e.cmp_reg_reg(Reg64::Rcx, Reg64::Rax);
                                e.xor_rax_rax();
                                e.bytes.extend_from_slice(&[0x0F, 0x9F, 0xC0]); // setg al
                                e.movzx_rax_al();
                            }
                            _ => return Err(BxError::Unsupported),
                        }
                    }
                    BinOp::Menor => {
                        match &mut self.emitter {
                            TargetEmitter::X86_64(e) => {
                                e.cmp_reg_reg(Reg64::Rcx, Reg64::Rax);
                                e.xor_rax_rax();
                                e.bytes.extend_from_slice(&[0x0F, 0x9C, 0xC0]); // setl al
                                e.movzx_rax_al();
                            }
                            _ => return Err(BxError::Unsupported),
                        }
                    }
                    BinOp::Y => {
                        match &mut self.emitter { TargetEmitter::X86_64(e) => e.and_rax_rcx(), _ => return Err(BxError::Unsupported) }
                    }
                    BinOp::O => {
                        match &mut self.emitter { TargetEmitter::X86_64(e) => e.or_rax_rcx(), _ => return Err(BxError::Unsupported) }
                    }
                }
            }
            Expr::No(inner) => {
                self.codegen_expr_x86(inner)?;
                match &mut self.emitter {
                    TargetEmitter::X86_64(e) => {
                        e.test_rax_rax();
                        e.xor_rax_rax();
                        e.sete_al();
                        e.movzx_rax_al();
                    }
                    _ => return Err(BxError::Unsupported),
                }
            }
            Expr::Aloc(size_expr) => {
                self.codegen_expr_x86(size_expr)?;
                // For now, just leave size in RAX (actual alloc needs syscall).
            }
            Expr::Call { name, args } => {
                // BMO ABI: arguments in RDI, RSI, RDX, R10, R8, R9, RAX (7 max).
                // Evaluate arguments and place them in the correct registers.
                for (i, arg) in args.iter().enumerate() {
                    if i >= 7 {
                        return Err(BxError::InvalidArgument); // Too many arguments
                    }
                    // Evaluate argument into RAX
                    self.codegen_expr_x86(arg)?;
                    // Move to the correct BMO ABI register
                    let dst_reg = BMO_ARG_REGS_X86[i];
                    if dst_reg != Reg64::Rax {
                        match &mut self.emitter {
                            TargetEmitter::X86_64(e) => {
                                e.mov_reg_reg(dst_reg, Reg64::Rax);
                            }
                            _ => return Err(BxError::Unsupported),
                        }
                    }
                }
                // Emit call instruction (with back-patching for forward references)
                let call_offset = match &mut self.emitter {
                    TargetEmitter::X86_64(e) => e.call_rel32(),
                    _ => return Err(BxError::Unsupported),
                };
                // Check if function is already compiled (backward call) or forward
                if let Some(entry) = self.fn_table.get(name) {
                    // Backward call: patch immediately
                    let code_offset = entry.code_offset;
                    match &mut self.emitter {
                        TargetEmitter::X86_64(e) => {
                            e.patch_rel32(call_offset, call_offset + 4, code_offset);
                        }
                        _ => {}
                    }
                } else {
                    // Forward call: add to patch list
                    self.call_patches.push(CallPatch {
                        call_offset,
                        fn_name: name.clone(),
                    });
                }
                // Result is already in RAX (return value per BMO ABI)
            }
            Expr::LitNulo => {
                match &mut self.emitter {
                    TargetEmitter::X86_64(e) => e.xor_rax_rax(),
                    _ => return Err(BxError::Unsupported),
                }
            }
            Expr::LitStr(_) => {
                match &mut self.emitter {
                    TargetEmitter::X86_64(e) => e.xor_rax_rax(),
                    _ => return Err(BxError::Unsupported),
                }
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
        // El programa debe contener el string "hola\0" al final
        assert!(bytes.windows(5).any(|w| w == b"hola\0"));
    }

    #[test]
    fn test_traductor_aarch64() {
        let mut trad = Traductor::with_target(TargetArch::Aarch64);
        let res = trad.traducir(b"def main() { reg x0 = 42 reg x1 = \"hola\" }");
        assert!(res.is_ok());
        let bytes = res.unwrap();
        assert!(bytes.windows(5).any(|w| w == b"hola\0"));
    }

    #[test]
    fn test_traductor_riscv() {
        let mut trad = Traductor::with_target(TargetArch::Riscv64);
        let res = trad.traducir(b"def main() { reg a0 = 42 reg a1 = \"hola\" }");
        assert!(res.is_ok());
        let bytes = res.unwrap();
        assert!(bytes.windows(5).any(|w| w == b"hola\0"));
    }
}
