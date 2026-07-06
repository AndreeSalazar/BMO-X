use bmo_abi::bef::writer::{BefBuilder, BefSection};
use crate::{CobolProgram, CobolStatement, CobolError};

type Result<T> = core::result::Result<T, CobolError>;

pub fn compile_to_bef_bytes(program: &CobolProgram) -> Result<Vec<u8>> {
    let mut cg = Codegen::new();
    cg.emit_program(program)?;
    Ok(cg.build_bef())
}

struct Fixup {
    lea_offset: usize,
    string_idx: usize,
}

struct Codegen {
    code: Vec<u8>,
    strings: Vec<String>,
    fixups: Vec<Fixup>,
}

impl Codegen {
    fn new() -> Self {
        Self { code: Vec::new(), strings: Vec::new(), fixups: Vec::new() }
    }

    fn emit_program(&mut self, program: &CobolProgram) -> Result<()> {
        self.collect_strings(program);
        for stmt in &program.statements {
            self.emit_statement(stmt);
        }
        self.emit_exit();
        self.patch_string_fixups();
        Ok(())
    }

    fn collect_strings(&mut self, program: &CobolProgram) {
        for stmt in &program.statements {
            if let CobolStatement::Display(ref s) = stmt {
                if !self.strings.iter().any(|t| *t == *s) {
                    self.strings.push(s.clone());
                }
            }
        }
    }

    fn patch_string_fixups(&mut self) {
        let code_end = self.code.len();
        let mut str_off = code_end;
        for (idx, s) in self.strings.iter().enumerate() {
            for f in &self.fixups {
                if f.string_idx == idx {
                    let rip = f.lea_offset + 4;
                    let disp = str_off as i64 - rip as i64;
                    self.code[f.lea_offset..f.lea_offset + 4].copy_from_slice(&(disp as i32).to_le_bytes());
                }
            }
            self.code.extend_from_slice(s.as_bytes());
            self.code.push(0);
            str_off += s.len() + 1;
        }
    }

    fn emit_statement(&mut self, stmt: &CobolStatement) {
        match stmt {
            CobolStatement::Display(s) => {
                let idx = self.strings.iter().position(|t| *t == *s).unwrap_or(0);
                self.code.extend_from_slice(&[0x48, 0x8D, 0x3D]);
                self.fixups.push(Fixup { lea_offset: self.code.len(), string_idx: idx });
                self.code.extend_from_slice(&[0, 0, 0, 0]);
                self.code.extend_from_slice(&[0xBE]);
                self.code.extend_from_slice(&(s.len() as u32).to_le_bytes());
                self.emit_mov_eax_syscall(0x1F0);
            }
            CobolStatement::Accept(_) => {
                self.emit_mov_eax_syscall(0x160);
            }
            CobolStatement::Move(val, _target) => {
                let num: u64 = val.parse().unwrap_or(0);
                self.code.extend_from_slice(&[0x48, 0xB8]);
                self.code.extend_from_slice(&num.to_le_bytes());
            }
            CobolStatement::Add(val, _target) => {
                let num: u64 = val.parse().unwrap_or(0);
                self.code.extend_from_slice(&[0x48, 0xB8]);
                self.code.extend_from_slice(&num.to_le_bytes());
            }
            CobolStatement::Subtract(val, _target) => {
                let num: u64 = val.parse().unwrap_or(0);
                self.code.extend_from_slice(&[0x48, 0xB8]);
                self.code.extend_from_slice(&num.to_le_bytes());
            }
            CobolStatement::Multiply(val, _target) => {
                let num: u64 = val.parse().unwrap_or(0);
                self.code.extend_from_slice(&[0x48, 0xB8]);
                self.code.extend_from_slice(&num.to_le_bytes());
            }
            CobolStatement::Divide(val, _target) => {
                let num: u64 = val.parse().unwrap_or(0);
                self.code.extend_from_slice(&[0x48, 0xB8]);
                self.code.extend_from_slice(&num.to_le_bytes());
            }
            CobolStatement::Compute(_target, _expr) => {
                self.code.extend_from_slice(&[0x48, 0x31, 0xC0]);
            }
            CobolStatement::If(_cond, _then, _else) => {
                self.code.extend_from_slice(&[0x48, 0x31, 0xC0]);
            }
            CobolStatement::Perform(n) => {
                for _ in 0..*n {
                    self.code.extend_from_slice(&[0x48, 0x31, 0xC0]);
                }
            }
            CobolStatement::PerformUntil(_, _) => {
                self.code.extend_from_slice(&[0x48, 0x31, 0xC0]);
            }
            CobolStatement::Open(_, _) => {
                self.emit_mov_eax_syscall(0x150);
            }
            CobolStatement::Close(_) => {
                self.emit_mov_eax_syscall(0x151);
            }
            CobolStatement::Read(_, _) => {
                self.emit_mov_eax_syscall(0x152);
            }
            CobolStatement::Write(_) => {
                self.emit_mov_eax_syscall(0x153);
            }
            CobolStatement::StopRun => {}
            CobolStatement::Expr(_) => {}
        }
    }

    fn emit_exit(&mut self) {
        self.code.extend_from_slice(&[0x31, 0xFF]);
        self.emit_mov_eax_syscall(0x181);
        self.code.push(0xF4);
    }

    fn emit_mov_eax_syscall(&mut self, nr: u32) {
        self.code.extend_from_slice(&[0xB8]);
        self.code.extend_from_slice(&nr.to_le_bytes());
        self.code.extend_from_slice(&[0x0F, 0x05]);
    }

    fn build_bef(&mut self) -> Vec<u8> {
        let all = core::mem::take(&mut self.code);
        let mut b = BefBuilder::new();
        b.add_section(BefSection::code(all));
        b.entry_offset = 0;
        b.build().unwrap_or_default()
    }
}
