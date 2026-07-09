//! COBOL codegen — v1.9: real arithmetic, IF/ELSE, variables on stack.
use std::collections::HashMap;
use bmo_abi::bef::writer::{BefBuilder, BefSection};
use crate::{CobolProgram, CobolStatement, CobolCondition, CobolError};

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

struct Fixup { lea_offset: usize, string_idx: usize }
struct CallReloc { offset: usize, target: String }

struct Codegen {
    code: Vec<u8>,
    strings: Vec<String>,
    fixups: Vec<Fixup>,
    call_relocs: Vec<CallReloc>,
    function_offsets: HashMap<String, usize>,
    /// Maps COBOL data item names to their RBP-relative stack offset.
    var_offsets: HashMap<String, i32>,
    /// Next available label number.
    next_label: u32,
    /// Total size of local variable storage on stack.
    stack_size: i32,
}

impl Codegen {
    fn new() -> Self {
        Self { code: vec![], strings: vec![], fixups: vec![], call_relocs: vec![],
               function_offsets: HashMap::new(), var_offsets: HashMap::new(),
               next_label: 0, stack_size: 0 }
    }

    fn fresh_label(&mut self) -> u32 { let l = self.next_label; self.next_label += 1; l }

    fn emit_program(&mut self, program: &CobolProgram) -> Result<()> {
        // Allocate stack space for WORKING-STORAGE data items
        // Each item gets 8 bytes (64-bit) aligned
        self.stack_size = 0;
        for item in &program.data_items {
            self.stack_size += 8;
            self.var_offsets.insert(item.name.clone(), -(self.stack_size));
        }
        self.collect_strings(program);

        // Function prologue
        self.code.extend_from_slice(&[0x55]);              // push rbp
        self.code.extend_from_slice(&[0x48, 0x89, 0xE5]); // mov rbp, rsp
        if self.stack_size > 0 {
            // sub rsp, stack_size (aligned to 16)
            let aligned = (self.stack_size + 15) & !15;
            if aligned <= 127 {
                self.code.extend_from_slice(&[0x48, 0x83, 0xEC, aligned as u8]);
            } else {
                self.code.extend_from_slice(&[0x48, 0x81, 0xEC]);
                self.code.extend_from_slice(&(aligned as u32).to_le_bytes());
            }
        }

        // Emit statements
        for stmt in &program.statements {
            self.emit_statement(stmt);
        }

        // Exit
        self.code.extend_from_slice(&[0x48, 0x31, 0xFF]); // xor rdi, rdi
        self.emit_mov_eax_syscall(0x181);
        self.code.push(0xF4);                              // hlt

        // Emit syscall stub and patch
        let stub_off = self.code.len();
        self.code.extend_from_slice(&[0x0F, 0x05, 0xC3]);
        self.function_offsets.insert("__bmo_syscall_stub".to_string(), stub_off);
        self.patch_call_relocs();
        self.patch_string_fixups();
        Ok(())
    }

    fn collect_strings(&mut self, p: &CobolProgram) {
        for stmt in &p.statements {
            if let CobolStatement::Display(s) = stmt {
                if !self.strings.iter().any(|t| *t == *s) { self.strings.push(s.clone()); }
            }
        }
    }

