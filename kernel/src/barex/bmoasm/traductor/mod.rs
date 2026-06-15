//! Traductor central de BMO Simple.
//! Coordina el Lexer, Parser, Sema y Emitter para producir bytes nativos del target.
//! Implementa la resolución semántica de cadenas literales.

extern crate alloc;
use alloc::vec::Vec;

use crate::barex::{BxError, BxResult};
use super::parser::{Parser, Ast, Stmt, Expr};
use super::sema::Sema;
use super::emit::{TargetArch, TargetEmitter, TargetRegister, Reg64};
use super::builtin::{IntrinsicId, emit_intrinsic};

struct StringRef {
    disp_offset: usize, // Offset en el código donde va el displacement del LEA/ADR
    rodata_offset: usize, // Offset del string en el bloque de datos rodata
}

pub struct Traductor {
    target: TargetArch,
    emitter: TargetEmitter,
    rodata: Vec<u8>,
    string_refs: Vec<StringRef>,
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
                Stmt::Def { name: _, params: _, ret: _, body } => {
                    self.compilar_body(body)?;
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
                        _ => return Err(BxError::Unsupported),
                    }
                }
                Stmt::Let { name: _, ty: _, value } => {
                    // let simple mapeado a registro por defecto
                    match value {
                        Expr::LitInt(imm) => {
                            match &mut self.emitter {
                                TargetEmitter::X86_64(e) => e.mov_reg_imm64(Reg64::Rax, *imm),
                                TargetEmitter::Aarch64(e) => e.mov_reg_imm64(super::emit::aarch64::RegArm::X0, *imm),
                                TargetEmitter::Riscv64(e) => e.mov_reg_imm64(super::emit::riscv::RegRiscv::A0, *imm),
                            }
                        }
                        _ => return Err(BxError::Unsupported),
                    }
                }
                Stmt::Retorna(expr_opt) => {
                    if let Some(expr) = expr_opt {
                        match expr {
                            Expr::LitInt(imm) => {
                                match &mut self.emitter {
                                    TargetEmitter::X86_64(e) => e.mov_reg_imm64(Reg64::Rax, *imm),
                                    TargetEmitter::Aarch64(e) => e.mov_reg_imm64(super::emit::aarch64::RegArm::X0, *imm),
                                    TargetEmitter::Riscv64(e) => e.mov_reg_imm64(super::emit::riscv::RegRiscv::A0, *imm),
                                }
                            }
                            Expr::Reg(r_name) => {
                                let r = TargetRegister::from_name(self.target, r_name).ok_or(BxError::InvalidArgument)?;
                                match (&mut self.emitter, r) {
                                    (TargetEmitter::X86_64(e), TargetRegister::X86_64(rs)) => {
                                        if rs != Reg64::Rax {
                                            e.mov_reg_reg(Reg64::Rax, rs);
                                        }
                                    }
                                    (TargetEmitter::Aarch64(e), TargetRegister::Aarch64(rs)) => {
                                        if rs != super::emit::aarch64::RegArm::X0 {
                                            e.mov_reg_reg(super::emit::aarch64::RegArm::X0, rs);
                                        }
                                    }
                                    (TargetEmitter::Riscv64(e), TargetRegister::Riscv64(rs)) => {
                                        if rs != super::emit::riscv::RegRiscv::A0 {
                                            e.mov_reg_reg(super::emit::riscv::RegRiscv::A0, rs);
                                        }
                                    }
                                    _ => return Err(BxError::InvalidArgument),
                                }
                            }
                            _ => return Err(BxError::Unsupported),
                        }
                    }
                    match &mut self.emitter {
                        TargetEmitter::X86_64(e) => e.ret(),
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
}

impl Default for Traductor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::barex::bmoasm::sample::SALUDO;

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
