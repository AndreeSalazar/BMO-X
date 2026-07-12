use std::collections::HashMap;
use bmo_abi::bef::writer::{BefBuilder, BefSection};
use bmo_abi::syscalls::*;
use crate::ast::{CobolProgram, CobolStatement, CobolCondition};
use crate::ast::error::CobolError;

type Result<T> = core::result::Result<T, CobolError>;

const REG_MOV: &[[u8; 3]] = &[
    [0x48, 0x89, 0xC7], [0x48, 0x89, 0xC6], [0x48, 0x89, 0xC2],
    [0x49, 0x89, 0xC2], [0x49, 0x89, 0xC0], [0x49, 0x89, 0xC1],
];

pub fn compile_to_bef_bytes(program: &CobolProgram) -> Result<Vec<u8>> {
    let mut cg = Codegen::new();
    cg.emit_program(program)?;
    Ok(cg.build_bef())
}

struct StrFixup { lea_offset: usize, string_idx: usize }
struct CallReloc { offset: usize, target: String }

struct Codegen {
    code: Vec<u8>,
    strings: Vec<String>,
    str_fixups: Vec<StrFixup>,
    call_relocs: Vec<CallReloc>,
    function_offsets: HashMap<String, usize>,
    var_offsets: HashMap<String, i32>,
    next_label: u32,
    stack_size: i32,
}

impl Codegen {
    fn new() -> Self {
        Self {
            code: vec![],
            strings: vec![],
            str_fixups: vec![],
            call_relocs: vec![],
            function_offsets: HashMap::new(),
            var_offsets: HashMap::new(),
            next_label: 0,
            stack_size: 0,
        }
    }

    fn fresh_label(&mut self) -> u32 { let l = self.next_label; self.next_label += 1; l }

    fn emit_program(&mut self, program: &CobolProgram) -> Result<()> {
        self.stack_size = 0;
        for item in &program.data_items {
            let size = item.storage_size();
            let aligned = (size as i32 + 7) & !7;
            self.stack_size += aligned;
            self.var_offsets.insert(item.name.clone(), -(self.stack_size));
        }
        self.collect_strings(program);

        // Function prologue
        self.code.extend_from_slice(&[0x55]);              // push rbp
        self.code.extend_from_slice(&[0x48, 0x89, 0xE5]); // mov rbp, rsp
        if self.stack_size > 0 {
            let aligned = (self.stack_size + 15) & !15;
            if aligned <= 127 {
                self.code.extend_from_slice(&[0x48, 0x83, 0xEC, aligned as u8]);
            } else {
                self.code.extend_from_slice(&[0x48, 0x81, 0xEC]);
                self.code.extend_from_slice(&(aligned as u32).to_le_bytes());
            }
        }

        // Emit COBOL statements
        for stmt in &program.statements {
            self.emit_statement(stmt);
        }

        // Exit
        self.code.extend_from_slice(&[0x48, 0x31, 0xFF]); // xor rdi, rdi
        self.emit_mov_eax_syscall(NR_PROC_EXIT);
        self.code.push(0xF4);                              // hlt

        // Syscall stub
        let stub_off = self.code.len();
        self.code.extend_from_slice(&[0x0F, 0x05, 0xC3]); // syscall; ret
        self.function_offsets.insert("__bmo_syscall_stub".to_string(), stub_off);
        self.patch_call_relocs();
        self.patch_string_fixups();
        Ok(())
    }

    fn collect_strings(&mut self, p: &CobolProgram) {
        for stmt in &p.statements {
            if let CobolStatement::Display(s) = stmt {
                if !self.strings.iter().any(|t| *t == *s) {
                    self.strings.push(s.clone());
                }
            }
        }
    }

    fn load_var(&mut self, name: &str) {
        if let Some(&off) = self.var_offsets.get(name) {
            if off >= -128 && off <= 127 {
                self.code.extend_from_slice(&[0x48, 0x8B, 0x45, off as u8]);
            } else {
                self.code.extend_from_slice(&[0x48, 0x8B, 0x85]);
                self.code.extend_from_slice(&off.to_le_bytes());
            }
        }
    }

