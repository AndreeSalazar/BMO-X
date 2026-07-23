use std::collections::HashMap;
use bmo_abi::bef::writer::{BefBuilder, BefSection};
use bmo_abi::syscalls::*;
use bmo_abi::syscalls::surface;
use bmo_sem_asm::Instructions;
use bmo_sem_asm::x86_64::{Asm, Reg};
use crate::ast::{CobolProgram, CobolStatement, CobolCondition};
use crate::ast::error::CobolError;

type Result<T> = core::result::Result<T, CobolError>;

// BMO x86-64 SYSCALL argument registers: RDI, RSI, RDX, R10, R8, R9.
// RCX lo pisa el CPU con la dirección de retorno del usuario. Antes esto era
// la tabla REG_MOV de bytes a mano; ahora el mov reg,rax lo emite el encoder
// sem-asm (mismos bytes, leídos de la tabla TOML).
const ARG_REGS: [Reg; 6] = [Reg::Rdi, Reg::Rsi, Reg::Rdx, Reg::R10, Reg::R8, Reg::R9];

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
    /// Escala decimal por variable (dígitos tras la V del PIC). El alma
    /// bancaria: un `PIC 9(3)V99` tiene escala 2 → guarda centavos.
    var_scales: HashMap<String, u32>,
    next_label: u32,
    stack_size: i32,
    /// Tabla de instrucciones sem-asm (opcodes leídos de la TOML).
    isa: Instructions,
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
            var_scales: HashMap::new(),
            next_label: 0,
            stack_size: 0,
            isa: Instructions::load_x86_64().expect("tablas sem-asm x86-64 (forge/sem-asm/tables)"),
        }
    }

    /// Escala decimal de una variable (0 = entero / no declarada).
    fn var_scale(&self, name: &str) -> u32 {
        *self.var_scales.get(name).unwrap_or(&0)
    }

    /// Convierte un literal COBOL a su ENTERO escalado por `scale`. Es el
    /// corazón del decimal exacto: `"10.05"` con escala 2 → `1005` centavos;
    /// `"3.2"` → `320`; `"7"` → `700`. Trunca los decimales sobrantes (el
    /// default de COBOL sin `ROUNDED`). Un entero con escala 0 queda igual.
    fn scaled_imm(lit: &str, scale: u32) -> u64 {
        let s = lit.trim().trim_start_matches(['+', '-']);
        let (int_part, frac_part) = s.split_once('.').unwrap_or((s, ""));
        let int_val: u64 = int_part.parse().unwrap_or(0);
        let mut frac = frac_part.to_string();
        while (frac.len() as u32) < scale {
            frac.push('0');
        }
        let frac_val: u64 = if scale == 0 {
            0
        } else {
            frac[..scale as usize].parse().unwrap_or(0)
        };
        int_val * 10u64.pow(scale) + frac_val
    }

    /// `mov rax, <literal escalado a `scale`>`.
    fn load_scaled_imm(&mut self, lit: &str, scale: u32) {
        let v = Self::scaled_imm(lit, scale);
        self.emit_asm(|a| { a.mov_imm64(Reg::Rax, v).unwrap(); });
    }

    /// Emite bytes con el encoder sem-asm (opcode de la tabla + REX/ModRM).
    fn emit_asm(&mut self, build: impl FnOnce(&mut Asm)) {
        let mut a = Asm::new(&self.isa);
        build(&mut a);
        self.code.extend_from_slice(a.bytes());
    }

    fn fresh_label(&mut self) -> u32 { let l = self.next_label; self.next_label += 1; l }

    fn emit_program(&mut self, program: &CobolProgram) -> Result<()> {
        self.stack_size = 0;
        for item in &program.data_items {
            let size = item.storage_size();
            let aligned = (size as i32 + 7) & !7;
            self.stack_size += aligned;
            self.var_offsets.insert(item.name.clone(), -(self.stack_size));
            // Recuerda la escala decimal del PIC para la aritmética exacta.
            self.var_scales.insert(item.name.clone(), item.scale());
        }
        self.collect_strings(program);

        // Function prologue
        self.code.extend_from_slice(&[0x55]);              // push rbp
        self.code.extend_from_slice(&[0x48, 0x89, 0xE5]); // mov rbp, rsp
        // A BEF process entry may be reached by a jump rather than CALL. Do not
        // assume an incoming RSP residue: reserve all locals plus alignment
        // slack, then establish BMO's required 64-byte pre-call alignment.
        self.code.extend_from_slice(&[0x48, 0x81, 0xEC]); // sub rsp, imm32
        self.code.extend_from_slice(&((self.stack_size as u32) + 63).to_le_bytes());
        self.code.extend_from_slice(&[0x48, 0x83, 0xE4, 0xC0]); // and rsp, -64

        // Emit COBOL statements
        for stmt in &program.statements {
            self.emit_statement(stmt);
        }

        // Exit
        self.emit_v2_task_invoke(surface::task_op::EXIT, &[]);
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
        // mov rax, imm64 — antes [0x48,0xB8]+imm a mano; ahora el encoder.
        self.emit_asm(|a| { a.mov_imm64(Reg::Rax, num).unwrap(); });
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
                if let Some(operation) = surface::task_operation_for_legacy_syscall(def.nr) {
                    self.emit_v2_task_invoke(operation, args);
                } else {
                    for (i, arg) in args.iter().enumerate() {
                        if i < ARG_REGS.len() {
                            let value: u64 = arg.parse().unwrap_or(0);
                            self.emit_imm64_syscall_arg(i, value);
                        }
                    }
                    self.emit_mov_eax_syscall(def.nr);
                }
            }
            CobolStatement::Display(s) => self.emit_display(s),
            CobolStatement::Accept(_) => self.emit_mov_eax_syscall(NR_INPUT_POLL_EVENT),

            // Decimal EXACTO: el literal se escala a la escala del destino,
            // así $10.05 en un PIC 9(3)V99 se guarda como el entero 1005.
            CobolStatement::Move(src, dst) => {
                let sc = self.var_scale(dst);
                self.load_scaled_imm(src, sc);
                self.store_var(dst);
            }
            // ADD a misma escala: ambos operandos en centavos → `add` es
            // suma decimal exacta (sin float, sin redondeo). El alma bancaria.
            CobolStatement::Add(src, dst) => {
                let sc = self.var_scale(dst);
                self.load_var(dst);
                self.code.push(0x50);                    // push rax
                self.load_scaled_imm(src, sc);
                self.code.push(0x5A);                    // pop rdx
                self.code.extend_from_slice(&[0x48, 0x01, 0xD0]); // add rax, rdx
                self.store_var(dst);
            }
            CobolStatement::Subtract(src, dst) => {
                let sc = self.var_scale(dst);
                self.load_var(dst);
                self.code.push(0x50);
                self.load_scaled_imm(src, sc);
                self.code.push(0x5B);
                self.code.extend_from_slice(&[0x48, 0x89, 0xD8]);
                self.code.push(0x50);
                self.load_scaled_imm(src, sc);
                self.code.push(0x5A);
                self.code.extend_from_slice(&[0x48, 0x29, 0xC2]);
                self.code.extend_from_slice(&[0x48, 0x89, 0xD0]);
                self.store_var(dst);
            }
            // MULTIPLY decimal exacto: dst y src están en escala `sc`
            // (centavos). El producto queda en escala 2·sc → se REESCALA
            // dividiendo entre 10^sc para volver a `sc`. $2.00 × 3 = $6.00.
            CobolStatement::Multiply(src, dst) => {
                let sc = self.var_scale(dst);
                self.load_var(dst);                              // rax = dst_scaled
                self.code.push(0x50);                            // push rax
                self.load_scaled_imm(src, sc);                   // rax = src_scaled
                self.code.push(0x5A);                            // pop rdx
                self.code.extend_from_slice(&[0x48, 0x0F, 0xAF, 0xC2]); // imul rax, rdx
                if sc > 0 {
                    let p = 10u64.pow(sc);
                    self.emit_asm(|a| { a.mov_imm64(Reg::Rcx, p).unwrap(); }); // mov rcx, 10^sc
                    self.code.extend_from_slice(&[0x48, 0x99]);         // cqo
                    self.code.extend_from_slice(&[0x48, 0xF7, 0xF9]);   // idiv rcx
                }
                self.store_var(dst);
            }
            // DIVIDE decimal exacto: dst / src. Para conservar la escala, el
            // dividendo se PREESCALA ×10^sc antes de dividir (si no, el
            // resultado quedaría en escala 0). $10.00 / 4 = $2.50.
            CobolStatement::Divide(src, dst) => {
                let sc = self.var_scale(dst);
                self.load_scaled_imm(src, sc);                   // rax = src_scaled (divisor)
                self.code.push(0x50);                            // push rax
                self.load_var(dst);                              // rax = dst_scaled (dividendo)
                if sc > 0 {
                    let p = 10u64.pow(sc);
                    self.emit_asm(|a| { a.mov_imm64(Reg::Rcx, p).unwrap(); }); // mov rcx, 10^sc
                    self.code.extend_from_slice(&[0x48, 0x0F, 0xAF, 0xC1]);    // imul rax, rcx
                }
                self.code.push(0x59);                            // pop rcx (divisor)
                self.code.extend_from_slice(&[0x48, 0x99]);       // cqo
                self.code.extend_from_slice(&[0x48, 0xF7, 0xF9]); // idiv rcx
                self.store_var(dst);
            }
            CobolStatement::Compute(dst, expr) => {
                let sc = self.var_scale(dst);
                self.load_scaled_imm(expr, sc);
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

    fn emit_v2_task_invoke(&mut self, operation: u64, args: &[String]) {
        self.emit_imm64_syscall_arg(0, surface::CURRENT_TASK);
        self.emit_imm64_syscall_arg(1, operation);
        for index in 0..4 {
            let value = args.get(index).and_then(|arg| arg.parse().ok()).unwrap_or(0);
            self.emit_imm64_syscall_arg(index + 2, value);
        }
        self.emit_mov_eax_syscall(surface::NR_INVOKE);
    }

    fn emit_imm64_syscall_arg(&mut self, index: usize, value: u64) {
        // mov rax, imm64 ; mov <arg_reg>, rax — todo por el encoder sem-asm.
        let dst = ARG_REGS[index];
        self.emit_asm(|a| {
            a.mov_imm64(Reg::Rax, value).unwrap();
            a.mov_reg(dst, Reg::Rax).unwrap();
        });
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

#[cfg(test)]
mod tests {
    use super::Codegen;

    #[test]
    fn decimal_literals_scale_to_exact_cents() {
        // El alma bancaria: literal decimal → entero escalado exacto.
        assert_eq!(Codegen::scaled_imm("10.05", 2), 1005); // $10.05 → 1005 centavos
        assert_eq!(Codegen::scaled_imm("3.2", 2), 320);    // $3.20  → 320
        assert_eq!(Codegen::scaled_imm("7", 2), 700);      // $7.00  → 700
        assert_eq!(Codegen::scaled_imm("0.99", 2), 99);    // 99 centavos
        // Suma exacta de centavos, sin float:
        assert_eq!(
            Codegen::scaled_imm("10.05", 2) + Codegen::scaled_imm("3.20", 2),
            1325 // $13.25 exacto
        );
    }

    #[test]
    fn scale_zero_is_plain_integer() {
        // Enteros: comportamiento previo intacto (no rompe nada).
        assert_eq!(Codegen::scaled_imm("42", 0), 42);
        assert_eq!(Codegen::scaled_imm("1000", 0), 1000);
    }

    #[test]
    fn truncates_extra_decimals() {
        // COBOL sin ROUNDED trunca (no redondea).
        assert_eq!(Codegen::scaled_imm("1.999", 2), 199);
    }
}
