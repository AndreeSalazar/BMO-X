//! Traductor central de BMO Simple.
//! Coordina el Lexer, Parser, Sema y Emitter para producir bytes nativos del target.
//! Implementa la resolución semántica de cadenas literales.

extern crate alloc;
use alloc::vec::Vec;

use crate::barex::{BxError, BxResult};
use super::parser::{Parser, Ast, Stmt, Expr, BinOp};
use super::sema::Sema;
use super::sema::scope::{Scope, ScopeEntry};
use super::emit::{TargetArch, TargetEmitter, TargetRegister, Reg64};
use super::builtin::{IntrinsicId, emit_intrinsic};

struct StringRef {
    disp_offset: usize, // Offset en el código donde va el displacement del LEA/ADR
    rodata_offset: usize, // Offset del string en el bloque de datos rodata
}

struct LoopContext {
    break_patches: Vec<usize>,   // Offsets of jmp rel32 to back-patch to loop end
    continue_patches: Vec<usize>, // Offsets of jmp rel32 to back-patch to loop start
}

pub struct Traductor {
    target: TargetArch,
    emitter: TargetEmitter,
    rodata: Vec<u8>,
    string_refs: Vec<StringRef>,
    loop_stack: Vec<LoopContext>,
    scope: Scope,
    frame_size: u32,
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

        // 3. Generación de Código
        self.compilar_ast(&ast)?;

        // 4. Back-patching de Cadenas Literales (PC-relative/RIP-relative)
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

        // Concatena el código generado con el bloque de datos de lectura (RoData)
        let mut final_bytes = Vec::new();
        final_bytes.extend_from_slice(self.emitter.bytes());
        final_bytes.extend_from_slice(&self.rodata);

        Ok(final_bytes)
    }

    fn compilar_ast(&mut self, ast: &Ast) -> BxResult<()> {
        for item in &ast.items {
            match item {
                Stmt::Def { name: _, params, ret: _, body } => {
                    self.scope = Scope::default();
                    self.frame_size = 0;
                    // Reserve parameter slots on stack (caller pushes args right-to-left)
                    for (_pname, _pty) in params {
                        self.scope.frame_size += 8;
                    }
                    // Emit prologue: push rbp; mov rbp, rsp; sub rsp, imm32 (placeholder)
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
                    self.compilar_body(body)?;
                    // Back-patch sub rsp, N with actual frame size
                    let aligned = (self.frame_size + 15) & !15; // 16-byte align
                    match &mut self.emitter {
                        TargetEmitter::X86_64(e) => {
                            let imm_bytes = (aligned as i32).to_le_bytes();
                            e.bytes[sub_rsp_offset + 3] = imm_bytes[0];
                            e.bytes[sub_rsp_offset + 4] = imm_bytes[1];
                            e.bytes[sub_rsp_offset + 5] = imm_bytes[2];
                            e.bytes[sub_rsp_offset + 6] = imm_bytes[3];
                            // Emit epilogue: leave; ret
                            e.leave();
                            e.ret();
                        }
                        _ => {}
                    }
                }
                _ => return Err(BxError::InvalidArgument),
            }
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