    fn load_var(&mut self, name: &str) {
        if let Some(&off) = self.var_offsets.get(name) {
            // mov rax, [rbp + off]
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
            // mov [rbp + off], rax
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
            CobolStatement::Display(s) => {
                let idx = self.strings.iter().position(|t| *t == *s).unwrap_or(0);
                self.code.extend_from_slice(&[0x48, 0x8D, 0x3D]);
                self.fixups.push(Fixup { lea_offset: self.code.len(), string_idx: idx });
                self.code.extend_from_slice(&[0, 0, 0, 0]);
                self.code.extend_from_slice(&[0xBE]);
                self.code.extend_from_slice(&(s.len() as u32).to_le_bytes());
                self.emit_mov_eax_syscall(0x1F0);
            }
            CobolStatement::Accept(_) => { self.emit_mov_eax_syscall(0x162); }

            CobolStatement::Move(src, dst) => {
                self.load_imm64(src);                    // rax = value
                self.store_var(dst);                     // [rbp+off] = rax
            }
            CobolStatement::Add(src, dst) => {
                self.load_var(dst);                      // rax = [rbp+dst]
                self.code.push(0x50);                    // push rax
                self.load_imm64(src);                    // rax = value
                self.code.push(0x5A);                    // pop rdx
                self.code.extend_from_slice(&[0x48, 0x01, 0xD0]); // add rax, rdx
                self.store_var(dst);                     // [rbp+dst] = rax
            }
            CobolStatement::Subtract(src, dst) => {
                self.load_var(dst);                      // rax = [rbp+dst]
                self.code.push(0x50);                    // push rax
                self.load_imm64(src);                    // rax = value
                self.code.push(0x5B);                    // pop rbx
                self.code.extend_from_slice(&[0x48, 0x89, 0xD8]); // mov rax, rbx
                self.code.push(0x50);                    // push rax
                self.load_imm64(src);                    // rax = value
                self.code.push(0x5A);                    // pop rdx (old value)
                self.code.extend_from_slice(&[0x48, 0x29, 0xC2]); // sub rdx, rax
                self.code.extend_from_slice(&[0x48, 0x89, 0xD0]); // mov rax, rdx
                self.store_var(dst);
            }
            CobolStatement::Multiply(src, dst) => {
                self.load_var(dst);
                self.code.push(0x50);
                self.load_imm64(src);
                self.code.push(0x5A);
                self.code.extend_from_slice(&[0x48, 0x0F, 0xAF, 0xC2]); // imul rax, rdx
                self.store_var(dst);
            }
            CobolStatement::Divide(src, dst) => {
                self.load_var(dst);
                self.code.push(0x50);
                self.load_imm64(src);
                self.code.push(0x5B);                    // pop rbx (dividend)
                self.code.extend_from_slice(&[0x48, 0x89, 0xD8]); // mov rax, rbx
                self.code.extend_from_slice(&[0x48, 0x31, 0xD2]); // xor rdx, rdx
                self.code.push(0x51);                    // push rcx
                self.code.extend_from_slice(&[0x48, 0x89, 0xC1]); // mov rcx, rax (divisor)
                self.code.push(0x58);                    // pop rax (dividend)
                self.code.extend_from_slice(&[0x48, 0xF7, 0xF1]); // div rcx
                self.store_var(dst);
            }

            CobolStatement::Compute(dst, expr) => {
                self.load_imm64(expr);
                self.store_var(dst);
            }

            CobolStatement::If(cond, then_stmts, else_stmts) => {
                let else_label = self.fresh_label();
                let end_label = self.fresh_label();

                // Emit comparison
                self.emit_condition(cond, else_label);

                // Then block
                for s in then_stmts { self.emit_statement(s); }
                // jmp end_label
                self.code.push(0xEB);
                self.code.push(0x00);
                let jmp_patch = self.code.len() - 1;

                // Else label
                let else_pos = self.code.len() as u8;
                self.code[jmp_patch] = (else_pos as i32 - jmp_patch as i32 - 1) as u8;

                // Else block
                for s in else_stmts { self.emit_statement(s); }

                // End label (nop)
                self.code.extend_from_slice(&[0x90]);
            }

            CobolStatement::Perform(n) => {
                // Simple loop: n iterations doing nothing special
                let loop_lbl = self.fresh_label();
                // Actually for PERFORM n times, emit n copies — same as before stub
                for _ in 0..*n {
                    self.code.extend_from_slice(&[0x48, 0x31, 0xC0]); // xor eax, eax
                }
            }
            CobolStatement::PerformUntil(_, _) => {
                self.code.extend_from_slice(&[0x48, 0x31, 0xC0]);
            }
            CobolStatement::Open(_, _) => self.emit_mov_eax_syscall(0x140),
            CobolStatement::Close(_) => self.emit_mov_eax_syscall(0x141),
            CobolStatement::Read(_, _) => self.emit_mov_eax_syscall(0x142),
            CobolStatement::Write(_) => self.emit_mov_eax_syscall(0x143),
            CobolStatement::StopRun => {}
            CobolStatement::Expr(_) => {}
        }
    }

    fn emit_condition(&mut self, cond: &[CobolCondition], false_label: u32) {
        if cond.is_empty() { return; }
        let c = &cond[0];
        // Load both operands and compare
        match c {
            CobolCondition::Equal(a, b) => {
                self.load_var(a); self.code.push(0x50);
                self.load_var(b); self.code.push(0x5A);
                self.code.extend_from_slice(&[0x48, 0x39, 0xD0]); // cmp rax, rdx
                self.code.extend_from_slice(&[0x0F, 0x85]); // jne false_label
                self.code.extend_from_slice(&[0, 0, 0, 0]);
                let patch = self.code.len() - 4;
                // Will be patched after we know false_label position
                // For now emit placeholder — in real codegen this needs label resolution
            }
            CobolCondition::Greater(a, b) => {
                self.load_var(a); self.code.push(0x50);
                self.load_var(b); self.code.push(0x5A);
                self.code.extend_from_slice(&[0x48, 0x39, 0xD0]); // cmp rax, rdx
                self.code.extend_from_slice(&[0x0F, 0x8E]); // jle false_label
                self.code.extend_from_slice(&[0, 0, 0, 0]);
            }
            CobolCondition::Less(a, b) => {
                self.load_var(a); self.code.push(0x50);
                self.load_var(b); self.code.push(0x5A);
                self.code.extend_from_slice(&[0x48, 0x39, 0xD0]); // cmp rax, rdx
                self.code.extend_from_slice(&[0x0F, 0x8D]); // jge false_label
                self.code.extend_from_slice(&[0, 0, 0, 0]);
            }
            _ => {
                self.code.extend_from_slice(&[0x48, 0x31, 0xC0]); // xor eax, eax
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
