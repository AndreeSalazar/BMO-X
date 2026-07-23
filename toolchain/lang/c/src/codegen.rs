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
    /// Nombres de TODAS las funciones del programa (para distinguir una
    /// llamada directa de una indirecta por puntero, y la decadencia
    /// función→dirección).
    known_functions: std::collections::HashSet<String>,
    /// Sitios donde hay que escribir la dirección (rip-relativa) de una
    /// función: `lea rax, [rip+func]`. Habilita punteros a función.
    func_addr_fixups: Vec<(usize, String)>,
    break_target: Vec<u32>,
    continue_target: Vec<u32>,
    var_offsets: HashMap<String, (i32, TypeSpec)>,
    // bytes de stack locales de la función actual (arrays/structs con tamaño REAL)
    frame_size: i32,
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
    /// Enum constants: name → integer value.
    enum_values: HashMap<String, i64>,
    /// Tabla de instrucciones sem-asm (opcodes leídos de la TOML de forge).
    isa: bmo_sem_asm::Instructions,
    /// Tabla de intrínsecos (la fusión __nombre() ↔ bytes exactos).
    intrinsics: bmo_sem_asm::Intrinsics,
    /// Errores acumulados durante la emisión (p.ej. intrínseco desconocido) —
    /// el compilador FALLA con mensaje, jamás emite bytes adivinados.
    errors: Vec<String>,
}

impl Codegen {
    /// Emite bytes con el encoder sem-asm (opcode de la tabla + REX/ModRM).
    fn emit_asm(&mut self, build: impl FnOnce(&mut bmo_sem_asm::x86_64::Asm)) {
        let mut a = bmo_sem_asm::x86_64::Asm::new(&self.isa);
        build(&mut a);
        self.code.extend_from_slice(a.bytes());
    }

