//! Traductor central de BMO Simple.
//! Coordina el Lexer, Parser, Sema y Emitter para producir bytes nativos de x86-64.
//! Implementa la resolución semántica de cadenas literales ("hola" -> RIP-relative LEA + RoData).

extern crate alloc;
use alloc::vec::Vec;
use alloc::string::String;

use crate::barex::{BxError, BxResult};
use super::parser::{Parser, Ast, Stmt, Expr};
use super::sema::Sema;
use super::emit::{Emitter, Reg64};
use super::builtin::{IntrinsicId, emit_intrinsic};

struct StringRef {
    disp_offset: usize, // Offset en el código donde va la disp32 de LEA
    rodata_offset: usize, // Offset del string en el bloque de datos rodata
}

pub struct Traductor {
    emitter: Emitter,
    rodata: Vec<u8>,
    string_refs: Vec<StringRef>,
}

impl Traductor {
    pub const fn new() -> Self {
        Self {
            emitter: Emitter::new(),
            rodata: Vec::new(),
            string_refs: Vec::new(),
        }
    }

    /// Traduce código fuente en español de BMO Simple a bytes nativos x86-64.
    pub fn traducir(&mut self, src: &[u8]) -> BxResult<Vec<u8>> {
        // 1. Parser
        let mut parser = Parser::new(src);
        let ast = parser.parse()?;

        // 2. Análisis Semántico (Sema)
        let sema = Sema::new();
        sema.check(&ast)?;

        // 3. Generación de Código
        self.compilar_ast(&ast)?;

        // 4. Back-patching de Cadenas Literales (RIP-relative)
        let final_code_len = self.emitter.bytes.len();
        for s_ref in &self.string_refs {
            let next_pc = s_ref.disp_offset + 4; // Disp32 es de 4 bytes
            let target_addr = final_code_len + s_ref.rodata_offset;
            let disp = (target_addr as isize) - (next_pc as isize);
            let disp32 = (disp as i32) as u32;

            // Escribir disp32 en el code stream
            let le_bytes = disp32.to_le_bytes();
            self.emitter.bytes[s_ref.disp_offset] = le_bytes[0];
            self.emitter.bytes[s_ref.disp_offset + 1] = le_bytes[1];
            self.emitter.bytes[s_ref.disp_offset + 2] = le_bytes[2];
            self.emitter.bytes[s_ref.disp_offset + 3] = le_bytes[3];
        }

        // Concatena el código generado con el bloque de datos de lectura (RoData)
        let mut final_bytes = Vec::new();
        final_bytes.extend_from_slice(&self.emitter.bytes);
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
                    let dst_reg = Reg64::from_name(reg).ok_or(BxError::InvalidArgument)?;
                    match value {
                        Expr::LitInt(imm) => {
                            self.emitter.mov_reg_imm64(dst_reg, *imm);
                        }
                        Expr::LitStr(s) => {
                            // Almacenar el string en rodata
                            let rodata_offset = self.rodata.len();
                            self.rodata.extend_from_slice(s.as_bytes());
                            self.rodata.push(0); // Null terminator para FFI/BMO C-strings si se requiere
                            
                            // Emitir LEA con placeholder disp32
                            let disp_offset = self.emitter.lea_reg_rip_placeholder(dst_reg);
                            self.string_refs.push(StringRef {
                                disp_offset,
                                rodata_offset,
                            });
                        }
                        Expr::Reg(src_reg_name) => {
                            let src_reg = Reg64::from_name(src_reg_name).ok_or(BxError::InvalidArgument)?;
                            self.emitter.mov_reg_reg(dst_reg, src_reg);
                        }
                        _ => return Err(BxError::Unsupported),
                    }
                }
                Stmt::Let { name: _, ty: _, value } => {
                    // let simple mapeado a RAX por simplicidad temporal
                    match value {
                        Expr::LitInt(imm) => {
                            self.emitter.mov_reg_imm64(Reg64::Rax, *imm);
                        }
                        _ => return Err(BxError::Unsupported),
                    }
                }
                Stmt::Retorna(expr_opt) => {
                    if let Some(expr) = expr_opt {
                        match expr {
                            Expr::LitInt(imm) => {
                                self.emitter.mov_reg_imm64(Reg64::Rax, *imm);
                            }
                            Expr::Reg(r_name) => {
                                let r = Reg64::from_name(r_name).ok_or(BxError::InvalidArgument)?;
                                if r != Reg64::Rax {
                                    self.emitter.mov_reg_reg(Reg64::Rax, r);
                                }
                            }
                            _ => return Err(BxError::Unsupported),
                        }
                    }
                    self.emitter.ret();
                }
                Stmt::Emit(raw_bytes) => {
                    self.emitter.emit_raw(raw_bytes);
                }
                Stmt::ExprStmt(Expr::Reg(r_name)) => {
                    // Intrínsecos directos
                    if r_name == "syscall" {
                        self.emitter.syscall();
                    } else if r_name == "nop" {
                        self.emitter.nop();
                    } else if let Some(intrinsic) = self.map_intrinsic_name(r_name) {
                        emit_intrinsic(&mut self.emitter, intrinsic)?;
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
}