    fn store_var(&mut self, name: &str) {
        if let Some(&off) = self.var_offsets.get(name) {
            if off >= -128 && off <= 127 {
                self.code.extend_from_slice(&[0x48, 0x89, 0x45, off as u8]);
            } else {
                self.code.extend_from_slice(&[0x48, 0x89, 0x85]);
                self.code.extend_from_slice(&off.to_le_bytes());
            }
        }
    }

    fn load_imm64(&mut self, val: &str) {
        let num: u64 = val.parse().unwrap_or(0);
        self.code.extend_from_slice(&[0x48, 0xB8]);
        self.code.extend_from_slice(&num.to_le_bytes());
    }

    fn emit_display(&mut self, s: &str) {
        let idx = self.strings.iter().position(|t| *t == *s).unwrap_or(0);
        // lea rdi, [rip + string_offset]
        self.code.extend_from_slice(&[0x48, 0x8D, 0x3D]);
        self.str_fixups.push(StrFixup { lea_offset: self.code.len(), string_idx: idx });
        self.code.extend_from_slice(&[0, 0, 0, 0]); // placeholder
        // mov esi, len
        self.code.extend_from_slice(&[0xBE]);
        self.code.extend_from_slice(&(s.len() as u32).to_le_bytes());
        self.emit_mov_eax_syscall(NR_DEBUG_PRINT);
    }

    fn emit_statement(&mut self, stmt: &CobolStatement) {
        match stmt {
            CobolStatement::Syscall(def, args) => {
                for (i, arg) in args.iter().enumerate() {
                    if i < 6 {
                        self.load_imm64(arg);
                        self.code.extend_from_slice(&REG_MOV[i]);
                    }
                }
                self.emit_mov_eax_syscall(def.nr);
            }
            CobolStatement::Display(s) => self.emit_display(s),
            CobolStatement::Accept(_) => self.emit_mov_eax_syscall(NR_INPUT_POLL_EVENT),

            CobolStatement::Move(src, dst) => {
                self.load_imm64(src);
                self.store_var(dst);
            }
            CobolStatement::Add(src, dst) => {
                self.load_var(dst);
                self.code.push(0x50);                    // push rax
                self.load_imm64(src);
                self.code.push(0x5A);                    // pop rdx
                self.code.extend_from_slice(&[0x48, 0x01, 0xD0]); // add rax, rdx
                self.store_var(dst);
            }
            CobolStatement::Subtract(src, dst) => {
                self.load_var(dst);
                self.code.push(0x50);
                self.load_imm64(src);
                self.code.push(0x5B);
                self.code.extend_from_slice(&[0x48, 0x89, 0xD8]);
                self.code.push(0x50);
                self.load_imm64(src);
                self.code.push(0x5A);
                self.code.extend_from_slice(&[0x48, 0x29, 0xC2]);
                self.code.extend_from_slice(&[0x48, 0x89, 0xD0]);
                self.store_var(dst);
            }
            CobolStatement::Multiply(src, dst) => {
                self.load_var(dst);
                self.code.push(0x50);
                self.load_imm64(src);
                self.code.push(0x5A);
                self.code.extend_from_slice(&[0x48, 0x0F, 0xAF, 0xC2]);
                self.store_var(dst);
            }
            CobolStatement::Divide(src, dst) => {
                self.load_var(dst);
                self.code.push(0x50);
                self.load_imm64(src);
                self.code.push(0x5B);
                self.code.extend_from_slice(&[0x48, 0x89, 0xD8]);
                self.code.extend_from_slice(&[0x48, 0x31, 0xD2]);
                self.code.push(0x51);
                self.code.extend_from_slice(&[0x48, 0x89, 0xC1]);
                self.code.push(0x58);
                self.code.extend_from_slice(&[0x48, 0xF7, 0xF1]);
                self.store_var(dst);
            }
            CobolStatement::Compute(dst, expr) => {
                self.load_imm64(expr);
                self.store_var(dst);
            }
            CobolStatement::If(cond, then_stmts, else_stmts) => {
                let _else_label = self.fresh_label();
                let _end_label = self.fresh_label();
                self.emit_condition(cond);
                for s in then_stmts { self.emit_statement(s); }
                self.code.push(0xEB);
                self.code.push(0x00);
                for s in else_stmts { self.emit_statement(s); }
                self.code.extend_from_slice(&[0x90]);
            }
            CobolStatement::Perform(n) => {
                let _loop_lbl = self.fresh_label();
                for _ in 0..*n {
                    self.code.extend_from_slice(&[0x48, 0x31, 0xC0]);
                }
            }
            CobolStatement::PerformUntil(_, _) => {
                self.code.extend_from_slice(&[0x48, 0x31, 0xC0]);
            }
            CobolStatement::Open(_, _) => self.emit_mov_eax_syscall(NR_FS_OPEN),
            CobolStatement::Close(_) => self.emit_mov_eax_syscall(NR_FS_CLOSE),
            CobolStatement::Read(_, _) => self.emit_mov_eax_syscall(NR_FS_READ),
            CobolStatement::Write(_) => self.emit_mov_eax_syscall(NR_FS_WRITE),
            CobolStatement::StopRun => {}
            CobolStatement::Expr(_) => {}
        }
    }

