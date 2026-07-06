use bmo_abi::bef::writer::{BefBuilder, BefSection};
use crate::ast::*;
use crate::CError;

type Result<T> = core::result::Result<T, CError>;

pub fn compile_to_bef_bytes(program: &Program) -> Result<Vec<u8>> {
    let mut cg = Codegen::new();
    cg.emit_program(program)?;
    Ok(cg.build_bef())
}

struct Fixup {
    lea_offset: usize,
    string_idx: usize,
}

struct PendingReloc {
    offset: usize,
    target_label: u32,
}

struct Codegen {
    code: Vec<u8>,
    strings: Vec<String>,
    fixups: Vec<Fixup>,
    labels: u32,
    pending_relocs: Vec<PendingReloc>,
    break_target: Vec<u32>,
    continue_target: Vec<u32>,
}

impl Codegen {
    fn new() -> Self {
        Self { code: Vec::new(), strings: Vec::new(), fixups: Vec::new(), labels: 0, pending_relocs: Vec::new(), break_target: Vec::new(), continue_target: Vec::new() }
    }

    fn fresh_label(&mut self) -> u32 {
        let l = self.labels;
        self.labels += 1;
        l
    }

    fn emit_program(&mut self, program: &Program) -> Result<()> {
        self.collect_strings(program);
        for func in &program.functions {
            self.emit_function(func);
        }
        self.patch_string_fixups();
        Ok(())
    }

    fn collect_strings(&mut self, program: &Program) {
        for func in &program.functions {
            for stmt in &func.body { self.collect_stmt_strings(stmt); }
        }
    }

