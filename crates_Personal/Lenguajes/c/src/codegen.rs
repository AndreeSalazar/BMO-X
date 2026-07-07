use std::collections::HashMap;
use bmo_abi::bef::writer::{BefBuilder, BefSection};
use crate::ast::*;
use crate::CError;

type Result<T> = core::result::Result<T, CError>;

/// Target execution environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetProfile {
    /// Ring 0: inline `syscall` instruction, no stub needed.
    Ring0Kernel,
    /// Ring 3: emit `__bmo_syscall_stub` and call through it.
    Ring3App,
}

impl Default for TargetProfile {
    fn default() -> Self { TargetProfile::Ring3App }
}

pub fn compile_to_bef_bytes(program: &Program) -> Result<Vec<u8>> {
    compile_with_target(program, TargetProfile::default())
}

pub fn compile_to_bef_bytes_filtered(program: &Program, used: &[String]) -> Result<Vec<u8>> {
    let mut filtered = program.clone();
    filtered.functions.retain(|f| f.name == "main" || used.contains(&f.name));
    compile_with_target(&filtered, TargetProfile::default())
}

pub fn compile_with_target(program: &Program, target: TargetProfile) -> Result<Vec<u8>> {
    let mut cg = Codegen::new(target);
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

struct CallReloc {
    offset: usize,
    target: String,
}

struct Codegen {
    target: TargetProfile,
    code: Vec<u8>,
    strings: Vec<String>,
    fixups: Vec<Fixup>,
    labels: u32,
    pending_relocs: Vec<PendingReloc>,
    call_relocs: Vec<CallReloc>,
    function_offsets: HashMap<String, usize>,
    break_target: Vec<u32>,
    continue_target: Vec<u32>,
    var_offsets: HashMap<String, (i32, TypeSpec)>,
    struct_layouts: HashMap<String, Vec<(String, u32, u32)>>,
    struct_sizes: HashMap<String, u32>,
    label_positions: HashMap<String, usize>,
    goto_relocs: Vec<(usize, String)>,
    entry_offset: usize,
    is_entry_function: bool,
    global_offsets: HashMap<String, (u32, TypeSpec)>,
    global_data: Vec<u8>,
    global_fixups: Vec<(usize, String)>,
    instruction_end: usize,
    string_data_end: usize,
    /// Functions from userland_ring3 that need imports.
    stdlib_imports: std::collections::HashSet<String>,
}

impl Codegen {
    fn new(target: TargetProfile) -> Self {
        Self {
            target,
            code: Vec::new(), strings: Vec::new(), fixups: Vec::new(),
            labels: 0, pending_relocs: Vec::new(), call_relocs: Vec::new(),
            function_offsets: HashMap::new(), break_target: Vec::new(),
            continue_target: Vec::new(), var_offsets: HashMap::new(),
            struct_layouts: HashMap::new(), struct_sizes: HashMap::new(),
            label_positions: HashMap::new(), goto_relocs: Vec::new(),
            entry_offset: 0, is_entry_function: false,
            global_offsets: HashMap::new(), global_data: Vec::new(),
            global_fixups: Vec::new(),
            instruction_end: 0, string_data_end: 0,
            stdlib_imports: std::collections::HashSet::new(),
        }
    }

    fn fresh_label(&mut self) -> u32 {
        let l = self.labels;
        self.labels += 1;
        l
    }

    // ---- Program ----
    fn emit_program(&mut self, program: &Program) -> Result<()> {
        // build struct/union layouts
        for decl in &program.globals {
            match decl {
                GlobalDecl::Struct(name, members) => {
                    self.build_struct_layout(name, members);
                }
                GlobalDecl::Union(name, members) => {
                    self.build_union_layout(name, members);
                }
                _ => {}
            }
        }
        // allocate space for global variables
        for decl in &program.globals {
            if let GlobalDecl::Var(typ, name, init) = decl {
                let size = typ.stack_size() as u32;
                let pad = (8 - self.global_data.len() as u32 % 8) % 8;
                for _ in 0..pad { self.global_data.push(0); }
                let off = self.global_data.len() as u32;
                match init {
                    Some(Expr::Int(n)) => {
                        let bytes: Vec<u8> = match size {
                            1 => vec![*n as u8],
                            2 => (*n as u16).to_le_bytes().to_vec(),
                            4 => (*n as u32).to_le_bytes().to_vec(),
                            _ => (*n as u64).to_le_bytes().to_vec(),
                        };
                        self.global_data.extend_from_slice(&bytes);
                    }
                    _ => {
                        for _ in 0..size { self.global_data.push(0); }
                    }
                }
                self.global_offsets.insert(name.clone(), (off, typ.clone()));
            }
        }
        self.collect_strings(program);
        // emit all functions, tracking entry point
        for func in &program.functions {
            let off = self.code.len();
            self.function_offsets.insert(func.name.clone(), off);
            if func.name == "main" { self.entry_offset = off; }
            self.is_entry_function = func.name == "main";
            self.emit_function(func);
        }
        self.is_entry_function = false;
        // Emit syscall stub only for Ring 3 (Ring 0 uses inline syscall)
        if self.target == TargetProfile::Ring3App {
            let stub_off = self.code.len();
            self.code.extend_from_slice(&[0x0F, 0x05, 0xC3]);
            self.function_offsets.insert("__bmo_syscall_stub".to_string(), stub_off);
        }
        // patch all call relocs
        self.patch_call_relocs();
        self.patch_goto_relocs();
        self.patch_all_fixups();
        Ok(())
    }

    fn build_struct_layout(&mut self, name: &str, members: &[StructMember]) {
        let mut layout = Vec::new();
        let mut offset = 0u32;
        for m in members {
            let sz = self.type_stack_size(&m.typ);
            // align to member size (max 8)
            let align = sz.min(8).max(1);
            offset = (offset + align - 1) / align * align;
            layout.push((m.name.clone(), offset, sz));
            offset += sz;
        }
        // total struct size aligns to largest member
        let max_align = members.iter().map(|m| self.type_stack_size(&m.typ).min(8).max(1)).max().unwrap_or(1);
        let total = (offset + max_align - 1) / max_align * max_align;
        self.struct_layouts.insert(name.to_string(), layout);
        self.struct_sizes.insert(name.to_string(), total);
    }

    fn build_union_layout(&mut self, name: &str, members: &[StructMember]) {
        let mut layout = Vec::new();
        let mut max_sz = 0u32;
        for m in members {
            let sz = self.type_stack_size(&m.typ);
            layout.push((m.name.clone(), 0u32, sz));
            if sz > max_sz { max_sz = sz; }
        }
        self.struct_layouts.insert(name.to_string(), layout);
        self.struct_sizes.insert(name.to_string(), max_sz);
    }

    fn type_stack_size(&self, typ: &TypeSpec) -> u32 {
        match typ {
            TypeSpec::Void => 0,
            TypeSpec::Char | TypeSpec::UnsignedChar => 1,
            TypeSpec::Short | TypeSpec::UnsignedShort => 2,
            TypeSpec::Int | TypeSpec::UnsignedInt => 4,
            TypeSpec::Long | TypeSpec::UnsignedLong | TypeSpec::LongLong | TypeSpec::UnsignedLongLong => 8,
            TypeSpec::Float => 4,
            TypeSpec::Double => 8,
            TypeSpec::Ptr(_) => 8,
            TypeSpec::StructRef(name) | TypeSpec::UnionRef(name) => {
                self.struct_sizes.get(name).copied().unwrap_or(8)
            }
        }
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
            Stmt::Expr(e) | Stmt::Return(Some(e)) => { self.collect_expr_strings(e); }
            Stmt::DeclAssign(_, _, Some(e)) => { self.collect_expr_strings(e); }
            _ => {}
        }
    }

    fn collect_expr_strings(&mut self, expr: &Expr) {
        match expr {
            Expr::StringLit(s) => {
                if !self.strings.iter().any(|t| *t == *s) { self.strings.push(s.clone()); }
            }
            Expr::Neg(a) | Expr::Not(a) | Expr::BitNot(a) | Expr::Deref(a) | Expr::AddrOf(a) => self.collect_expr_strings(a),
            Expr::Add(a,b) | Expr::Sub(a,b) | Expr::Mul(a,b) | Expr::Div(a,b) | Expr::Mod(a,b)
                | Expr::Eq(a,b) | Expr::Neq(a,b) | Expr::Lt(a,b) | Expr::Gt(a,b) | Expr::Le(a,b) | Expr::Ge(a,b)
                | Expr::BitAnd(a,b) | Expr::BitXor(a,b) | Expr::BitOr(a,b) | Expr::LAnd(a,b) | Expr::LOr(a,b)
                | Expr::Shl(a,b) | Expr::Shr(a,b) => { self.collect_expr_strings(a); self.collect_expr_strings(b); }
            Expr::Conditional(c,t,f) => { self.collect_expr_strings(c); self.collect_expr_strings(t); self.collect_expr_strings(f); }
            Expr::Call(_, args) | Expr::Syscall(_, args) => { for a in args { self.collect_expr_strings(a); } }
            Expr::Arrow(p,_,_) | Expr::AssignArrow(p,_,_,_) => self.collect_expr_strings(p),
            Expr::Assign(_, v) | Expr::AssignField(_,_,_,v) => self.collect_expr_strings(v),
            Expr::AssignDeref(a, v) => { self.collect_expr_strings(a); self.collect_expr_strings(v); }
            Expr::Field(b,_,_) => self.collect_expr_strings(b),
            Expr::Comma(v) => { for e in v { self.collect_expr_strings(e); } }
            _ => {}
        }
    }

    fn patch_all_fixups(&mut self) {
        let code_end = self.code.len();
        self.instruction_end = code_end;
        // Patch string fixups and append string data
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
        // Patch global fixups and append global data
        self.string_data_end = self.code.len();
        let global_base = self.code.len();
        for &(lea_offset, ref name) in &self.global_fixups {
            if let Some(&(data_off, _)) = self.global_offsets.get(name) {
                let rip = lea_offset + 4;
                let disp = (global_base as i64 + data_off as i64) - rip as i64;
                self.code[lea_offset..lea_offset + 4].copy_from_slice(&(disp as i32).to_le_bytes());
            }
        }
        self.code.extend_from_slice(&self.global_data);
    }

    fn patch_goto_relocs(&mut self) {
        for (off, label) in &self.goto_relocs {
            if let Some(&target) = self.label_positions.get(label) {
                let disp = target as i32 - (*off as i32 + 4);
                self.code[*off..*off + 4].copy_from_slice(&disp.to_le_bytes());
            }
        }
    }

    fn patch_call_relocs(&mut self) {
        for reloc in &self.call_relocs {
            if let Some(&target_offset) = self.function_offsets.get(&reloc.target) {
                let off = reloc.offset;
                let disp = target_offset as i32 - (off as i32 + 4);
                self.code[off..off + 4].copy_from_slice(&disp.to_le_bytes());
            }
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

    // ---- Stack frame helpers ----
    fn build_var_map(&mut self, params: &[Param], var_names: &[String], func: &Function) {
        self.var_offsets.clear();
        // build map from param name → type
        let mut type_map: HashMap<&str, &TypeSpec> = HashMap::new();
        for p in params { type_map.insert(&p.name, &p.typ); }
        let param_count = params.len();
        // from var_names + declared types in body, build type map
        // Function doesn't store types for body variables, only param types.
        // We use the function's DeclAssign statements to determine types.
        for stmt in &func.body {
            if let Stmt::DeclAssign(t, n, _) = stmt {
                type_map.insert(n.as_str(), t);
            }
        }

        for (i, p) in params.iter().enumerate() {
            let off = 16 + i as i32 * 8;
            let typ = type_map.get(p.name.as_str()).copied().cloned().unwrap_or(TypeSpec::Long);
            self.var_offsets.insert(p.name.clone(), (off, typ));
        }
        for (i, name) in var_names.iter().skip(param_count).enumerate() {
            let off = -((i as i32 + 1) * 8);
            let typ = type_map.get(name.as_str()).copied().cloned().unwrap_or(TypeSpec::Long);
            self.var_offsets.insert(name.clone(), (off, typ));
        }
    }

    fn emit_store_var(&mut self, name: &str) {
        if let Some(&(offset, ref typ)) = self.var_offsets.get(name) {
            let disp = offset;
            let rex8 = if disp >= -128 && disp <= 127 { 0x45 } else { 0x85 };
            match typ {
                TypeSpec::Char | TypeSpec::UnsignedChar => {
                    self.code.extend_from_slice(&[0x88, rex8]);
                    if disp >= -128 && disp <= 127 { self.code.push(disp as u8); }
                    else { self.code.extend_from_slice(&(disp as i32).to_le_bytes()); }
                }
                TypeSpec::Short | TypeSpec::UnsignedShort => {
                    self.code.extend_from_slice(&[0x66, 0x89, rex8]);
                    if disp >= -128 && disp <= 127 { self.code.push(disp as u8); }
                    else { self.code.extend_from_slice(&(disp as i32).to_le_bytes()); }
                }
                TypeSpec::Int | TypeSpec::UnsignedInt => {
                    if disp >= -128 && disp <= 127 {
                        self.code.extend_from_slice(&[0x89, 0x45, disp as u8]);
                    } else {
                        self.code.extend_from_slice(&[0x89, 0x85]);
                        self.code.extend_from_slice(&(disp as i32).to_le_bytes());
                    }
                }
                _ => {
                    if disp >= -128 && disp <= 127 {
                        self.code.extend_from_slice(&[0x48, 0x89, 0x45, disp as u8]);
                    } else {
                        self.code.extend_from_slice(&[0x48, 0x89, 0x85]);
                        self.code.extend_from_slice(&(disp as i32).to_le_bytes());
                    }
                }
            }
        } else if let Some(&(_, ref typ)) = self.global_offsets.get(name) {
            // rax already has value; lea rdi, [rip+0]; mov [rdi], reg
            self.code.extend_from_slice(&[0x48, 0x8D, 0x3D, 0, 0, 0, 0]);
            self.global_fixups.push((self.code.len() - 4, name.to_string()));
            match typ {
                TypeSpec::Char | TypeSpec::UnsignedChar => self.code.extend_from_slice(&[0x88, 0x07]),
                TypeSpec::Short | TypeSpec::UnsignedShort => self.code.extend_from_slice(&[0x66, 0x89, 0x07]),
                TypeSpec::Int | TypeSpec::UnsignedInt => self.code.extend_from_slice(&[0x89, 0x07]),
                _ => self.code.extend_from_slice(&[0x48, 0x89, 0x07]),
            }
        }
    }

    fn emit_load_var(&mut self, name: &str) {
        if let Some(&(offset, ref typ)) = self.var_offsets.get(name) {
            let disp = offset;
            match typ {
            TypeSpec::Char => {
                if disp >= -128 && disp <= 127 {
                    self.code.extend_from_slice(&[0x48, 0x0F, 0xBE, 0x45, disp as u8]);
                } else {
                    self.code.extend_from_slice(&[0x48, 0x0F, 0xBE, 0x85]);
                    self.code.extend_from_slice(&(disp as i32).to_le_bytes());
                }
            }
            TypeSpec::UnsignedChar => {
                if disp >= -128 && disp <= 127 {
                    self.code.extend_from_slice(&[0x48, 0x0F, 0xB6, 0x45, disp as u8]);
                } else {
                    self.code.extend_from_slice(&[0x48, 0x0F, 0xB6, 0x85]);
                    self.code.extend_from_slice(&(disp as i32).to_le_bytes());
                }
            }
            TypeSpec::Short => {
                if disp >= -128 && disp <= 127 {
                    self.code.extend_from_slice(&[0x48, 0x0F, 0xBF, 0x45, disp as u8]);
                } else {
                    self.code.extend_from_slice(&[0x48, 0x0F, 0xBF, 0x85]);
                    self.code.extend_from_slice(&(disp as i32).to_le_bytes());
                }
            }
            TypeSpec::UnsignedShort => {
                if disp >= -128 && disp <= 127 {
                    self.code.extend_from_slice(&[0x48, 0x0F, 0xB7, 0x45, disp as u8]);
                } else {
                    self.code.extend_from_slice(&[0x48, 0x0F, 0xB7, 0x85]);
                    self.code.extend_from_slice(&(disp as i32).to_le_bytes());
                }
            }
            TypeSpec::Int | TypeSpec::UnsignedInt => {
                if disp >= -128 && disp <= 127 {
                    self.code.extend_from_slice(&[0x8B, 0x45, disp as u8]);
                } else {
                    self.code.extend_from_slice(&[0x8B, 0x85]);
                    self.code.extend_from_slice(&(disp as i32).to_le_bytes());
                }
            }
            _ => {
                if disp >= -128 && disp <= 127 {
                    self.code.extend_from_slice(&[0x48, 0x8B, 0x45, disp as u8]);
                } else {
                    self.code.extend_from_slice(&[0x48, 0x8B, 0x85]);
                    self.code.extend_from_slice(&(disp as i32).to_le_bytes());
                }
            }
        }
        } else if let Some(&(_, ref typ)) = self.global_offsets.get(name) {
            // lea rax, [rip+0]; then mov with size to load value
            self.code.extend_from_slice(&[0x48, 0x8D, 0x05, 0, 0, 0, 0]);
            self.global_fixups.push((self.code.len() - 4, name.to_string()));
            match typ {
                TypeSpec::Char => self.code.extend_from_slice(&[0x48, 0x0F, 0xBE, 0x00]),
                TypeSpec::UnsignedChar => self.code.extend_from_slice(&[0x48, 0x0F, 0xB6, 0x00]),
                TypeSpec::Short => self.code.extend_from_slice(&[0x48, 0x0F, 0xBF, 0x00]),
                TypeSpec::UnsignedShort => self.code.extend_from_slice(&[0x48, 0x0F, 0xB7, 0x00]),
                TypeSpec::Int | TypeSpec::UnsignedInt => self.code.extend_from_slice(&[0x8B, 0x00]),
                _ => self.code.extend_from_slice(&[0x48, 0x8B, 0x00]),
            }
        } else {
            self.emit_xor_eax();
        }
    }

    fn emit_xor_eax(&mut self) {
        self.code.extend_from_slice(&[0x48, 0x31, 0xC0]);
    }

    fn emit_inc_var(&mut self, name: &str) {
        if !self.var_offsets.contains_key(name) && !self.global_offsets.contains_key(name) { self.emit_xor_eax(); return; }
        self.emit_load_var(name);
        self.code.extend_from_slice(&[0x48, 0x83, 0xC0, 0x01]);
        self.emit_store_var(name);
    }

    fn emit_dec_var(&mut self, name: &str) {
        if !self.var_offsets.contains_key(name) && !self.global_offsets.contains_key(name) { self.emit_xor_eax(); return; }
        self.emit_load_var(name);
        self.code.extend_from_slice(&[0x48, 0x83, 0xE8, 0x01]);
        self.emit_store_var(name);
    }

    // ---- Function emit ----
    fn emit_function(&mut self, func: &Function) {
        self.build_var_map(&func.params, &func.var_names, func);
        // prologue
        self.code.extend_from_slice(&[0x55, 0x48, 0x89, 0xE5]); // push rbp; mov rbp, rsp
        // copy params from incoming stack to local slots
        let param_count = func.params.len();
        for (i, p) in func.params.iter().enumerate() {
            // param is at [rbp + 16 + i*8]
            let src_off = 16 + i as i32 * 8;
            if src_off >= -128 && src_off <= 127 {
                self.code.extend_from_slice(&[0x48, 0x8B, 0x45, src_off as u8]);
            } else {
                self.code.extend_from_slice(&[0x48, 0x8B, 0x85]);
                self.code.extend_from_slice(&(src_off as i32).to_le_bytes());
            }
            // store to local slot if it differs
                    if let Some(&(local_off, _)) = self.var_offsets.get(&p.name) {
                        if local_off != src_off {
                            if local_off >= -128 && local_off <= 127 {
                                self.code.extend_from_slice(&[0x48, 0x89, 0x45, local_off as u8]);
                            } else {
                                self.code.extend_from_slice(&[0x48, 0x89, 0x85]);
                                self.code.extend_from_slice(&(local_off as i32).to_le_bytes());
                            }
                        }
                    }
        }
        // allocate local var space
        let local_count = func.var_names.len() as i32 - param_count as i32;
        if local_count > 0 {
            let stack_size = local_count * 8;
            if stack_size <= 127 {
                self.code.extend_from_slice(&[0x48, 0x83, 0xEC, stack_size as u8]);
            } else if stack_size <= 0x7FFF {
                self.code.extend_from_slice(&[0x48, 0x81, 0xEC]);
                self.code.extend_from_slice(&(stack_size as u32).to_le_bytes());
            } else {
                self.code.extend_from_slice(&[0x48, 0x81, 0xEC]);
                self.code.extend_from_slice(&(stack_size as u32).to_le_bytes());
            }
        }
        for stmt in &func.body { self.emit_stmt(stmt); }
        self.emit_epilogue();
    }

    fn emit_epilogue(&mut self) {
        self.code.extend_from_slice(&[0x48, 0x89, 0xEC, 0x5D]); // mov rsp,rbp; pop rbp
        if self.is_entry_function {
            self.code.extend_from_slice(&[0xB8]);
            self.code.extend_from_slice(&0x181u32.to_le_bytes());
            self.emit_call_to_syscall_stub();
        } else {
            self.code.push(0xC3); // ret
        }
    }

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
                        self.code.push(0x50);
                        self.emit_expr(&Expr::Int(val));
                        self.code.push(0x5A);
                        self.code.push(0x58);
                        self.code.extend_from_slice(&[0x48, 0x39, 0xD0]);
                        self.emit_jz_reloc(case_labels[i]);
                        self.code.push(0x50);
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
            Stmt::DeclAssign(_, name, init) => {
                if let Some(e) = init { self.emit_expr(e); } else { self.emit_expr(&Expr::Int(0)); }
                self.emit_store_var(name);
            }
            Stmt::Expr(e) => {
                self.emit_expr(e);
                self.emit_drop();
            }
            Stmt::Goto(label) => {
                self.code.extend_from_slice(&[0xE9]);
                self.goto_relocs.push((self.code.len(), label.clone()));
                self.code.extend_from_slice(&[0, 0, 0, 0]);
            }
            Stmt::Label(label) => {
                self.label_positions.insert(label.clone(), self.code.len());
                // patch any pending gotos to this label
                let mut i = 0;
                while i < self.goto_relocs.len() {
                    if self.goto_relocs[i].1 == *label {
                        let (off, _) = self.goto_relocs.swap_remove(i);
                        let disp = self.code.len() as i32 - (off as i32 + 4);
                        self.code[off..off + 4].copy_from_slice(&disp.to_le_bytes());
                    } else { i += 1; }
                }
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

    fn emit_drop(&mut self) {}

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
            Expr::StringLit(s) => {
                // lea rax, [rip + disp] — fixup patched in patch_string_fixups
                self.code.extend_from_slice(&[0x48, 0x8D, 0x05, 0, 0, 0, 0]);
                let idx = self.strings.iter().position(|t| t == s).unwrap_or(0);
                self.fixups.push(Fixup { lea_offset: self.code.len() - 4, string_idx: idx });
            }
            Expr::Var(name) => {
                self.emit_load_var(name);
            }
            Expr::Call(name, args) => {
                // push args right-to-left
                for arg in args.iter().rev() {
                    self.emit_expr(arg);
                    self.code.push(0x50); // push rax
                }
                // call rel32 placeholder
                self.code.extend_from_slice(&[0xE8]);
                self.call_relocs.push(CallReloc { offset: self.code.len(), target: name.clone() });
                self.code.extend_from_slice(&[0, 0, 0, 0]);
                // Track stdlib imports for Ring 3 apps
                if self.target == TargetProfile::Ring3App && !self.function_offsets.contains_key(name) {
                    self.stdlib_imports.insert(name.clone());
                }
                // cleanup stack (args * 8 bytes)
                let n = args.len() as u32 * 8;
                if n > 0 {
                    if n <= 127 {
                        self.code.extend_from_slice(&[0x48, 0x83, 0xC4, n as u8]);
                    } else {
                        self.code.extend_from_slice(&[0x48, 0x81, 0xC4]);
                        self.code.extend_from_slice(&n.to_le_bytes());
                    }
                }
            }
            Expr::Syscall(def, args) => {
                // x86-64 SysV ABI syscall convention:
                // args: rdi, rsi, rdx, r10, r8, r9  →  result in rax
                let reg_mov: &[[u8; 3]] = &[
                    [0x48, 0x89, 0xC7], // mov rdi, rax
                    [0x48, 0x89, 0xC6], // mov rsi, rax
                    [0x48, 0x89, 0xC2], // mov rdx, rax
                    [0x49, 0x89, 0xC2], // mov r10, rax
                    [0x49, 0x89, 0xC0], // mov r8, rax
                    [0x49, 0x89, 0xC1], // mov r9, rax
                ];
                for (i, arg) in args.iter().enumerate() {
                    if i < 6 {
                        self.emit_expr(arg);          // rax = expr value
                        self.code.extend_from_slice(&reg_mov[i]); // mov reg, rax
                    }
                }
                self.code.extend_from_slice(&[0xB8]);        // mov eax, imm32
                self.code.extend_from_slice(&def.nr.to_le_bytes());
                self.emit_call_to_syscall_stub();
            }
            Expr::Assign(name, val) => {
                self.emit_expr(val);
                self.emit_store_var(name);
            }
            Expr::Neg(a) => { self.emit_expr(a); self.code.extend_from_slice(&[0x48, 0xF7, 0xD8]); }
            Expr::Not(a) => { self.emit_expr(a); self.code.extend_from_slice(&[0x85, 0xC0, 0x0F, 0x94, 0xC0]); }
            Expr::BitNot(a) => { self.emit_expr(a); self.code.extend_from_slice(&[0x48, 0xF7, 0xD0]); }
            Expr::PreInc(name) => {
                self.emit_inc_var(name);
                // rax already has new value
            }
            Expr::PreDec(name) => {
                self.emit_dec_var(name);
            }
            Expr::PostInc(name) => {
                self.emit_load_var(name);
                self.code.push(0x50); // push old value
                self.emit_inc_var(name);
                self.code.push(0x58); // pop rax (old value)
            }
            Expr::PostDec(name) => {
                self.emit_load_var(name);
                self.code.push(0x50);
                self.emit_dec_var(name);
                self.code.push(0x58);
            }
            Expr::Deref(a) => {
                self.emit_expr(a); // rax = address
                self.code.extend_from_slice(&[0x48, 0x8B, 0x00]); // mov rax, [rax]
            }
            Expr::AddrOf(inner) => {
                match inner.as_ref() {
                    Expr::Var(name) => {
                        if let Some(&(offset, _)) = self.var_offsets.get(name) {
                            if offset >= -128 && offset <= 127 {
                                self.code.extend_from_slice(&[0x48, 0x8D, 0x45, offset as u8]);
                            } else {
                                self.code.extend_from_slice(&[0x48, 0x8D, 0x85]);
                                self.code.extend_from_slice(&(offset as i32).to_le_bytes());
                            }
                        } else if self.global_offsets.contains_key(name) {
                            self.code.extend_from_slice(&[0x48, 0x8D, 0x05, 0, 0, 0, 0]);
                            self.global_fixups.push((self.code.len() - 4, name.clone()));
                        } else { self.emit_xor_eax(); }
                    }
                    Expr::Subscript(name, idx, scale) => {
                        // lea rax, [rbp + var_off + idx*scale]
                        let var_off = self.var_offsets.get(name).map(|&(off,_)| off).unwrap_or(0);
                        // compute index * scale
                        self.emit_expr(idx);
                        if *scale > 0 {
                            let shift = scale.trailing_zeros() as u8;
                            self.code.extend_from_slice(&[0x48, 0xC1, 0xE0, shift]);
                        }
                        self.code.push(0x50); // push scaled index
                        if var_off >= -128 && var_off <= 127 {
                            self.code.extend_from_slice(&[0x48, 0x8D, 0x45, var_off as u8]); // lea rax, [rbp+var_off]
                        } else {
                            self.code.extend_from_slice(&[0x48, 0x8D, 0x85]);
                            self.code.extend_from_slice(&(var_off as i32).to_le_bytes());
                        }
                        self.code.push(0x5A); // pop rdx = scaled index
                        self.code.extend_from_slice(&[0x48, 0x01, 0xD0]); // add rax, rdx
                    }
                    Expr::Deref(ptr) => {
                        self.emit_expr(ptr); // rax = address of the pointed-to data
                    }
                    _ => self.emit_xor_eax(),
                }
            }
            Expr::Subscript(name, index, scale) => {
                self.emit_load_var(name); // rax = base address (value of name, which is a pointer)
                self.code.push(0x50); // push base
                self.emit_expr(index); // rax = index
                if *scale > 0 {
                    let shift = scale.trailing_zeros() as u8;
                    self.code.extend_from_slice(&[0x48, 0xC1, 0xE0, shift]); // shl rax, scale
                }
                self.code.push(0x5A); // pop rdx = base
                self.code.extend_from_slice(&[0x48, 0x01, 0xD0]); // add rax, rdx => base + index*scale
                self.code.extend_from_slice(&[0x48, 0x8B, 0x00]); // mov rax, [rax]
            }
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
            Expr::Field(base, _field, offset) => {
                // compute base address, add field offset, load
                self.emit_expr_as_ptr(base);
                let off = *offset as i32;
                if off >= -128 && off <= 127 {
                    self.code.extend_from_slice(&[0x48, 0x83, 0xC0, off as u8]);
                } else {
                    self.code.extend_from_slice(&[0x48, 0x05]);
                    self.code.extend_from_slice(&(off as u32).to_le_bytes());
                }
                self.code.extend_from_slice(&[0x48, 0x8B, 0x00]);
            }
            Expr::Arrow(ptr, _field, offset) => {
                self.emit_expr(ptr);
                let off = *offset as i32;
                if off >= -128 && off <= 127 {
                    self.code.extend_from_slice(&[0x48, 0x83, 0xC0, off as u8]);
                } else {
                    self.code.extend_from_slice(&[0x48, 0x05]);
                    self.code.extend_from_slice(&(off as u32).to_le_bytes());
                }
                self.code.extend_from_slice(&[0x48, 0x8B, 0x00]);
            }
            Expr::AssignField(base, _field, offset, val) => {
                self.emit_expr(val);
                self.code.push(0x50);
                self.emit_expr_as_ptr(base);
                let off = *offset as i32;
                if off >= -128 && off <= 127 {
                    self.code.extend_from_slice(&[0x48, 0x83, 0xC0, off as u8]);
                } else {
                    self.code.extend_from_slice(&[0x48, 0x05]);
                    self.code.extend_from_slice(&(off as u32).to_le_bytes());
                }
                self.code.push(0x5A);
                self.code.extend_from_slice(&[0x48, 0x89, 0x10]);
                self.code.extend_from_slice(&[0x48, 0x89, 0xD0]);
            }
            Expr::AssignDeref(addr, val) => {
                self.emit_expr(val); // rax = value
                self.code.push(0x50); // push value
                self.emit_expr(addr); // rax = address
                self.code.push(0x5A); // pop rdx (value)
                self.code.extend_from_slice(&[0x48, 0x89, 0x10]); // mov [rax], rdx
                self.code.extend_from_slice(&[0x48, 0x89, 0xD0]); // mov rax, rdx (return value)
            }
            Expr::AssignArrow(ptr, _field, offset, val) => {
                self.emit_expr(val); // rax = value
                self.code.push(0x50); // push value
                self.emit_expr(ptr); // rax = pointer
                let off = *offset as i32;
                if off >= -128 && off <= 127 {
                    self.code.extend_from_slice(&[0x48, 0x83, 0xC0, off as u8]);
                } else {
                    self.code.extend_from_slice(&[0x48, 0x05]);
                    self.code.extend_from_slice(&(off as u32).to_le_bytes());
                }
                self.code.push(0x5A); // pop rdx (value)
                self.code.extend_from_slice(&[0x48, 0x89, 0x10]); // mov [rax], rdx
                self.code.extend_from_slice(&[0x48, 0x89, 0xD0]); // mov rax, rdx (return value)
            }
            Expr::Comma(exprs) => {
                for (i, e) in exprs.iter().enumerate() {
                    self.emit_expr(e);
                    if i < exprs.len() - 1 { self.emit_drop(); }
                }
            }
        }
    }

    /// Emit expression as an address (pointer), not as a value
    fn emit_expr_as_ptr(&mut self, expr: &Expr) {
        match expr {
            Expr::Var(name) => {
                if let Some(&(offset, _)) = self.var_offsets.get(name) {
                    if offset >= -128 && offset <= 127 {
                        self.code.extend_from_slice(&[0x48, 0x8D, 0x45, offset as u8]);
                    } else {
                        self.code.extend_from_slice(&[0x48, 0x8D, 0x85]);
                        self.code.extend_from_slice(&(offset as i32).to_le_bytes());
                    }
                } else if self.global_offsets.contains_key(name) {
                    self.code.extend_from_slice(&[0x48, 0x8D, 0x05, 0, 0, 0, 0]);
                    self.global_fixups.push((self.code.len() - 4, name.clone()));
                } else { self.emit_xor_eax(); }
            }
            Expr::Subscript(name, index, scale) => {
                let var_off = self.var_offsets.get(name).map(|&(off,_)| off).unwrap_or(0);
                self.emit_expr(index);
                if *scale > 0 {
                    let shift = scale.trailing_zeros() as u8;
                    self.code.extend_from_slice(&[0x48, 0xC1, 0xE0, shift]);
                }
                self.code.push(0x50);
                if var_off >= -128 && var_off <= 127 {
                    self.code.extend_from_slice(&[0x48, 0x8D, 0x45, var_off as u8]);
                } else {
                    self.code.extend_from_slice(&[0x48, 0x8D, 0x85]);
                    self.code.extend_from_slice(&(var_off as i32).to_le_bytes());
                }
                self.code.push(0x5A);
                self.code.extend_from_slice(&[0x48, 0x01, 0xD0]);
            }
            _ => self.emit_expr(expr),
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
        self.emit_call_to_syscall_stub();
    }

    fn emit_call_to_syscall_stub(&mut self) {
        if self.target == TargetProfile::Ring0Kernel {
            // Ring 0: inline syscall + ret (same 3 bytes, no call relocation needed)
            self.code.extend_from_slice(&[0x0F, 0x05, 0xC3]);
        } else {
            // Ring 3: call __bmo_syscall_stub via E8 rel32
            self.code.extend_from_slice(&[0xE8]);
            self.call_relocs.push(CallReloc { offset: self.code.len(), target: "__bmo_syscall_stub".to_string() });
            self.code.extend_from_slice(&[0, 0, 0, 0]);
        }
    }

    fn build_bef(&mut self) -> Vec<u8> {
        let all = core::mem::take(&mut self.code);
        let mut b = BefBuilder::new();

        let code_bytes = &all[..self.instruction_end];
        let rodata_bytes = &all[self.instruction_end..self.string_data_end];
        let data_bytes = &all[self.string_data_end..];

        let mut code_sec = BefSection::code(code_bytes.to_vec());
        code_sec.alignment = 1;
        b.add_section(code_sec);

        if !rodata_bytes.is_empty() {
            let mut rodata_sec = BefSection::rodata(rodata_bytes.to_vec());
            rodata_sec.alignment = 1;
            b.add_section(rodata_sec);
        }

        if !data_bytes.is_empty() {
            let mut data_sec = BefSection::data(data_bytes.to_vec());
            data_sec.alignment = 1;
            b.add_section(data_sec);
        }

        b.entry_offset = self.entry_offset as u64;
        b.build().unwrap_or_default()
    }
}