    fn emit_condition(&mut self, cond: &[CobolCondition]) {
        if cond.is_empty() { return; }
        let c = &cond[0];
        match c {
            CobolCondition::Equal(a, b) => {
                self.load_var(a); self.code.push(0x50);
                self.load_var(b); self.code.push(0x5A);
                self.code.extend_from_slice(&[0x48, 0x39, 0xD0]);
                self.code.extend_from_slice(&[0x0F, 0x85]);
                self.code.extend_from_slice(&[0, 0, 0, 0]);
            }
            CobolCondition::Greater(a, b) => {
                self.load_var(a); self.code.push(0x50);
                self.load_var(b); self.code.push(0x5A);
                self.code.extend_from_slice(&[0x48, 0x39, 0xD0]);
                self.code.extend_from_slice(&[0x0F, 0x8E]);
                self.code.extend_from_slice(&[0, 0, 0, 0]);
            }
            CobolCondition::Less(a, b) => {
                self.load_var(a); self.code.push(0x50);
                self.load_var(b); self.code.push(0x5A);
                self.code.extend_from_slice(&[0x48, 0x39, 0xD0]);
                self.code.extend_from_slice(&[0x0F, 0x8D]);
                self.code.extend_from_slice(&[0, 0, 0, 0]);
            }
            _ => {
                self.code.extend_from_slice(&[0x48, 0x31, 0xC0]);
            }
        }
    }

    fn emit_mov_eax_syscall(&mut self, nr: u32) {
        self.code.extend_from_slice(&[0xB8]);
        self.code.extend_from_slice(&nr.to_le_bytes());
        self.emit_call_to_syscall_stub();
    }

    fn emit_call_to_syscall_stub(&mut self) {
        self.code.extend_from_slice(&[0xE8]);
        self.call_relocs.push(CallReloc { offset: self.code.len(), target: "__bmo_syscall_stub".to_string() });
        self.code.extend_from_slice(&[0, 0, 0, 0]);
    }

    fn patch_string_fixups(&mut self) {
        let code_end = self.code.len();
        let mut str_off = code_end;
        for (idx, s) in self.strings.iter().enumerate() {
            for f in &self.str_fixups {
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

    fn patch_call_relocs(&mut self) {
        for reloc in &self.call_relocs {
            if let Some(&t) = self.function_offsets.get(&reloc.target) {
                let d = t as i32 - (reloc.offset as i32 + 4);
                self.code[reloc.offset..reloc.offset + 4].copy_from_slice(&d.to_le_bytes());
            }
        }
    }

    fn build_bef(&mut self) -> Vec<u8> {
        let all = core::mem::take(&mut self.code);
        let mut b = BefBuilder::new();
        b.add_section(BefSection::code(all));
        b.entry_offset = 0;
        b.build().unwrap_or_default()
    }
}