    fn collect_stmt_strings(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Printf(s) | Stmt::PrintfLn(s) => {
                if !self.strings.iter().any(|t| *t == *s) { self.strings.push(s.clone()); }
            }
            Stmt::If(_, t, e) => { self.collect_stmt_strings(t); if let Some(el) = e { self.collect_stmt_strings(el); } }
            Stmt::While(_, b) => self.collect_stmt_strings(b),
            Stmt::DoWhile(b, _) => self.collect_stmt_strings(b),
            Stmt::For(_, _, _, b) => self.collect_stmt_strings(b),
            Stmt::Switch(_, cases) => { for c in cases { for s in &c.stmts { self.collect_stmt_strings(s); } } }
            Stmt::Block(stmts) => { for s in stmts { self.collect_stmt_strings(s); } }
            _ => {}
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

    fn resolve_label(&mut self, label: u32) {
        let here = self.code.len() as i32;
        let mut i = 0;
        while i < self.pending_relocs.len() {
            if self.pending_relocs[i].target_label == label {
                let off = self.pending_relocs[i].offset;
                let disp = here - (off as i32 + 4);
                self.code[off..off + 4].copy_from_slice(&disp.to_le_bytes());
                self.pending_relocs.swap_remove(i);
            } else { i += 1; }
        }
    }

    fn emit_jmp_reloc(&mut self, label: u32) {
        self.code.extend_from_slice(&[0xE9]);
        self.pending_relocs.push(PendingReloc { offset: self.code.len(), target_label: label });
        self.code.extend_from_slice(&[0, 0, 0, 0]);
    }

    fn emit_jz_reloc(&mut self, label: u32) {
        self.code.extend_from_slice(&[0x0F, 0x84]);
        self.pending_relocs.push(PendingReloc { offset: self.code.len(), target_label: label });
        self.code.extend_from_slice(&[0, 0, 0, 0]);
    }

    fn emit_jnz_reloc(&mut self, label: u32) {
        self.code.extend_from_slice(&[0x0F, 0x85]);
        self.pending_relocs.push(PendingReloc { offset: self.code.len(), target_label: label });
        self.code.extend_from_slice(&[0, 0, 0, 0]);
    }

    // ---- Function emit ----
    fn emit_function(&mut self, func: &Function) {
        self.emit_push_rbp();
        let stack = func.var_count * 8;
        if stack > 0 { self.emit_sub_rsp(stack as u8); }
        for stmt in &func.body { self.emit_stmt(stmt); }
        self.emit_epilogue();
    }

    fn emit_push_rbp(&mut self) { self.code.extend_from_slice(&[0x55, 0x48, 0x89, 0xE5]); }
    fn emit_sub_rsp(&mut self, n: u8) { self.code.extend_from_slice(&[0x48, 0x83, 0xEC, n]); }
    fn emit_epilogue(&mut self) { self.code.extend_from_slice(&[0x48, 0x89, 0xEC, 0x5D, 0xC3]); }

    // ---- Statement emit ----
    fn emit_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Printf(s) => self.emit_printf(s, false),
            Stmt::PrintfLn(s) => self.emit_printf(s, true),
            Stmt::If(c, t, e) => {
                let else_lbl = self.fresh_label();
                let end_lbl = self.fresh_label();
                self.emit_test_cond(c, else_lbl);
                self.emit_stmt(t);
                if e.is_some() { self.emit_jmp_reloc(end_lbl); }
                self.resolve_label(else_lbl);
                if let Some(el) = e { self.emit_stmt(el); }
                self.resolve_label(end_lbl);
            }
            Stmt::While(c, b) => {
                let start = self.fresh_label();
                let end = self.fresh_label();
                self.continue_target.push(start);
                self.break_target.push(end);
                self.resolve_label(start);
                self.emit_test_cond(c, end);
                self.emit_stmt(b);
                self.emit_jmp_reloc(start);
                self.resolve_label(end);
                self.continue_target.pop();
                self.break_target.pop();
            }
            Stmt::DoWhile(b, c) => {
                let start = self.fresh_label();
                let end = self.fresh_label();
                self.continue_target.push(end);
                self.break_target.push(end);
                self.resolve_label(start);
                self.emit_stmt(b);
                self.resolve_label(end);
                self.emit_test_cond_jnz(c, start);
                self.continue_target.pop();
                self.break_target.pop();
            }
            Stmt::For(init, cond, inc, b) => {
                if let Some(e) = init { self.emit_expr(e); self.emit_drop(); }
                let start = self.fresh_label();
                let end = self.fresh_label();
                let inc_lbl = self.fresh_label();
                self.continue_target.push(inc_lbl);
                self.break_target.push(end);
                self.resolve_label(start);
                if let Some(c) = cond { self.emit_test_cond(c, end); }
                self.emit_stmt(b);
                self.resolve_label(inc_lbl);
                if let Some(e) = inc { self.emit_expr(e); self.emit_drop(); }
                self.emit_jmp_reloc(start);
                self.resolve_label(end);
                self.continue_target.pop();
                self.break_target.pop();
            }
            Stmt::Switch(expr, cases) => {
                self.emit_expr(expr);
                let end = self.fresh_label();
                self.break_target.push(end);
                let mut case_labels = Vec::new();
                for _ in cases { case_labels.push(self.fresh_label()); }
                let default_lbl = self.fresh_label();
                for (i, c) in cases.iter().enumerate() {
                    if let Some(val) = c.value {
                        self.code.push(0x50); // push rax (save switch value)
                        self.emit_expr(&Expr::Int(val));
                        self.code.push(0x5A); // pop rdx
                        self.code.push(0x58); // pop rax
                        self.code.extend_from_slice(&[0x48, 0x39, 0xD0]);
                        self.emit_jz_reloc(case_labels[i]);
                        self.code.push(0x50); // push rax back
                    }
                }
                self.emit_jmp_reloc(default_lbl);
                for (i, c) in cases.iter().enumerate() {
                    self.resolve_label(case_labels[i]);
                    for s in &c.stmts { self.emit_stmt(s); }
                }
                self.resolve_label(default_lbl);
                self.resolve_label(end);
                self.break_target.pop();
            }
            Stmt::Break => {
                if let Some(lbl) = self.break_target.last() { self.emit_jmp_reloc(*lbl); }
            }
            Stmt::Continue => {
                if let Some(lbl) = self.continue_target.last() { self.emit_jmp_reloc(*lbl); }
            }
            Stmt::Return(Some(e)) => {
                self.emit_expr(e);
                self.emit_epilogue();
            }
            Stmt::Return(None) => {
                self.emit_epilogue();
            }
            Stmt::DeclAssign(_, _name, init) => {
                if let Some(e) = init { self.emit_expr(e); } else { self.emit_expr(&Expr::Int(0)); }
            }
            Stmt::Expr(e) => {
                self.emit_expr(e);
                self.emit_drop();
            }
            Stmt::Block(stmts) => {
                for s in stmts { self.emit_stmt(s); }
            }
        }
    }

    fn emit_test_cond(&mut self, expr: &Expr, false_label: u32) {
        self.emit_expr(expr);
        self.code.extend_from_slice(&[0x85, 0xC0]);
        self.emit_jz_reloc(false_label);
    }

    fn emit_test_cond_jnz(&mut self, expr: &Expr, label: u32) {
        self.emit_expr(expr);
        self.code.extend_from_slice(&[0x85, 0xC0]);
        self.emit_jnz_reloc(label);
    }

    fn emit_drop(&mut self) {
        // result is already in rax, discard
    }

    // ---- Printf ----
    fn emit_printf(&mut self, s: &str, newline: bool) {
        let text = if newline { let mut t = s.to_string(); t.push('\n'); t } else { s.to_string() };
        let Some(idx) = self.strings.iter().position(|t| *t == text) else { return };
        self.code.extend_from_slice(&[0x48, 0x8D, 0x3D]);
        self.fixups.push(Fixup { lea_offset: self.code.len(), string_idx: idx });
        self.code.extend_from_slice(&[0, 0, 0, 0]);
        self.code.extend_from_slice(&[0xBE]);
        self.code.extend_from_slice(&(text.len() as u32).to_le_bytes());
        self.emit_mov_eax_syscall(0x1F0);
    }

    // ---- Expression emit ----
    fn emit_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Int(n) => {
                self.code.extend_from_slice(&[0x48, 0xB8]);
                self.code.extend_from_slice(&(*n as u64).to_le_bytes());
            }
            Expr::CharLit(c) => {
                self.code.extend_from_slice(&[0x48, 0xB8]);
                self.code.extend_from_slice(&(*c as u64).to_le_bytes());
            }
            Expr::StringLit(_) | Expr::Var(_) | Expr::Call(_, _) => {
                self.code.extend_from_slice(&[0x48, 0x31, 0xC0]);
            }
            Expr::Assign(_, val) => { self.emit_expr(val); }
            Expr::Neg(a) => { self.emit_expr(a); self.code.extend_from_slice(&[0x48, 0xF7, 0xD8]); }
            Expr::Not(a) => { self.emit_expr(a); self.code.extend_from_slice(&[0x85, 0xC0, 0x0F, 0x94, 0xC0]); }
            Expr::BitNot(a) => { self.emit_expr(a); self.code.extend_from_slice(&[0x48, 0xF7, 0xD0]); }
            Expr::PreInc(_n) => { self.code.extend_from_slice(&[0x48, 0x31, 0xC0]); }
            Expr::PreDec(_n) => { self.code.extend_from_slice(&[0x48, 0x31, 0xC0]); }
            Expr::PostInc(_n) => { self.code.extend_from_slice(&[0x48, 0x31, 0xC0]); }
            Expr::PostDec(_n) => { self.code.extend_from_slice(&[0x48, 0x31, 0xC0]); }
            Expr::Deref(a) => { self.emit_expr(a); }
            Expr::AddrOf(_n) => { self.code.extend_from_slice(&[0x48, 0x31, 0xC0]); }
            Expr::Subscript(_n, i) => { self.emit_expr(i); }
            Expr::Add(a, b) => self.emit_binop(a, b, &[0x48, 0x01, 0xD0]),
            Expr::Sub(a, b) => self.emit_binop(a, b, &[0x48, 0x29, 0xD0]),
            Expr::Mul(a, b) => self.emit_binop(a, b, &[0x48, 0x0F, 0xAF, 0xC2]),
            Expr::Div(a, b) => {
                self.emit_expr(a); self.code.push(0x50);
                self.emit_expr(b); self.code.push(0x5A);
                self.code.extend_from_slice(&[0x48, 0x89, 0xD6, 0x58, 0x48, 0x31, 0xD2, 0x48, 0xF7, 0xF6]);
            }
            Expr::Mod(a, b) => {
                self.emit_expr(a); self.code.push(0x50);
                self.emit_expr(b); self.code.push(0x5A);
                self.code.extend_from_slice(&[0x48, 0x89, 0xD6, 0x58, 0x48, 0x31, 0xD2, 0x48, 0xF7, 0xF6, 0x48, 0x89, 0xD0]);
            }
            Expr::Eq(a, b) => self.emit_cmp(a, b, &[0x0F, 0x94, 0xC0]),
            Expr::Neq(a, b) => self.emit_cmp(a, b, &[0x0F, 0x95, 0xC0]),
            Expr::Lt(a, b) => self.emit_cmp(a, b, &[0x0F, 0x9C, 0xC0]),
            Expr::Gt(a, b) => self.emit_cmp_swapped(b, a, &[0x0F, 0x9C, 0xC0]),
            Expr::Le(a, b) => self.emit_cmp_swapped(b, a, &[0x0F, 0x9E, 0xC0]),
            Expr::Ge(a, b) => self.emit_cmp(a, b, &[0x0F, 0x9D, 0xC0]),
            Expr::BitAnd(a, b) => self.emit_binop(a, b, &[0x48, 0x21, 0xD0]),
            Expr::BitXor(a, b) => self.emit_binop(a, b, &[0x48, 0x31, 0xD0]),
            Expr::BitOr(a, b) => self.emit_binop(a, b, &[0x48, 0x09, 0xD0]),
            Expr::Shl(a, b) => self.emit_binop(a, b, &[0x48, 0x89, 0xD1, 0x48, 0xD3, 0xE0]),
            Expr::Shr(a, b) => self.emit_binop(a, b, &[0x48, 0x89, 0xD1, 0x48, 0xD3, 0xE8]),
            Expr::LAnd(a, b) => {
                let end = self.fresh_label();
                self.emit_expr(a);
                self.code.extend_from_slice(&[0x85, 0xC0]);
                self.emit_jz_reloc(end);
                self.emit_expr(b);
                self.resolve_label(end);
            }
            Expr::LOr(a, b) => {
                let end = self.fresh_label();
                self.emit_expr(a);
                self.code.extend_from_slice(&[0x85, 0xC0]);
                self.emit_jnz_reloc(end);
                self.emit_expr(b);
                self.resolve_label(end);
            }
            Expr::Conditional(c, t, f) => {
                let else_lbl = self.fresh_label();
                let end_lbl = self.fresh_label();
                self.emit_test_cond(c, else_lbl);
                self.emit_expr(t);
                self.emit_jmp_reloc(end_lbl);
                self.resolve_label(else_lbl);
                self.emit_expr(f);
                self.resolve_label(end_lbl);
            }
            Expr::Comma(exprs) => {
                for (i, e) in exprs.iter().enumerate() {
                    self.emit_expr(e);
                    if i < exprs.len() - 1 { self.emit_drop(); }
                }
            }
        }
    }

    fn emit_binop(&mut self, a: &Expr, b: &Expr, op: &[u8]) {
        self.emit_expr(a);
        self.code.push(0x50);
        self.emit_expr(b);
        self.code.push(0x5A);
        self.code.extend_from_slice(op);
    }

    fn emit_cmp(&mut self, a: &Expr, b: &Expr, setcc: &[u8]) {
        self.emit_expr(a);
        self.code.push(0x50);
        self.emit_expr(b);
        self.code.push(0x5A);
        self.code.extend_from_slice(&[0x48, 0x39, 0xD0]);
        self.code.extend_from_slice(setcc);
    }

    fn emit_cmp_swapped(&mut self, a: &Expr, b: &Expr, setcc: &[u8]) {
        self.emit_expr(a);
        self.code.push(0x50);
        self.emit_expr(b);
        self.code.push(0x5A);
        self.code.extend_from_slice(&[0x48, 0x39, 0xD0]);
        self.code.extend_from_slice(setcc);
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