    fn new(target: TargetProfile) -> Self {
        Self {
            target,
            code: Vec::new(), strings: Vec::new(), fixups: Vec::new(),
            labels: 0, pending_relocs: Vec::new(), call_relocs: Vec::new(),
            function_offsets: HashMap::new(),
            known_functions: std::collections::HashSet::new(),
            func_addr_fixups: Vec::new(),
            break_target: Vec::new(),
            continue_target: Vec::new(), var_offsets: HashMap::new(),
            frame_size: 0,
            struct_layouts: HashMap::new(), struct_sizes: HashMap::new(),
            label_positions: HashMap::new(), goto_relocs: Vec::new(),
            entry_offset: 0, is_entry_function: false,
            global_offsets: HashMap::new(), global_data: Vec::new(),
            global_fixups: Vec::new(),
            instruction_end: 0, string_data_end: 0,
            stdlib_imports: std::collections::HashSet::new(),
            enum_values: HashMap::new(),
            isa: bmo_sem_asm::Instructions::load_x86_64()
                .expect("tablas sem-asm x86-64 (forge/sem-asm/tables)"),
            intrinsics: bmo_sem_asm::Intrinsics::load_x86_64()
                .expect("tabla de intrínsecos x86-64 (forge/sem-asm/tables)"),
            errors: Vec::new(),
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
        // registrar todos los nombres de función ANTES de emitir: una llamada
        // puede referir a una función definida más abajo (forward reference).
        for func in &program.functions {
            self.known_functions.insert(func.name.clone());
        }
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
        self.patch_func_addr_fixups();
        self.patch_goto_relocs();
        self.patch_all_fixups();
        // errores acumulados en la emisión: fallar con claridad, no callar
        if !self.errors.is_empty() {
            return Err(CError::new(0, self.errors.join("; ")));
        }
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
            TypeSpec::Array(t, n) => self.type_stack_size(t) * n,
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
            Expr::Arrow(p,_,_,_) => self.collect_expr_strings(p),
            Expr::AssignArrow(p,_,_,_,v) => { self.collect_expr_strings(p); self.collect_expr_strings(v); }
            Expr::Assign(_, v) | Expr::AssignField(_,_,_,_,v) => self.collect_expr_strings(v),
            Expr::Cast(_, a) => self.collect_expr_strings(a),
            Expr::Intrinsic(_, args) => { for a in args { self.collect_expr_strings(a); } }
            Expr::AssignDeref(a, v) => { self.collect_expr_strings(a); self.collect_expr_strings(v); }
            Expr::Field(b,_,_,_) => self.collect_expr_strings(b),
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

    /// Escribe la dirección rip-relativa de cada función referida por un
    /// `lea rax, [rip+func]` (punteros a función). Mismo esquema que las
    /// call relocs: displacement dentro de la sección de código.
    fn patch_func_addr_fixups(&mut self) {
        for (off, name) in &self.func_addr_fixups {
            if let Some(&target) = self.function_offsets.get(name) {
                let disp = target as i32 - (*off as i32 + 4);
                self.code[*off..*off + 4].copy_from_slice(&disp.to_le_bytes());
            } else {
                self.errors.push(format!("no existe la funcion '{name}' cuya direccion se tomo"));
            }
        }
    }

    /// `lea rax, [rip+func]` — deja en rax la dirección de una función.
    fn emit_func_addr(&mut self, name: &str) {
        self.code.extend_from_slice(&[0x48, 0x8D, 0x05, 0, 0, 0, 0]);
        self.func_addr_fixups.push((self.code.len() - 4, name.to_string()));
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

    /// Recolecta TODAS las DeclAssign del cuerpo, a cualquier profundidad.
    /// Antes solo se miraba el nivel superior: una `int i` dentro de un
    /// for/if/bloque NO recibía slot — stores descartados, loads = 0.
    fn collect_decls_stmt<'a>(s: &'a Stmt, out: &mut Vec<(&'a String, &'a TypeSpec)>) {
        match s {
            Stmt::DeclAssign(t, n, _) => out.push((n, t)),
            Stmt::Block(v) => for x in v { Self::collect_decls_stmt(x, out); },
            Stmt::If(_, a, b) => {
                Self::collect_decls_stmt(a, out);
                if let Some(b) = b { Self::collect_decls_stmt(b, out); }
            }
            Stmt::While(_, b) | Stmt::DoWhile(b, _) | Stmt::For(_, _, _, b) => Self::collect_decls_stmt(b, out),
            Stmt::Switch(_, cases) => for c in cases { for st in &c.stmts { Self::collect_decls_stmt(st, out); } },
            _ => {}
        }
    }

    fn build_var_map(&mut self, params: &[Param], var_names: &[String], func: &Function) {
        self.var_offsets.clear();
        // parámetros: slots de 8 en [rbp+16+i*8] (convención de llamada propia)
        for (i, p) in params.iter().enumerate() {
            let off = 16 + i as i32 * 8;
            self.var_offsets.insert(p.name.clone(), (off, p.typ.clone()));
        }
        // locales: tamaño REAL del tipo (arrays y structs incluidos), alineado a 8
        let mut decls = Vec::new();
        for stmt in &func.body { Self::collect_decls_stmt(stmt, &mut decls); }
        let mut cur: i32 = 0;
        for (name, typ) in &decls {
            if self.var_offsets.contains_key(*name) { continue; } // sombra: un solo slot
            let sz = self.type_stack_size(typ).max(8);
            let sz = ((sz + 7) / 8 * 8) as i32;
            cur -= sz;
            self.var_offsets.insert((*name).clone(), (cur, (*typ).clone()));
        }
        // legado: nombres registrados por el parser sin DeclAssign visible
        for name in var_names.iter().skip(params.len()) {
            if !self.var_offsets.contains_key(name) {
                cur -= 8;
                self.var_offsets.insert(name.clone(), (cur, TypeSpec::Long));
            }
        }
        self.frame_size = -cur;
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
        // Enum constants: emit integer literal directly
        if let Some(&val) = self.enum_values.get(name) {
            self.code.extend_from_slice(&[0xB8]); // mov eax, imm32
            self.code.extend_from_slice(&(val as i32).to_le_bytes());
            return;
        }
        // Función usada como VALOR (fp = myfunc): decae a su dirección.
        if self.known_functions.contains(name)
            && !self.var_offsets.contains_key(name)
            && !self.global_offsets.contains_key(name)
        {
            self.emit_func_addr(name);
            return;
        }
        // Arrays: decaen a puntero — "cargar" arr es su DIRECCIÓN, no su contenido
        if self.var_is_array(name) {
            if let Some(&(off, _)) = self.var_offsets.get(name) {
                if off >= -128 && off <= 127 {
                    self.code.extend_from_slice(&[0x48, 0x8D, 0x45, off as u8]); // lea rax,[rbp+off]
                } else {
                    self.code.extend_from_slice(&[0x48, 0x8D, 0x85]);
                    self.code.extend_from_slice(&off.to_le_bytes());
                }
            } else {
                self.code.extend_from_slice(&[0x48, 0x8D, 0x05, 0, 0, 0, 0]);
                self.global_fixups.push((self.code.len() - 4, name.to_string()));
            }
            return;
        }
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
        // allocate local var space — tamaño REAL calculado por build_var_map
        // (antes: var_count*8, y los arrays/structs pisaban a sus vecinos)
        let _ = param_count;
        let stack_size = self.frame_size;
        if stack_size > 0 {
            if stack_size <= 127 {
                self.code.extend_from_slice(&[0x48, 0x83, 0xEC, stack_size as u8]);
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

    fn emit_printf_variadic(&mut self, args: &[Expr]) {
        // printf(fmt, a1, a2, ...)
        // Emit: RDI = fmt, push va_args right-to-left, RSI = RSP, RDX = num_va_args, call bmo_printf
        let va_args = &args[1..];
        let num_va = va_args.len() as u64;

        // Push variadic args right-to-left (so they're in order on stack)
        for arg in va_args.iter().rev() {
            self.emit_expr(arg);
            self.code.push(0x50); // push rax
        }

        // RDI = format string (first arg)
        // The format string Expr is either StringLit or Var
        self.emit_expr(&args[0]);
        // After emit_expr, value is in RAX. Move to RDI.
        self.code.extend_from_slice(&[0x48, 0x89, 0xC7]); // mov rdi, rax

        // RSI = RSP (pointer to first va_arg on stack)
        self.code.extend_from_slice(&[0x48, 0x89, 0xE6]); // mov rsi, rsp

        // RDX = number of va_args
        self.code.extend_from_slice(&[0xBA]); // mov edx, imm32
        self.code.extend_from_slice(&(num_va as u32).to_le_bytes());

        // Call bmo_printf from userland_ring3
        self.code.extend_from_slice(&[0xE8]);
        self.call_relocs.push(CallReloc { offset: self.code.len(), target: "bmo_printf".to_string() });
        self.code.extend_from_slice(&[0, 0, 0, 0]);

        if self.target == TargetProfile::Ring3App {
            self.stdlib_imports.insert("bmo_printf".to_string());
        }

        // Cleanup stack
        let n = num_va as u32 * 8;
        if n > 0 {
            if n <= 127 {
                self.code.extend_from_slice(&[0x48, 0x83, 0xC4, n as u8]);
            } else {
                self.code.extend_from_slice(&[0x48, 0x81, 0xC4]);
                self.code.extend_from_slice(&n.to_le_bytes());
            }
        }
    }
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
                let v = *n as u64;
                self.emit_asm(|a| { a.mov_imm64(bmo_sem_asm::x86_64::Reg::Rax, v).unwrap(); });
            }
            Expr::CharLit(c) => {
                let v = *c as u64;
                self.emit_asm(|a| { a.mov_imm64(bmo_sem_asm::x86_64::Reg::Rax, v).unwrap(); });
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
                // Special case: printf → emit bmo_printf from userland_ring3
                if name == "printf" && !args.is_empty() {
                    self.emit_printf_variadic(args);
                    return;
                }
                // ¿Llamada INDIRECTA? El nombre no es una función pero SÍ una
                // variable → contiene una dirección (puntero a función).
                let is_indirect = !self.known_functions.contains(name)
                    && (self.var_offsets.contains_key(name) || self.global_offsets.contains_key(name));

                // push args right-to-left
                for arg in args.iter().rev() {
                    self.emit_expr(arg);
                    self.code.push(0x50); // push rax
                }
                if is_indirect {
                    self.emit_load_var(name);                 // rax = dirección
                    self.code.extend_from_slice(&[0xFF, 0xD0]); // call rax
                } else {
                    // call rel32 placeholder (directa)
                    self.code.extend_from_slice(&[0xE8]);
                    self.call_relocs.push(CallReloc { offset: self.code.len(), target: name.clone() });
                    self.code.extend_from_slice(&[0, 0, 0, 0]);
                    // Track stdlib imports for Ring 3 apps
                    if self.target == TargetProfile::Ring3App && !self.function_offsets.contains_key(name) {
                        self.stdlib_imports.insert(name.clone());
                    }
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
                // args: rdi, rsi, rdx, r10, r8, r9  →  result in rax.
                // El `mov <reg>, rax` lo emite el encoder sem-asm (antes era
                // la tabla reg_mov de bytes a mano — misma dup que COBOL).
                use bmo_sem_asm::x86_64::Reg;
                const ARG_REGS: [Reg; 6] =
                    [Reg::Rdi, Reg::Rsi, Reg::Rdx, Reg::R10, Reg::R8, Reg::R9];
                for (i, arg) in args.iter().enumerate() {
                    if i < 6 {
                        self.emit_expr(arg);          // rax = expr value
                        let dst = ARG_REGS[i];
                        self.emit_asm(|a| { a.mov_reg(dst, Reg::Rax).unwrap(); });
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
                        } else if self.known_functions.contains(name) {
                            // &myfunc — dirección de la función
                            self.emit_func_addr(name);
                        } else { self.emit_xor_eax(); }
                    }
                    Expr::Subscript(name, idx, scale) => {
                        self.emit_subscript_addr(name, idx, *scale);
                    }
                    Expr::Deref(ptr) => {
                        self.emit_expr(ptr); // rax = address of the pointed-to data
                    }
                    _ => self.emit_xor_eax(),
                }
            }
            Expr::Subscript(name, index, scale) => {
                // dirección exacta (array o puntero) + carga del TAMAÑO del elemento
                self.emit_subscript_addr(name, index, *scale);
                let elem = self.elem_type_of(name);
                self.emit_load_elem(&elem);
            }
            Expr::AssignSubscript(name, index, scale, val) => {
                self.emit_expr(val);          // rax = valor
                self.code.push(0x50);         // push valor
                self.emit_subscript_addr(name, index, *scale); // rax = dirección
                self.code.push(0x5A);         // pop rdx = valor
                let elem = self.elem_type_of(name);
                self.emit_store_elem(&elem);  // [rax] = rdx (tamaño exacto)
                self.code.extend_from_slice(&[0x48, 0x89, 0xD0]); // rax = valor (resultado del assign)
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
            Expr::Field(base, _field, offset, ftyp) => {
                // dirección base + offset, carga del TAMAÑO/SIGNO del campo
                self.emit_expr_as_ptr(base);
                self.emit_add_offset(*offset);
                self.emit_load_elem(&ftyp.clone());
            }
            Expr::Arrow(ptr, _field, offset, ftyp) => {
                self.emit_expr(ptr);
                self.emit_add_offset(*offset);
                self.emit_load_elem(&ftyp.clone());
            }
            Expr::AssignField(base, _field, offset, ftyp, val) => {
                self.emit_expr(val);
                self.code.push(0x50);
                self.emit_expr_as_ptr(base);
                self.emit_add_offset(*offset);
                self.code.push(0x5A);
                // store del TAMAÑO exacto: pt.x=10 con x:int ya no pisa a pt.y
                self.emit_store_elem(&ftyp.clone());
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
            Expr::AssignArrow(ptr, _field, offset, ftyp, val) => {
                self.emit_expr(val); // rax = value
                self.code.push(0x50); // push value
                self.emit_expr(ptr); // rax = pointer
                self.emit_add_offset(*offset);
                self.code.push(0x5A); // pop rdx (value)
                self.emit_store_elem(&ftyp.clone()); // tamaño exacto del campo
                self.code.extend_from_slice(&[0x48, 0x89, 0xD0]); // mov rax, rdx
            }
            Expr::Intrinsic(name, args) => self.emit_intrinsic(name, args),
            Expr::Cast(t, inner) => {
                // cast REAL: trunca/extiende rax al tamaño del tipo destino.
                // Antes era no-op: (char)300 quedaba como 300.
                self.emit_expr(inner);
                match t {
                    TypeSpec::Char => self.code.extend_from_slice(&[0x48, 0x0F, 0xBE, 0xC0]), // movsx rax, al
                    TypeSpec::UnsignedChar => self.code.extend_from_slice(&[0x48, 0x0F, 0xB6, 0xC0]), // movzx
                    TypeSpec::Short => self.code.extend_from_slice(&[0x48, 0x0F, 0xBF, 0xC0]),
                    TypeSpec::UnsignedShort => self.code.extend_from_slice(&[0x48, 0x0F, 0xB7, 0xC0]),
                    TypeSpec::Int => self.code.extend_from_slice(&[0x48, 0x63, 0xC0]), // movsxd rax, eax
                    TypeSpec::UnsignedInt => self.code.extend_from_slice(&[0x89, 0xC0]), // mov eax, eax (zero-ext)
                    _ => {} // 64-bit y punteros: sin cambio de representación
                }
            }
            Expr::Comma(exprs) => {
                for (i, e) in exprs.iter().enumerate() {
                    self.emit_expr(e);
                    if i < exprs.len() - 1 { self.emit_drop(); }
                }
            }
        }
    }

    // ---- Subscript helpers (array en memoria vs puntero-valor) ----

    /// ¿`name` es un array (su memoria vive en el slot) o un puntero (el slot
    /// guarda una dirección)? La distinción que antes no existía y corrompía.
    fn var_is_array(&self, name: &str) -> bool {
        if let Some(&(_, ref t)) = self.var_offsets.get(name) { return matches!(t, TypeSpec::Array(_, _)); }
        if let Some(&(_, ref t)) = self.global_offsets.get(name) { return matches!(t, TypeSpec::Array(_, _)); }
        false
    }

    /// Tipo del elemento de un array/puntero (para cargas/stores del tamaño exacto).
    fn elem_type_of(&self, name: &str) -> TypeSpec {
        let t = self.var_offsets.get(name).map(|&(_, ref t)| t.clone())
            .or_else(|| self.global_offsets.get(name).map(|&(_, ref t)| t.clone()));
        match t {
            Some(TypeSpec::Array(e, _)) | Some(TypeSpec::Ptr(e)) => *e,
            _ => TypeSpec::Long,
        }
    }

    /// rax = rax * scale (shl si es potencia de 2; imul si no — structs)
    fn emit_scale_index(&mut self, scale: u8) {
        if scale > 1 {
            if scale.is_power_of_two() {
                self.code.extend_from_slice(&[0x48, 0xC1, 0xE0, scale.trailing_zeros() as u8]);
            } else {
                self.code.extend_from_slice(&[0x48, 0x6B, 0xC0, scale]); // imul rax, rax, imm8
            }
        }
    }

    /// rax = dirección de name[idx]. Array → base = lea del slot;
    /// puntero → base = VALOR del slot. Local o global.
    fn emit_subscript_addr(&mut self, name: &str, index: &Expr, scale: u8) {
        self.emit_expr(index);
        self.emit_scale_index(scale);
        self.code.push(0x50); // push índice escalado
        if self.var_is_array(name) {
            if let Some(&(off, _)) = self.var_offsets.get(name) {
                if off >= -128 && off <= 127 {
                    self.code.extend_from_slice(&[0x48, 0x8D, 0x45, off as u8]); // lea rax,[rbp+off]
                } else {
                    self.code.extend_from_slice(&[0x48, 0x8D, 0x85]);
                    self.code.extend_from_slice(&off.to_le_bytes());
                }
            } else {
                self.code.extend_from_slice(&[0x48, 0x8D, 0x05, 0, 0, 0, 0]); // lea rax,[rip+global]
                self.global_fixups.push((self.code.len() - 4, name.to_string()));
            }
        } else {
            self.emit_load_var(name); // rax = valor del puntero
        }
        self.code.push(0x5A); // pop rdx = índice escalado
        self.code.extend_from_slice(&[0x48, 0x01, 0xD0]); // add rax, rdx
    }

    /// rax += offset (encoding corto si cabe en imm8)
    fn emit_add_offset(&mut self, offset: u32) {
        if offset == 0 { return; }
        let off = offset as i32;
        if off <= 127 {
            self.code.extend_from_slice(&[0x48, 0x83, 0xC0, off as u8]);
        } else {
            self.code.extend_from_slice(&[0x48, 0x05]);
            self.code.extend_from_slice(&(off as u32).to_le_bytes());
        }
    }

    /// Carga [rax] → rax con el tamaño y signo EXACTOS del elemento.
    /// Antes siempre era `mov rax,[rax]` (8 bytes): leer int[i] traía basura vecina.
    fn emit_load_elem(&mut self, elem: &TypeSpec) {
        match elem {
            // agregados: la dirección ES el valor (a.b.c anidado, arrays en structs)
            TypeSpec::Array(_, _) | TypeSpec::StructRef(_) | TypeSpec::UnionRef(_) => {}
            TypeSpec::Char => self.code.extend_from_slice(&[0x48, 0x0F, 0xBE, 0x00]), // movsx rax, byte
            TypeSpec::UnsignedChar => self.code.extend_from_slice(&[0x48, 0x0F, 0xB6, 0x00]), // movzx
            TypeSpec::Short => self.code.extend_from_slice(&[0x48, 0x0F, 0xBF, 0x00]),
            TypeSpec::UnsignedShort => self.code.extend_from_slice(&[0x48, 0x0F, 0xB7, 0x00]),
            TypeSpec::Int => self.code.extend_from_slice(&[0x48, 0x63, 0x00]), // movsxd rax, dword
            TypeSpec::UnsignedInt | TypeSpec::Float => self.code.extend_from_slice(&[0x8B, 0x00]), // mov eax, dword
            _ => self.code.extend_from_slice(&[0x48, 0x8B, 0x00]), // mov rax, qword
        }
    }

    /// Guarda rdx → [rax] con el tamaño EXACTO del elemento.
    /// Antes un store de 8 bytes a int[i] pisaba el elemento siguiente.
    fn emit_store_elem(&mut self, elem: &TypeSpec) {
        match self.type_stack_size(elem) {
            1 => self.code.extend_from_slice(&[0x88, 0x10]),        // mov [rax], dl
            2 => self.code.extend_from_slice(&[0x66, 0x89, 0x10]),  // mov [rax], dx
            4 => self.code.extend_from_slice(&[0x89, 0x10]),        // mov [rax], edx
            _ => self.code.extend_from_slice(&[0x48, 0x89, 0x10]),  // mov [rax], rdx
        }
    }

    /// LA FUSIÓN sem-asm↔C: emite un intrínseco de la tabla.
    /// Evalúa cada argumento, lo apila, y lo vuelca al registro que dicta
    /// la tabla justo antes de los bytes de la instrucción. Bytes EXACTOS,
    /// sin caja negra: si el nombre o la aridad no cuadran → error, no adivina.
    fn emit_intrinsic(&mut self, name: &str, args: &[Expr]) {
        let Some(def) = self.intrinsics.get(name) else {
            self.errors.push(format!(
                "intrinsic __{name}() no existe en la tabla sem-asm (tables/arch/x86_64/intrinsics.toml)"));
            return;
        };
        if args.len() != def.args.len() {
            self.errors.push(format!(
                "intrinsic __{name}() espera {} argumento(s), recibio {}",
                def.args.len(), args.len()));
            return;
        }
        let bytes = def.bytes.clone();
        let arg_regs = def.args.clone();
        let returns = def.returns.clone();

        // 1) evaluar cada argumento a rax y apilarlo (orden de aparición)
        for a in args {
            self.emit_expr(a);
            self.code.push(0x50); // push rax
        }
        // 2) volcar a los registros destino, en REVERSA (el tope es el último
        //    arg). Cada destino es un registro DISTINTO (rax/rcx/rdx) → pop
        //    directo sin pisarse.
        for reg in arg_regs.iter().rev() {
            self.emit_pop_to_reg(reg);
        }
        // 3) los bytes exactos de la instrucción
        self.code.extend_from_slice(&bytes);
        // 4) normalizar el valor de retorno a rax
        self.emit_intrinsic_return(returns.as_deref());
    }

    /// Saca el tope de la pila al registro destino de un argumento.
    fn emit_pop_to_reg(&mut self, reg: &str) {
        match reg {
            "eax" | "ax" | "al" => self.code.push(0x58),          // pop rax
            "ecx" | "cx" | "cl" => self.code.push(0x59),          // pop rcx
            "edx" | "dx"        => self.code.push(0x5A),          // pop rdx
            "u64_edx_eax" => {
                // valor de 64 bits en rax → edx:eax (para wrmsr)
                self.code.push(0x58);                              // pop rax
                self.code.extend_from_slice(&[0x48, 0x89, 0xC2]); // mov rdx, rax
                self.code.extend_from_slice(&[0x48, 0xC1, 0xEA, 0x20]); // shr rdx, 32
            }
            _ => self.errors.push(format!("registro de argumento desconocido: {reg}")),
        }
    }

    /// Deja el resultado del intrínseco limpio en rax según de dónde salga.
    fn emit_intrinsic_return(&mut self, returns: Option<&str>) {
        match returns {
            Some("u64_edx_eax") => {
                self.code.extend_from_slice(&[0x48, 0xC1, 0xE2, 0x20]); // shl rdx, 32
                self.code.extend_from_slice(&[0x48, 0x09, 0xD0]);       // or rax, rdx
            }
            Some("al") => self.code.extend_from_slice(&[0x48, 0x0F, 0xB6, 0xC0]), // movzx rax, al
            Some("ax") => self.code.extend_from_slice(&[0x48, 0x0F, 0xB7, 0xC0]), // movzx rax, ax
            // "eax": escribir eax en modo 64-bit ya deja rax con el alto en cero
            _ => {}
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
                self.emit_subscript_addr(name, index, *scale);
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
