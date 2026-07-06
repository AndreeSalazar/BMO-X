use bmo_abi::bef::writer::{BefBuilder, BefSection};
use crate::{CobolProgram, CobolStatement, CobolError};

type Result<T> = core::result::Result<T, CobolError>;

/// SysV ABI register move sequences for syscall args
const REG_MOV: &[[u8; 3]] = &[
    [0x48, 0x89, 0xC7], // mov rdi, rax
    [0x48, 0x89, 0xC6], // mov rsi, rax
    [0x48, 0x89, 0xC2], // mov rdx, rax
    [0x49, 0x89, 0xC2], // mov r10, rax
    [0x49, 0x89, 0xC0], // mov r8,  rax
    [0x49, 0x89, 0xC1], // mov r9,  rax
];

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
        // Emit entry point: a minimal function header (push rbp; mov rbp, rsp)
        self.code.extend_from_slice(&[0x55, 0x48, 0x89, 0xE5]); // push rbp; mov rbp, rsp
        for stmt in &program.statements {
            self.emit_statement(stmt);
        }
        self.emit_exit();
        self.patch_string_fixups();
        Ok(())
    }

    fn collect_strings(&mut self, program: &CobolProgram) {
        for stmt in &program.statements {
            match stmt {
                CobolStatement::Display(ref s) => {
                    if !self.strings.iter().any(|t| *t == *s) { self.strings.push(s.clone()); }
                }
                _ => {}
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
            CobolStatement::Syscall(def, args) => {
                for (i, arg) in args.iter().enumerate() {
                    if i < 6 {
                        let num: u64 = arg.parse().unwrap_or(0);
                        self.code.extend_from_slice(&[0x48, 0xB8]); // mov rax, imm64
                        self.code.extend_from_slice(&num.to_le_bytes());
                        self.code.extend_from_slice(&REG_MOV[i]);    // mov reg, rax
                    }
                }
                self.code.extend_from_slice(&[0xB8]);
                self.code.extend_from_slice(&def.nr.to_le_bytes());
                self.code.extend_from_slice(&[0x0F, 0x05]);
            }
            CobolStatement::Display(s) => {
                let idx = self.strings.iter().position(|t| *t == *s).unwrap_or(0);
                self.code.extend_from_slice(&[0x48, 0x8D, 0x3D]); // lea rdi, [rip+disp]
                self.fixups.push(Fixup { lea_offset: self.code.len(), string_idx: idx });
                self.code.extend_from_slice(&[0, 0, 0, 0]);
                self.code.extend_from_slice(&[0xBE]); // mov esi, len
                self.code.extend_from_slice(&(s.len() as u32).to_le_bytes());
                self.emit_mov_eax_syscall(0x1F0); // NR_DEBUG_PRINT
            }
            CobolStatement::Accept(_) => {
                self.emit_mov_eax_syscall(0x162); // NR_INPUT_POLL_EVENT
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
                self.code.extend_from_slice(&[0x48, 0x31, 0xC0]); // xor rax, rax
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
                self.emit_mov_eax_syscall(0x140); // NR_FS_OPEN (was WRONG 0x150!)
            }
            CobolStatement::Close(_) => {
                self.emit_mov_eax_syscall(0x141); // NR_FS_CLOSE (was WRONG 0x151!)
            }
            CobolStatement::Read(_, _) => {
                self.emit_mov_eax_syscall(0x142); // NR_FS_READ (was WRONG 0x152!)
            }
            CobolStatement::Write(_) => {
                self.emit_mov_eax_syscall(0x143); // NR_FS_WRITE (was WRONG 0x153!)
            }
            CobolStatement::StopRun => {}
            CobolStatement::Expr(_) => {}
        }
    }

    fn emit_exit(&mut self) {
        // mov rdi, 0 (exit code) then syscall
        self.code.extend_from_slice(&[0x48, 0x31, 0xFF]); // xor rdi, rdi
        self.emit_mov_eax_syscall(0x181); // NR_PROC_EXIT
        self.code.push(0xF4);             // hlt (safety)
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
