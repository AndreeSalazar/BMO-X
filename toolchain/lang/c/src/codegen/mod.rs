use std::collections::HashMap;
use bmo_abi::bef::writer::{BefBuilder, BefSection};
use crate::ast::*;
use crate::CError;

/// Structs y uniones POR VALOR, en su propio fichero. Ver su cabecera para
/// la ABI de agregados de BMO y para que hacen SysV y Win64 con esto mismo.
mod agregados;
/// La ENTRADA de C (`getchar`, `scanf`), tambien aparte. Escribir es empujar
/// bytes; leer es ESPERAR, guardar lo que sobra y decidir que significa lo que
/// alguien tecleo. Tres problemas que la salida no tiene.
mod entrada;

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
    /// Offset donde quedo fijada cada etiqueta (para saltos hacia atras).
    label_offsets: HashMap<u32, usize>,
    pending_relocs: Vec<PendingReloc>,
    call_relocs: Vec<CallReloc>,
    function_offsets: HashMap<String, usize>,
    /// Nombres de TODAS las funciones del programa (para distinguir una
    /// llamada directa de una indirecta por puntero, y la decadencia
    /// función→dirección).
    known_functions: std::collections::HashSet<String>,
    /// Los TIPOS de los parametros y del retorno de cada funcion.
    ///
    /// Hacia falta desde que un argumento puede no caber en un registro: el
    /// llamante tiene que saber cuantas ranuras empuja, y eso lo dice el
    /// PARAMETRO, no la expresion que se le pasa. Antes solo se guardaban los
    /// nombres, asi que pasar un struct empujaba una palabra y la funcion
    /// recibia el primer campo y basura detras — sin una palabra de aviso.
    firmas: std::collections::HashMap<String, (Vec<TypeSpec>, TypeSpec)>,
    /// Sitios donde hay que escribir la dirección (rip-relativa) de una
    /// función: `lea rax, [rip+func]`. Habilita punteros a función.
    func_addr_fixups: Vec<(usize, String)>,
    break_target: Vec<u32>,
    continue_target: Vec<u32>,
    var_offsets: HashMap<String, (i32, TypeSpec)>,
    // bytes de stack locales de la función actual (arrays/structs con tamaño REAL)
    frame_size: i32,
    /// ¿La función que se está emitiendo declara `...`?
    es_variadica: bool,
    /// Ranuras que ocupan sus parámetros CON NOMBRE. Justo detrás empiezan los
    /// variádicos, porque los argumentos van seguidos en la pila.
    ranuras_con_nombre: i32,
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
            labels: 0, label_offsets: HashMap::new(), pending_relocs: Vec::new(), call_relocs: Vec::new(),
            function_offsets: HashMap::new(),
            known_functions: std::collections::HashSet::new(),
            firmas: std::collections::HashMap::new(),
            func_addr_fixups: Vec::new(),
            break_target: Vec::new(),
            continue_target: Vec::new(), var_offsets: HashMap::new(),
            frame_size: 0,
            es_variadica: false,
            ranuras_con_nombre: 0,
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
            self.firmas.insert(
                func.name.clone(),
                (
                    func.params.iter().map(|p| p.typ.clone()).collect(),
                    func.ret_type.clone(),
                ),
            );
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
        // Saltos hacia atras (bucles): se resuelven aqui, cuando ya se
        // conocen todas las etiquetas.
        self.patch_backward_relocs();
        // patch all call relocs
        self.patch_call_relocs();
        self.patch_func_addr_fixups();
        self.patch_goto_relocs();
        self.patch_all_fixups();
        // Errores acumulados durante la emisión: fallar con claridad, no
        // entregar un binario que hace algo distinto de lo escrito.
        if let Some(message) = self.errors.first() {
            return Err(CError::new(0, message.clone()));
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
            // ★ LAS CONDICIONES TAMBIÉN. Estaban descartadas —el `_` de cada
            // una— así que un literal dentro de una condición nunca entraba en
            // la tabla, y al emitirlo el `unwrap_or(0)` lo hacía apuntar **a la
            // primera cadena del programa**.
            //
            // Llevaba ahí desde siempre y no se veía porque hasta hoy no había
            // forma de poner un literal en una condición: hacía falta algo como
            // `if (strcmp(s, "salir") == 0)`, y `strcmp` no existía. El primer
            // test que lo pisó decía `menor` y no imprimía nada — comparando
            // "abc" contra el formato de un `printf` anterior.
            //
            // Un `unwrap_or(0)` sobre una tabla de direcciones es exactamente
            // la clase de fallo silencioso que este compilador no cuenta: no
            // falla, apunta a otro sitio.
            Stmt::If(c, t, e) => {
                self.collect_expr_strings(c);
                self.collect_stmt_strings(t);
                if let Some(el) = e { self.collect_stmt_strings(el); }
            }
            Stmt::While(c, b) => { self.collect_expr_strings(c); self.collect_stmt_strings(b); }
            Stmt::DoWhile(b, c) => { self.collect_stmt_strings(b); self.collect_expr_strings(c); }
            Stmt::For(ini, cond, paso, b) => {
                if let Some(e) = ini { self.collect_expr_strings(e); }
                if let Some(e) = cond { self.collect_expr_strings(e); }
                if let Some(e) = paso { self.collect_expr_strings(e); }
                self.collect_stmt_strings(b);
            }
            Stmt::Switch(c, cases) => {
                self.collect_expr_strings(c);
                for c in cases { for s in &c.stmts { self.collect_stmt_strings(s); } }
            }
            Stmt::Block(stmts) => { for s in stmts { self.collect_stmt_strings(s); } }
            Stmt::Expr(e) | Stmt::Return(Some(e)) => { self.collect_expr_strings(e); }
            Stmt::DeclAssign(_, _, Some(e)) => { self.collect_expr_strings(e); }
            // Sin esto, un `%s` dentro de una lista de inicializacion
            // apuntaria a una cadena que nunca se puso en .rodata.
            Stmt::DeclInit(_, _, es) => { for e in es { self.collect_expr_strings(&e.valor); } }
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
            Expr::IndexPtr(b, idx, _) => { self.collect_expr_strings(b); self.collect_expr_strings(idx); }
            Expr::AssignIndexPtr(b, idx, _, v) => { self.collect_expr_strings(b); self.collect_expr_strings(idx); self.collect_expr_strings(v); }
            Expr::CallPtr(c, args) => { self.collect_expr_strings(c); for a in args { self.collect_expr_strings(a); } }
            Expr::AssignDeref(a, v) => { self.collect_expr_strings(a); self.collect_expr_strings(v); }
            Expr::Field(b,_,_,_) => self.collect_expr_strings(b),
            Expr::Comma(v) => { for e in v { self.collect_expr_strings(e); } }
            _ => {}
        }
    }

    /// Rellena hasta la siguiente frontera de página con `int3`.
    ///
    /// Si el CPU llegara aquí es que se salió del código: `int3` lo detiene
    /// en seco en vez de deslizarse por ceros hasta cualquier parte.
    fn pad_to_page(code: &mut Vec<u8>) {
        const PAGE: usize = 4096;
        while code.len() % PAGE != 0 {
            code.push(0xCC);
        }
    }

    /// Coloca las cadenas y los globales, y parchea los `lea [rip+disp]`
    /// que los alcanzan.
    ///
    /// # Por qué hay relleno a página
    ///
    /// Estos desplazamientos se calculan asumiendo que los datos van
    /// PEGADOS detrás del código. Pero el cargador del kernel
    /// (`ring0/proc.rs`) coloca cada sección en la PÁGINA siguiente:
    /// `va_cursor = va_start + pages * PAGE`. Con el código a 500 bytes, el
    /// compilador apunta al byte 500 y el cargador pone la cadena en el
    /// 4096 — un `%s` leería basura EN HARDWARE.
    ///
    /// Rellenando cada tramo hasta una página, las dos cuentas coinciden.
    /// La solución definitiva son relocations en el BEF; esto es el acuerdo
    /// correcto mientras no existan, y no depende de que el cargador cambie.
    ///
    /// NOTA: esto NO lo puede detectar el emulador de pruebas, porque allí
    /// las secciones se concatenan tal cual. Es un fallo que solo aparece en
    /// metal — la razón por la que un banco de pruebas localiza bugs pero no
    /// sustituye a arrancar la máquina.
    fn patch_all_fixups(&mut self) {
        Self::pad_to_page(&mut self.code);
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
        Self::pad_to_page(&mut self.code);
        self.string_data_end = self.code.len();
        let global_base = self.code.len();
        for &(lea_offset, ref name) in &self.global_fixups {
            if let Some(&(data_off, _)) = self.global_offsets.get(name) {
                let rip = lea_offset + 4;
                let disp = (global_base as i64 + data_off as i64) - rip as i64;
                self.code[lea_offset..lea_offset + 4].copy_from_slice(&(disp as i32).to_le_bytes());
            }
        }
        let globals = core::mem::take(&mut self.global_data);
        self.code.extend_from_slice(&globals);
        self.global_data = globals;
        Self::pad_to_page(&mut self.code);
    }

    fn patch_goto_relocs(&mut self) {
        for (off, label) in &self.goto_relocs {
            if let Some(&target) = self.label_positions.get(label) {
                let disp = target as i32 - (*off as i32 + 4);
                self.code[*off..*off + 4].copy_from_slice(&disp.to_le_bytes());
            }
        }
    }

    /// Escribe el destino de cada `call rel32`.
    ///
    /// ★ Una llamada sin destino es un ERROR, no un hueco.
    ///
    /// Antes el `if let` no tenía `else`: el desplazamiento se quedaba en 0, y
    /// `E8 00000000` es "llama a la instrucción siguiente" — o sea, un `call`
    /// que empuja una dirección de retorno, no hace nada y vuelve. Un nombre mal
    /// escrito, o una macro con parámetros que este preprocesador todavía no
    /// expande, producía un programa que compilaba y **se saltaba la llamada en
    /// silencio**.
    ///
    /// Aquí no hay enlazado que pueda rellenarlo más tarde: no existe tabla de
    /// importaciones en la salida de este codegen, así que todo lo que se llama
    /// tiene que estar en esta misma unidad. La prueba de que era un descuido y
    /// no una decisión está tres funciones más abajo: `patch_func_addr_fixups`
    /// ya reportaba exactamente este caso para los punteros a función.
    fn patch_call_relocs(&mut self) {
        let mut faltan: Vec<String> = Vec::new();
        for reloc in &self.call_relocs {
            if let Some(&target_offset) = self.function_offsets.get(&reloc.target) {
                let off = reloc.offset;
                let disp = target_offset as i32 - (off as i32 + 4);
                self.code[off..off + 4].copy_from_slice(&disp.to_le_bytes());
            } else if !faltan.contains(&reloc.target) {
                faltan.push(reloc.target.clone());
            }
        }
        for nombre in faltan {
            self.errors.push(format!(
                "no existe la funcion '{nombre}' que se llama (aqui no hay enlazado: \
                 todo lo que se llama tiene que estar en esta unidad)"
            ));
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

    /// Fija una etiqueta en la posición actual y resuelve los saltos que ya
    /// la esperaban.
    ///
    /// El `label_offsets` es lo que faltaba: antes esta función SOLO
    /// parcheaba los saltos pendientes en ese instante, así que un salto
    /// emitido DESPUÉS de fijar la etiqueta —es decir, todo salto hacia
    /// atrás— se quedaba con desplazamiento 0 para siempre. Eso significa
    /// "seguir a la instrucción siguiente": **ningún bucle de C daba más de
    /// una vuelta**. `while`, `for`, `do-while`, y por tanto `break` y
    /// `continue`, ejecutaban el cuerpo exactamente una vez y salían. El
    /// binario compilaba y validaba igual.
    ///
    /// Es el mismo defecto que tenía el `IF` de COBOL, en otro lenguaje.
    fn resolve_label(&mut self, label: u32) {
        let here = self.code.len();
        self.label_offsets.insert(label, here);
        let mut i = 0;
        while i < self.pending_relocs.len() {
            if self.pending_relocs[i].target_label == label {
                let off = self.pending_relocs[i].offset;
                let disp = here as i32 - (off as i32 + 4);
                self.code[off..off + 4].copy_from_slice(&disp.to_le_bytes());
                self.pending_relocs.swap_remove(i);
            } else { i += 1; }
        }
    }

    /// Resuelve los saltos que quedaron pendientes: los que apuntan a una
    /// etiqueta fijada ANTES de emitirlos (saltos hacia atrás).
    ///
    /// Una etiqueta usada y jamás fijada es un bug del emisor: se aborta en
    /// vez de dejar un salto a ninguna parte.
    fn patch_backward_relocs(&mut self) {
        for reloc in std::mem::take(&mut self.pending_relocs) {
            let target = *self
                .label_offsets
                .get(&reloc.target_label)
                .unwrap_or_else(|| panic!("etiqueta {} usada pero nunca fijada", reloc.target_label));
            let disp = target as i32 - (reloc.offset as i32 + 4);
            self.code[reloc.offset..reloc.offset + 4].copy_from_slice(&disp.to_le_bytes());
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
            // El hueco en la pila. Sin esta linea la variable caia al
            // reparto de legado (8 bytes, tipo Long) y un struct de 16
            // habria escrito sobre la de al lado.
            Stmt::DeclInit(t, n, _) => out.push((n, t)),
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
        // ── Los parámetros, en la pila del llamante ──
        //
        // Empiezan en `[rbp+16]` (detrás de la dirección de retorno y del `rbp`
        // guardado) y avanzan por RANURAS, no de ocho en ocho: un agregado de
        // 12 bytes ocupa dos y corre el que viene detrás.
        //
        // Era `16 + i*8` fijo. Mientras todo cupo en un registro daba lo mismo;
        // el día que entró un struct por valor, el segundo parámetro empezaba a
        // leerse desde la mitad del primero.
        let mut off = 16i32;
        for p in params.iter() {
            self.var_offsets.insert(p.name.clone(), (off, p.typ.clone()));
            let bytes = self.type_stack_size(&p.typ);
            off += agregados::ranuras(bytes) as i32 * 8;
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

    /// Guarda `rax` en `[rbp+disp]` con el tamaño EXACTO de `tipo`.
    ///
    /// La pareja de `emit_store_var`, pero por offset en vez de por nombre: una
    /// lista de inicialización escribe **dentro** de una variable, no sobre
    /// ella. Escribir siempre 8 bytes pisaría el campo siguiente — es el mismo
    /// bug que ya se pagó con `pt.x = 10` cuando `x` era `int`.
    fn emit_store_rbp(&mut self, disp: i32, tipo: &TypeSpec) {
        let corto = (-128..=127).contains(&disp);
        let modrm = if corto { 0x45 } else { 0x85 };
        let opcode: &[u8] = match tipo {
            TypeSpec::Char | TypeSpec::UnsignedChar => &[0x88],
            TypeSpec::Short | TypeSpec::UnsignedShort => &[0x66, 0x89],
            TypeSpec::Int | TypeSpec::UnsignedInt | TypeSpec::Float => &[0x89],
            _ => &[0x48, 0x89],
        };
        self.code.extend_from_slice(opcode);
        self.code.push(modrm);
        if corto {
            self.code.push(disp as u8);
        } else {
            self.code.extend_from_slice(&disp.to_le_bytes());
        }
    }

    /// Pone a cero `bytes` bytes a partir de `[rbp+base]`.
    ///
    /// De ocho en ocho mientras quepa, y el resto byte a byte. Sin memset:
    /// aquí no hay libc, y para los tamaños de un struct local un bucle
    /// desenrollado es más corto que la llamada que no existe.
    fn emit_cero_local(&mut self, base: i32, bytes: u32) {
        if bytes == 0 {
            return;
        }
        self.emit_xor_eax();
        let mut hecho = 0u32;
        while bytes - hecho >= 8 {
            self.emit_store_rbp(base + hecho as i32, &TypeSpec::Long);
            hecho += 8;
        }
        while hecho < bytes {
            self.emit_store_rbp(base + hecho as i32, &TypeSpec::Char);
            hecho += 1;
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
            // Un `int` con signo debe EXTENDER EL SIGNO al leerse: el resto
            // del codegen trabaja en 64 bits. Antes usaba `mov eax, [..]`,
            // que rellena de ceros, así que un `int y = -7;` se releía como
            // 4294967289. Los tipos más chicos ya lo hacían bien (movsx);
            // solo `int` se había quedado sin su versión con signo.
            TypeSpec::Int => {
                if disp >= -128 && disp <= 127 {
                    self.code.extend_from_slice(&[0x48, 0x63, 0x45, disp as u8]); // movsxd
                } else {
                    self.code.extend_from_slice(&[0x48, 0x63, 0x85]);
                    self.code.extend_from_slice(&(disp as i32).to_le_bytes());
                }
            }
            TypeSpec::UnsignedInt => {
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
            // ★ Un nombre que no es variable, ni global, ni constante de enum,
            // ni función, NO VALE CERO: no existe.
            //
            // Esto era un `xor eax,eax` mudo, y es lo que escondió que
            // `#include` tiraba los `#define` de la cabecera: `BMO_TECLA_REPAG`
            // y `BMO_TECLA_AVPAG` llegaban sin expandir, el codegen los ponía a
            // cero **a los dos**, y `if (t == REPAG)` era cierto para AvPag.
            // Comparaba cero contra cero y el programa parecía correcto.
            //
            // Un cero inventado es la peor respuesta posible a "no sé qué es
            // esto": es un valor legítimo en cualquier expresión, así que el
            // error viaja hasta donde ya no se puede rastrear.
            self.errors.push(format!(
                "'{name}' no esta declarado (ni variable, ni global, ni constante de enum, \
                 ni funcion). Si venia de un #define, la cabecera no llego a expandirse."
            ));
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
        // Lo que `__va_arg` necesita saber, y sólo se sabe aquí: si esta
        // función admite variádicos y dónde acaban los que tienen nombre.
        self.es_variadica = func.variadica;
        self.ranuras_con_nombre = func
            .params
            .iter()
            .map(|p| agregados::ranuras(self.type_stack_size(&p.typ)) as i32)
            .sum();
        // prologue
        self.code.extend_from_slice(&[0x55, 0x48, 0x89, 0xE5]); // push rbp; mov rbp, rsp
        // Copiar los parámetros a su ranura local. Hoy es un no-op —
        // `build_var_map` los deja donde ya están— y se conserva por si algún
        // día un parámetro necesita hueco propio. El offset se recalcula igual
        // que allí: por ranuras, no `i*8`.
        let param_count = func.params.len();
        let mut src_off = 16i32;
        for p in func.params.iter() {
            let avance = agregados::ranuras(self.type_stack_size(&p.typ)) as i32 * 8;
            // Un agregado no se copia con un `mov` de 8 bytes: ya está en su
            // sitio, y "copiarlo" así se llevaría sólo su primera palabra.
            if self.es_agregado(&p.typ) {
                src_off += avance;
                continue;
            }
            if src_off >= -128 && src_off <= 127 {
                self.code.extend_from_slice(&[0x48, 0x8B, 0x45, src_off as u8]);
            } else {
                self.code.extend_from_slice(&[0x48, 0x8B, 0x85]);
                self.code.extend_from_slice(&(src_off as i32).to_le_bytes());
            }
            // A su ranura local, si es otra.
            if let Some(&(local_off, _)) = self.var_offsets.get(&p.name) {
                if local_off != src_off {
                    if (-128..=127).contains(&local_off) {
                        self.code.extend_from_slice(&[0x48, 0x89, 0x45, local_off as u8]);
                    } else {
                        self.code.extend_from_slice(&[0x48, 0x89, 0x85]);
                        self.code.extend_from_slice(&local_off.to_le_bytes());
                    }
                }
            }
            src_off += avance;
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
            // Volver de `main` termina el proceso. Antes esto emitía
            // `mov eax,0x181; syscall`: otro número plano que el kernel no
            // despacha — el syscall retornaba error y la ejecución seguía
            // de largo hacia lo que hubiera después del código de main.
            //
            // NOTA: el valor de retorno de `main` se descarta. `TASK_OP_EXIT`
            // no acepta código de salida hoy (el kernel hace revoke + reap);
            // cuando lo acepte, se pasa `rax` como argumento aquí.
            bmo_lower::task::exit(&mut self.code);
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
            // `switch`: el valor se guarda en un hueco de pila y cada
            // comparación lo relee de ahí.
            //
            // El despacho anterior hacía DOS `pop` habiendo empujado una
            // sola vez, así que comparaba contra un valor de la pila que no
            // era suyo: siempre entraba por el primer `case`. Y el
            // `default:` era inalcanzable — su etiqueta se fijaba DESPUÉS
            // de todos los cuerpos, o sea al final, saltándose su propio
            // código.
            Stmt::Switch(expr, cases) => {
                self.emit_expr(expr);
                let end = self.fresh_label();
                self.break_target.push(end);

                self.code.push(0x50); // push rax → el valor vive en [rsp]

                let case_labels: Vec<u32> = cases.iter().map(|_| self.fresh_label()).collect();
                for (i, c) in cases.iter().enumerate() {
                    if let Some(val) = c.value {
                        self.code.extend_from_slice(&[0x48, 0xBA]); // mov rdx, imm64
                        self.code.extend_from_slice(&val.to_le_bytes());
                        self.code.extend_from_slice(&[0x48, 0x8B, 0x04, 0x24]); // mov rax, [rsp]
                        self.code.extend_from_slice(&[0x48, 0x39, 0xD0]); // cmp rax, rdx
                        self.emit_jz_reloc(case_labels[i]);
                    }
                }
                // Sin coincidencia: al `default:` si existe, si no al final.
                match cases.iter().position(|c| c.value.is_none()) {
                    Some(i) => self.emit_jmp_reloc(case_labels[i]),
                    None => self.emit_jmp_reloc(end),
                }

                for (i, c) in cases.iter().enumerate() {
                    self.resolve_label(case_labels[i]);
                    for s in &c.stmts { self.emit_stmt(s); }
                }

                // `end` va ANTES de liberar el hueco para que un `break`
                // dentro de un caso también lo libere.
                self.resolve_label(end);
                self.code.extend_from_slice(&[0x48, 0x83, 0xC4, 0x08]); // add rsp, 8
                self.break_target.pop();
            }
            Stmt::Break => {
                if let Some(lbl) = self.break_target.last() { self.emit_jmp_reloc(*lbl); }
            }
            Stmt::Continue => {
                if let Some(lbl) = self.continue_target.last() { self.emit_jmp_reloc(*lbl); }
            }
            Stmt::Return(Some(e)) => {
                // return de un float: el valor vive en xmm0 (ABI de retorno SSE);
                // el epílogo preserva xmm0. En contexto entero, emit_expr trunca.
                if self.expr_is_float(e) && !self.is_entry_function {
                    self.emit_fexpr(e);
                } else {
                    self.emit_expr(e);
                }
                self.emit_epilogue();
            }
            Stmt::Return(None) => {
                self.emit_epilogue();
            }
            Stmt::DeclAssign(typ, name, init) => {
                // Variable float/double: valor por la ruta SSE, store movsd/movss.
                if Self::is_float_ty(typ) {
                    match init {
                        Some(e) => self.emit_fexpr_operand(e), // acepta double d = 5;
                        None => self.code.extend_from_slice(&[0x66, 0x0F, 0x57, 0xC0]), // xorpd xmm0,xmm0 = 0.0
                    }
                    self.store_float_var(name);
                } else {
                    if let Some(e) = init { self.emit_expr(e); } else { self.emit_expr(&Expr::Int(0)); }
                    self.emit_store_var(name);
                }
            }
            // `T x = { … }` — la lista ya viene APLANADA a escrituras por
            // `parser/inicializador.rs`. Aquí no se sabe qué es un designador:
            // sólo "en el byte N va este valor, de este tamaño".
            Stmt::DeclInit(typ, name, escrituras) => {
                let Some(&(base, _)) = self.var_offsets.get(name) else {
                    self.errors.push(format!("'{name}' no tiene hueco en la pila"));
                    return;
                };
                // ★ C99 §6.7.9/21: lo NO mencionado vale cero. Se borra el
                // objeto entero ANTES de escribir, y por eso `{.y = 2}` deja la
                // `x` en 0 en vez de en lo que hubiera en la pila — que sería
                // basura distinta en cada llamada y un bug imposible de repetir.
                let bytes = self.type_stack_size(typ);
                self.emit_cero_local(base, bytes);
                for e in escrituras {
                    self.emit_expr(&e.valor);
                    self.emit_store_rbp(base + e.offset as i32, &e.tipo);
                }
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

    /// `printf(fmt, args…)` — la L2 de C sobre la librería de formateo.
    ///
    /// Antes esto empujaba los argumentos a la pila y llamaba a un
    /// `bmo_printf` **importado de `userland_ring3`**: un símbolo que en BMO
    /// nadie resuelve, porque no hay enlazado dinámico de una libc. El
    /// programa compilaba y luego saltaba a una dirección sin parchear.
    ///
    /// Ahora el formateo se emite EN LÍNEA: cada trozo literal baja por la
    /// puerta de consola y cada conversión evalúa su argumento y llama al
    /// emisor correspondiente de `bmo_lower::fmt`. Sin runtime, sin
    /// importaciones, sin dependencias del cargador.
    ///
    /// Lo específico de C —qué significa `%d`, que `%x` va en minúsculas,
    /// que `%%` es un porcentaje— se decide aquí. La librería solo sabe
    /// convertir un número en dígitos.
    /// **La superficie de biblioteca que se emite en línea.**
    ///
    /// Devuelve `Some(())` si `name` era una de ellas y ya se emitió.
    ///
    /// ★ Cada una carga sus argumentos en registros y llama al emisor de L1.
    /// El orden importa: se evalúa el último primero y se apila, porque
    /// evaluar el segundo argumento puede machacar el registro donde estaba el
    /// primero — un `memcpy(a, f(x), n)` con `f` llamando a otra cosa es el
    /// caso que lo destapa, y no se destapa en las pruebas fáciles.
    fn emitir_biblioteca(&mut self, name: &str, args: &[Expr]) -> Option<()> {
        use bmo_lower::memoria;
        use bmo_lower::x86;
        match (name, args.len()) {
            ("memcpy", 3) | ("memmove", 3) => {
                self.cargar_tres(args, x86::RDI, x86::RSI, x86::RCX);
                memoria::copiar(&mut self.code);
                // `memcpy` devuelve el destino, que sigue en la pila porque el
                // bucle se llevó rdi por delante.
                self.soltar_tres();
                Some(())
            }
            ("memset", 3) => {
                self.cargar_tres(args, x86::RDI, x86::RAX, x86::RCX);
                memoria::rellenar(&mut self.code);
                self.soltar_tres();
                Some(())
            }
            ("strlen", 1) => {
                self.emit_expr(&args[0]);
                self.code.extend_from_slice(&[0x48, 0x89, 0xC7]); // mov rdi, rax
                memoria::largo(&mut self.code);
                Some(())
            }
            ("strcmp", 2) => {
                self.emit_expr(&args[1]);
                self.code.push(0x50); // push
                self.emit_expr(&args[0]);
                self.code.extend_from_slice(&[0x48, 0x89, 0xC7]); // mov rdi, rax
                self.code.extend_from_slice(&[0x5E]);             // pop rsi
                memoria::comparar(&mut self.code);
                Some(())
            }
            ("strcpy", 2) => {
                // `strcpy` es `copiar` con la medida sacada del origen: se mide
                // primero y se copia el terminador también (de ahí el +1).
                self.emit_expr(&args[1]);
                self.code.push(0x50);                             // push src
                self.emit_expr(&args[0]);
                self.code.push(0x50);                             // push dst
                self.code.extend_from_slice(&[0x48, 0x8B, 0x7C, 0x24, 0x08]); // mov rdi,[rsp+8]
                memoria::largo(&mut self.code);                   // rax = largo(src)
                self.code.extend_from_slice(&[0x48, 0xFF, 0xC0]); // inc rax (el cero)
                self.code.extend_from_slice(&[0x48, 0x89, 0xC1]); // mov rcx, rax
                self.code.extend_from_slice(&[0x5F]);             // pop rdi (dst)
                self.code.extend_from_slice(&[0x5E]);             // pop rsi (src)
                self.code.push(0x57);                             // push rdi (para devolverlo)
                memoria::copiar(&mut self.code);
                self.code.extend_from_slice(&[0x58]);             // pop rax
                Some(())
            }
            ("abs", 1) => {
                self.emit_expr(&args[0]);
                memoria::absoluto(&mut self.code);
                Some(())
            }
            _ => None,
        }
    }

    /// Tres argumentos a tres registros, evaluando de derecha a izquierda.
    ///
    /// ★ Los tres se dejan EN LA PILA y los registros se cargan **leyendo**,
    /// no sacando. La primera versión los sacaba con `pop` y apilaba el
    /// destino dos veces para poder devolverlo — y eso desalineaba los tres
    /// `pop`: `memset` acababa con el valor de relleno en el registro del
    /// contador. Salió como `-16,-16,-16` donde tenía que salir `65,65,65`.
    ///
    /// Leyendo con desplazamiento no hay orden que cuadrar: cada argumento
    /// está donde se puso. Y quien llama limpia con [`Self::soltar_tres`], que
    /// es lo que faltaba también — la versión de `pop` dejaba dos valores
    /// vivos en la pila por cada `memcpy`, y eso no se ve hasta que un bucle
    /// hace mil.
    fn cargar_tres(&mut self, args: &[Expr], r0: u8, r1: u8, r2: u8) {
        self.emit_expr(&args[2]);
        self.code.push(0x50); // push n        -> [rsp+16]
        self.emit_expr(&args[1]);
        self.code.push(0x50); // push src      -> [rsp+8]
        self.emit_expr(&args[0]);
        self.code.push(0x50); // push dst      -> [rsp]
        self.mov_desde_pila(r0, 0);
        self.mov_desde_pila(r1, 8);
        self.mov_desde_pila(r2, 16);
    }

    /// Recupera el destino en `rax` y tira los otros dos. Cierra a
    /// [`Self::cargar_tres`].
    fn soltar_tres(&mut self) {
        self.code.push(0x58);                               // pop rax (dst)
        self.code.extend_from_slice(&[0x48, 0x83, 0xC4, 16]); // add rsp, 16
    }

    /// `mov <r64>, [rsp+disp8]`.
    fn mov_desde_pila(&mut self, reg: u8, disp: u8) {
        self.code.push(0x48 | if reg >= 8 { 0x04 } else { 0 }); // REX.W (+R)
        self.code.push(0x8B);
        self.code.push(0x44 | ((reg & 7) << 3)); // modrm: [SIB + disp8]
        self.code.push(0x24);                    // SIB: base = rsp
        self.code.push(disp);
    }

    fn emit_printf_variadic(&mut self, args: &[Expr]) {
        let Expr::StringLit(format) = &args[0] else {
            self.errors.push(
                "printf con formato calculado en tiempo de ejecucion no se compila: \
                 el formato debe ser un literal para poder emitirlo en linea"
                    .to_string(),
            );
            return;
        };
        let format = format.clone();
        let va_args: Vec<Expr> = args[1..].to_vec();
        let mut next_arg = 0usize;
        let mut literal: Vec<u8> = Vec::new();

        let chars: Vec<char> = format.chars().collect();
        let mut i = 0usize;
        while i < chars.len() {
            if chars[i] != '%' {
                let mut buf = [0u8; 4];
                literal.extend_from_slice(chars[i].encode_utf8(&mut buf).as_bytes());
                i += 1;
                continue;
            }

            // Saltar los modificadores de longitud: en BMO todo entero viaja
            // en 64 bits, así que `%ld` y `%d` producen lo mismo.
            let mut j = i + 1;
            while j < chars.len() && matches!(chars[j], 'l' | 'h' | 'z' | 'j' | 't') {
                j += 1;
            }
            let Some(&conversion) = chars.get(j) else {
                self.errors
                    .push("'%' al final del formato de printf".to_string());
                return;
            };

            if conversion == '%' {
                literal.push(b'%');
                i = j + 1;
                continue;
            }

            // Todo lo literal acumulado sale ANTES de la conversión.
            if !literal.is_empty() {
                bmo_lower::console::write_const(&mut self.code, &literal);
                literal.clear();
            }

            let Some(arg) = va_args.get(next_arg).cloned() else {
                self.errors.push(format!(
                    "printf: '%{conversion}' no tiene argumento correspondiente"
                ));
                return;
            };
            next_arg += 1;
            self.emit_expr(&arg); // el valor queda en rax

            match conversion {
                'd' | 'i' => bmo_lower::fmt::write_i64(&mut self.code),
                'u' => bmo_lower::fmt::write_u64_radix(&mut self.code, 10),
                'x' => bmo_lower::fmt::write_u64_radix(&mut self.code, 16),
                'c' => bmo_lower::fmt::write_char(&mut self.code),
                's' => bmo_lower::fmt::write_cstr(&mut self.code),
                other => {
                    self.errors.push(format!(
                        "printf: '%{other}' aun no se compila (se compilan \
                         %d %i %u %x %c %s %%; los flotantes necesitan la ruta SSE)"
                    ));
                    return;
                }
            }
            i = j + 1;
        }

        if !literal.is_empty() {
            bmo_lower::console::write_const(&mut self.code, &literal);
        }

        if next_arg < va_args.len() {
            self.errors.push(format!(
                "printf: sobran {} argumento(s) para el formato dado",
                va_args.len() - next_arg
            ));
        }
    }
    /// `printf("literal")` — la L2 de C sobre la puerta genérica (L1).
    ///
    /// Lo específico de C que se resuelve AQUÍ y en ningún otro sitio: que la
    /// cadena es un literal ya escapado por el lexer y que `\n` va pegado al
    /// final. Los bytes resultantes se los entrega a `bmo_lower::console`,
    /// que no sabe que existe C.
    ///
    /// Antes esto emitía `lea rdi,[str]; mov esi,len; syscall 0x1F0`: un
    /// número plano que el kernel no despacha, pasando además un PUNTERO,
    /// que la superficie congelada rechaza por diseño. No imprimía nada en
    /// hardware. La cadena ya no necesita vivir en `.rodata`: viaja como
    /// inmediatos dentro de las propias instrucciones.
    fn emit_printf(&mut self, s: &str, newline: bool) {
        let text = if newline { let mut t = s.to_string(); t.push('\n'); t } else { s.to_string() };
        bmo_lower::console::write_const(&mut self.code, text.as_bytes());
    }

    // ---- Expression emit ----
    fn emit_expr(&mut self, expr: &Expr) {
        // Guard SSE: una expresión FLOTANTE que llega a la ruta entera está en
        // contexto entero (int x = 1.5; return d;) → calcular en xmm y truncar
        // a rax (cvttsd2si). Las comparaciones dan int 0/1 (no son float) y se
        // manejan abajo. emit_fexpr_operand solo llama aquí para NO-floats, así
        // que no hay recursión infinita.
        if self.expr_is_float(expr) {
            self.emit_fexpr(expr);
            self.code.extend_from_slice(&[0xF2, 0x48, 0x0F, 0x2C, 0xC0]); // cvttsd2si rax, xmm0
            return;
        }
        match expr {
            Expr::Int(n) => {
                let v = *n as u64;
                self.emit_asm(|a| { a.mov_imm64(bmo_sem_asm::x86_64::Reg::Rax, v).unwrap(); });
            }
            // El guard SSE de arriba ya captura los floats; este brazo solo
            // existe por exhaustividad (defensivo: trunca a entero).
            Expr::FloatLit(_) => {
                self.emit_fexpr(expr);
                self.code.extend_from_slice(&[0xF2, 0x48, 0x0F, 0x2C, 0xC0]); // cvttsd2si rax, xmm0
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
                // ★ Las funciones de biblioteca que se emiten EN LÍNEA.
                //
                // No hay librería que enlazar, y no es una carencia: es el
                // modelo. Un `.bex` es una imagen entera y BEF no resuelve
                // relocaciones contra un `.so`. Emitir el bucle cuesta treinta
                // bytes y ahorra un enlazador, un formato de librería y un
                // cargador dinámico.
                //
                // Lo que se emite vive en `bmo_lower::memoria` (L1) porque
                // "mueve estos bytes" no tiene semántica de lenguaje: COBOL
                // mueve grupos y Ada asigna arrays con la misma emisión. Aquí
                // sólo se pone el nombre que usa C.
                if let Some(n) = self.emitir_biblioteca(name, args) {
                    let _ = n;
                    return;
                }
                // Special case: printf → emit bmo_printf from userland_ring3
                if name == "printf" && !args.is_empty() {
                    self.emit_printf_variadic(args);
                    return;
                }
                // La pareja de `printf`: se emiten EN LINEA por lo mismo — aqui
                // no hay libc que enlazar ni simbolo que nadie resuelva.
                if name == "getchar" && args.is_empty() {
                    self.emit_getchar();
                    return;
                }
                if name == "scanf" && !args.is_empty() {
                    self.emit_scanf(args);
                    return;
                }
                // ¿Llamada INDIRECTA? El nombre no es una función pero SÍ una
                // variable → contiene una dirección (puntero a función).
                let is_indirect = !self.known_functions.contains(name)
                    && (self.var_offsets.contains_key(name) || self.global_offsets.contains_key(name));

                // ── Los argumentos, de derecha a izquierda ──
                //
                // Cuántas ranuras ocupa cada uno lo dice el PARÁMETRO, no la
                // expresión: un `struct` de 12 bytes ocupa dos aunque quien lo
                // pase sea una variable. Si no hay firma —llamada indirecta por
                // puntero— se supone una ranura, que es lo que era antes.
                let tipos_param: Vec<TypeSpec> = self
                    .firmas
                    .get(name)
                    .map(|(p, _)| p.clone())
                    .unwrap_or_default();
                let mut ranuras_total = 0u32;
                for (i, arg) in args.iter().enumerate().rev() {
                    match tipos_param.get(i) {
                        Some(t) if self.es_agregado(t) => {
                            let bytes = self.type_stack_size(t);
                            ranuras_total += agregados::ranuras(bytes);
                            self.emit_empuja_agregado(arg, bytes);
                        }
                        _ => {
                            ranuras_total += 1;
                            self.emit_expr(arg);
                            self.code.push(0x50); // push rax
                        }
                    }
                }
                // Devolver un agregado es un tercer mecanismo (puntero oculto)
                // y todavía no está. Se dice: devolver ocho bytes de un struct
                // de doce sería la clase de mentira que este compilador no
                // cuenta.
                if let Some((_, ret)) = self.firmas.get(name) {
                    if self.es_agregado(&ret.clone()) {
                        self.errors.push(format!(
                            "'{name}' devuelve un struct por valor, y eso aun no se compila \
                             (pasa un puntero al destino como parametro)"
                        ));
                    }
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
                // Se quita de la pila lo que se PUSO, que ya no es una ranura
                // por argumento.
                let n = ranuras_total * 8;
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
                // `p = q` con `p` agregado: se copian sus BYTES, todos.
                //
                // Antes caía al camino normal —`mov rax,[q]` + `mov [p],rax`—
                // que se lleva ocho y deja el resto con lo que hubiera. Un
                // struct de 12 se copiaba a medias, en silencio.
                if let Some(t) = self.var_type_of(name) {
                    if self.es_agregado(&t) {
                        let bytes = self.type_stack_size(&t);
                        let destino = Expr::Var(name.clone());
                        self.emit_asigna_agregado(&destino, val, bytes);
                        return;
                    }
                }
                // Asignación a variable float/double → ruta SSE.
                if self.var_type_of(name).map_or(false, |t| Self::is_float_ty(&t)) {
                    self.emit_fexpr_operand(val);
                    self.store_float_var(name);
                } else {
                    self.emit_expr(val);
                    self.emit_store_var(name);
                }
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
            // `*p` debe leer el TAMAÑO DEL APUNTADO, no siempre 8 bytes.
            // Antes `*(p+1)` con `int *p` leía 8 bytes desde la posición
            // correcta, o sea dos enteros pegados: devolvía 504403158366158848
            // en vez de 6.
            Expr::Deref(a) => {
                self.emit_expr(a); // rax = dirección
                match self.pointee_type(a) {
                    Some(TypeSpec::Char) => self.code.extend_from_slice(&[0x48, 0x0F, 0xBE, 0x00]),
                    Some(TypeSpec::UnsignedChar) => self.code.extend_from_slice(&[0x48, 0x0F, 0xB6, 0x00]),
                    Some(TypeSpec::Short) => self.code.extend_from_slice(&[0x48, 0x0F, 0xBF, 0x00]),
                    Some(TypeSpec::UnsignedShort) => self.code.extend_from_slice(&[0x48, 0x0F, 0xB7, 0x00]),
                    Some(TypeSpec::Int) => self.code.extend_from_slice(&[0x48, 0x63, 0x00]),
                    Some(TypeSpec::UnsignedInt) => self.code.extend_from_slice(&[0x8B, 0x00]),
                    _ => self.code.extend_from_slice(&[0x48, 0x8B, 0x00]),
                }
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
            Expr::IndexPtr(base, index, elem) => {
                // p->arr[i]: dirección = base(puntero) + i*sizeof(elem), luego load
                self.emit_index_ptr_addr(base, index, elem);
                self.emit_load_elem(&elem.clone());
            }
            Expr::AssignIndexPtr(base, index, elem, val) => {
                self.emit_expr(val);          // rax = valor
                self.code.push(0x50);         // push valor
                self.emit_index_ptr_addr(base, index, elem); // rax = dirección
                self.code.push(0x5A);         // pop rdx = valor
                self.emit_store_elem(&elem.clone());
                self.code.extend_from_slice(&[0x48, 0x89, 0xD0]);
            }
            Expr::CallPtr(callee, args) => {
                // (*fp)(args): args a la pila, callee da la dirección, call rax
                for arg in args.iter().rev() {
                    self.emit_expr(arg);
                    self.code.push(0x50);
                }
                self.emit_expr(callee);                     // rax = dirección de la función
                self.code.extend_from_slice(&[0xFF, 0xD0]); // call rax
                let n = args.len() as u32 * 8;
                if n > 0 {
                    if n <= 127 { self.code.extend_from_slice(&[0x48, 0x83, 0xC4, n as u8]); }
                    else { self.code.extend_from_slice(&[0x48, 0x81, 0xC4]); self.code.extend_from_slice(&n.to_le_bytes()); }
                }
            }
            // Tras `emit_binop`: rdx = operando IZQUIERDO, rax = DERECHO.
            // Los operadores conmutativos daban igual; los que no lo son
            // estaban invertidos y nadie lo vio hasta ejecutarlos.
            // `p + n` con `p` puntero avanza n ELEMENTOS, no n bytes. Antes
            // sumaba bytes: con `int *p`, `*(p+1)` leía desde el byte 1 en
            // vez del 4, o sea a caballo entre dos enteros.
            Expr::Add(a, b) => {
                if let Some(scale) = self.pointer_scale(a) {
                    let scaled = Expr::Mul(b.clone(), Box::new(Expr::Int(scale as i64)));
                    self.emit_binop(a, &scaled, &[0x48, 0x01, 0xD0]);
                } else if let Some(scale) = self.pointer_scale(b) {
                    let scaled = Expr::Mul(a.clone(), Box::new(Expr::Int(scale as i64)));
                    self.emit_binop(&scaled, b, &[0x48, 0x01, 0xD0]);
                } else {
                    self.emit_binop(a, b, &[0x48, 0x01, 0xD0]);
                }
            }
            // `a - b`. Antes: `sub rax, rdx` = b - a, o sea al reves.
            // `10 - 3` daba -7.
            Expr::Sub(a, b) => {
                const SUB: &[u8] = &[
                    0x48, 0x29, 0xC2, // sub rdx, rax   → rdx = a - b
                    0x48, 0x89, 0xD0, // mov rax, rdx
                ];
                // `p - n` retrocede n ELEMENTOS (la resta puntero-puntero,
                // que daria un indice, no se deduce aqui).
                match self.pointer_scale(a) {
                    Some(scale) if self.pointer_scale(b).is_none() => {
                        let scaled = Expr::Mul(b.clone(), Box::new(Expr::Int(scale as i64)));
                        self.emit_binop(a, &scaled, SUB);
                    }
                    _ => self.emit_binop(a, b, SUB),
                }
            }
            Expr::Mul(a, b) => self.emit_binop(a, b, &[0x48, 0x0F, 0xAF, 0xC2]),
            // `a / b` CON SIGNO. Antes hacia dos `pop` habiendo empujado una
            // sola vez —se llevaba un valor de la pila que no era suyo— y
            // ademas dividia sin signo. `10 / 3` daba 0.
            Expr::Div(a, b) => self.emit_binop(a, b, &[
                0x48, 0x89, 0xC1, // mov rcx, rax   → divisor = b
                0x48, 0x89, 0xD0, // mov rax, rdx   → dividendo = a
                0x48, 0x99,       // cqo            → extiende el signo
                0x48, 0xF7, 0xF9, // idiv rcx
            ]),
            // `a % b`: el resto queda en rdx.
            Expr::Mod(a, b) => self.emit_binop(a, b, &[
                0x48, 0x89, 0xC1, // mov rcx, rax
                0x48, 0x89, 0xD0, // mov rax, rdx
                0x48, 0x99,       // cqo
                0x48, 0xF7, 0xF9, // idiv rcx
                0x48, 0x89, 0xD0, // mov rax, rdx  → el resto
            ]),
            // Comparaciones: si algún operando es float → comisd (setcc unsigned);
            // si no, la comparación entera de siempre.
            // Comparaciones enteras: todas comparan `a` contra `b` en ese
            // orden y usan el setcc que les toca. Antes `<`, `>` y `>=`
            // comparaban al reves —`1 < 2` daba 0— porque la comparacion se
            // hacia sobre `b - a` con el setcc de la forma directa.
            Expr::Eq(a, b) => if self.expr_is_float(a) || self.expr_is_float(b) { self.emit_fcmp(a, b, 0x94) } else { self.emit_cmp(a, b, 0x94) },
            Expr::Neq(a, b) => if self.expr_is_float(a) || self.expr_is_float(b) { self.emit_fcmp(a, b, 0x95) } else { self.emit_cmp(a, b, 0x95) },
            Expr::Lt(a, b) => if self.expr_is_float(a) || self.expr_is_float(b) { self.emit_fcmp(a, b, 0x92) } else { self.emit_cmp(a, b, 0x9C) },
            Expr::Gt(a, b) => if self.expr_is_float(a) || self.expr_is_float(b) { self.emit_fcmp(a, b, 0x97) } else { self.emit_cmp(a, b, 0x9F) },
            Expr::Le(a, b) => if self.expr_is_float(a) || self.expr_is_float(b) { self.emit_fcmp(a, b, 0x96) } else { self.emit_cmp(a, b, 0x9E) },
            Expr::Ge(a, b) => if self.expr_is_float(a) || self.expr_is_float(b) { self.emit_fcmp(a, b, 0x93) } else { self.emit_cmp(a, b, 0x9D) },
            Expr::BitAnd(a, b) => self.emit_binop(a, b, &[0x48, 0x21, 0xD0]),
            Expr::BitXor(a, b) => self.emit_binop(a, b, &[0x48, 0x31, 0xD0]),
            Expr::BitOr(a, b) => self.emit_binop(a, b, &[0x48, 0x09, 0xD0]),
            // `a << b` / `a >> b`. Antes desplazaban el operando DERECHO por
            // el izquierdo: `1 << 3` intentaba `3 << 1`.
            //
            // El desplazamiento a la derecha es ARITMETICO (`sar`), que es
            // lo correcto para `int`. Un tipo sin signo querria `shr`; hoy
            // el codegen no arrastra esa distincion hasta aqui.
            Expr::Shl(a, b) => self.emit_binop(a, b, &[
                0x48, 0x89, 0xC1, // mov rcx, rax   → cuenta = b
                0x48, 0x89, 0xD0, // mov rax, rdx   → valor  = a
                0x48, 0xD3, 0xE0, // shl rax, cl
            ]),
            Expr::Shr(a, b) => self.emit_binop(a, b, &[
                0x48, 0x89, 0xC1, // mov rcx, rax
                0x48, 0x89, 0xD0, // mov rax, rdx
                0x48, 0xD3, 0xF8, // sar rax, cl
            ]),
            // `&&` y `||` valen 0 o 1, no "el operando que quedó". Antes
            // `0 || 3` daba 3: cortocircuitaba bien pero devolvía el valor
            // crudo, y el estándar dice que el resultado es `int` 0/1.
            Expr::LAnd(a, b) => {
                let end = self.fresh_label();
                self.emit_expr(a);
                self.code.extend_from_slice(&[0x85, 0xC0]);
                self.emit_jz_reloc(end);
                self.emit_expr(b);
                self.resolve_label(end);
                self.emit_normalize_bool();
            }
            Expr::LOr(a, b) => {
                let end = self.fresh_label();
                self.emit_expr(a);
                self.code.extend_from_slice(&[0x85, 0xC0]);
                self.emit_jnz_reloc(end);
                self.emit_expr(b);
                self.resolve_label(end);
                self.emit_normalize_bool();
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

    /// rax = base_ptr + index * sizeof(elem), donde `base` es una EXPRESIÓN
    /// que produce un puntero (p->arr, a+1...). Deja la dirección en rax.
    fn emit_index_ptr_addr(&mut self, base: &Expr, index: &Expr, elem: &TypeSpec) {
        let size = self.type_stack_size(elem).max(1) as u8;
        self.emit_expr(base);          // rax = puntero base
        self.code.push(0x50);          // push base
        self.emit_expr(index);         // rax = índice
        self.emit_scale_index(size);   // rax = índice * size
        self.code.push(0x5A);          // pop rdx = base
        self.code.extend_from_slice(&[0x48, 0x01, 0xD0]); // add rax, rdx
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
        // ★ `__va_arg(i)` — el argumento variádico número `i`, contando desde 0
        // después de los que tienen nombre.
        //
        // No sale de la tabla de sem-asm porque no es una instrucción del CPU:
        // es aritmética sobre el marco de pila, y depende de CUÁNTOS parámetros
        // con nombre tiene la función que lo pregunta.
        //
        // Y es aritmética y no ABI porque BMO C pasa los argumentos **por la
        // pila**, de derecha a izquierda. En la convención de registros de
        // SysV esto obligaría a volcar seis registros en el prólogo y a llevar
        // dos cursores (registros y pila); aquí los argumentos ya están
        // seguidos en memoria y el número `i` es un desplazamiento. La
        // convención más vieja resultó ser la que hace los varargs triviales.
        //
        // El índice es de EJECUCIÓN, no una constante: sin eso no se puede
        // recorrer los argumentos en un bucle, que es justo lo que hace un
        // `vsprintf` — y un `vsprintf` es lo que pide `I_Error(fmt, ...)`.
        if name == "va_arg" {
            if args.len() != 1 {
                self.errors.push(
                    "__va_arg(i) espera UN argumento: el indice del variadico".into());
                return;
            }
            if !self.es_variadica {
                self.errors.push(
                    "__va_arg() en una funcion que no declara '...': no hay argumentos \
                     variadicos que leer".into());
                return;
            }
            self.emit_expr(&args[0]);                       // rax = i
            let base = 16 + self.ranuras_con_nombre * 8;    // primer variádico
            // lea rdx, [rbp + base]
            self.code.extend_from_slice(&[0x48, 0x8D, 0x95]);
            self.code.extend_from_slice(&base.to_le_bytes());
            // mov rax, [rdx + rax*8]
            self.code.extend_from_slice(&[0x48, 0x8B, 0x04, 0xC2]);
            return;
        }
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
    ///
    /// Los nombres de 64 bits (`rdi`, `rsi`, `r10`, `r8`) no estaban, y esa
    /// ausencia era justo la que dejaba `syscall` fuera del lenguaje: la
    /// convención de la puerta congelada pasa los argumentos por ahí, así que
    /// sin estos registros no había forma de escribir la llamada en C. Sólo
    /// existían los de los puertos de E/S (`dx`, `al`) y los de `rdmsr`.
    fn emit_pop_to_reg(&mut self, reg: &str) {
        match reg {
            "rax" | "eax" | "ax" | "al" => self.code.push(0x58),  // pop rax
            "rcx" | "ecx" | "cx" | "cl" => self.code.push(0x59),  // pop rcx
            "rdx" | "edx" | "dx"        => self.code.push(0x5A),  // pop rdx
            "rbx" => self.code.push(0x5B),
            "rsi" | "esi" | "si" => self.code.push(0x5E),
            "rdi" | "edi" | "di" => self.code.push(0x5F),
            // r8..r11 llevan REX.B: el `pop` corto sólo alcanza los ocho
            // registros clásicos.
            "r8"  => self.code.extend_from_slice(&[0x41, 0x58]),
            "r9"  => self.code.extend_from_slice(&[0x41, 0x59]),
            "r10" => self.code.extend_from_slice(&[0x41, 0x5A]),
            "r11" => self.code.extend_from_slice(&[0x41, 0x5B]),
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
            // La puerta devuelve DOS cosas: el código en rax y el valor en
            // rdx. Quien pide el valor se lleva rdx a rax, que es donde este
            // codegen espera todo resultado.
            Some("rdx") => self.code.extend_from_slice(&[0x48, 0x89, 0xD0]), // mov rax, rdx
            // "eax": escribir eax en modo 64-bit ya deja rax con el alto en cero
            _ => {}
        }
    }

    // ═══════════════ Ruta SSE: floats en xmm0 (doble precisión) ═══════════════
    // C tradicional oculta si un valor es float; aquí el codegen lo SABE y lo
    // rutea por xmm. Se computa todo en double; `float` (f32) se convierte en
    // los bordes (load/store). Registro de trabajo: xmm0; scratch: xmm1.

    fn var_type_of(&self, name: &str) -> Option<TypeSpec> {
        self.var_offsets.get(name).map(|&(_, ref t)| t.clone())
            .or_else(|| self.global_offsets.get(name).map(|&(_, ref t)| t.clone()))
    }

    fn is_float_ty(t: &TypeSpec) -> bool { matches!(t, TypeSpec::Float | TypeSpec::Double) }

    /// ¿Esta expresión produce un valor de punto flotante?
    fn expr_is_float(&self, e: &Expr) -> bool {
        match e {
            Expr::FloatLit(_) => true,
            Expr::Var(n) => self.var_type_of(n).map_or(false, |t| Self::is_float_ty(&t)),
            Expr::Cast(t, _) => Self::is_float_ty(t),
            Expr::Add(a, b) | Expr::Sub(a, b) | Expr::Mul(a, b) | Expr::Div(a, b) =>
                self.expr_is_float(a) || self.expr_is_float(b),
            Expr::Neg(a) => self.expr_is_float(a),
            Expr::Field(_, _, _, t) | Expr::Arrow(_, _, _, t) => Self::is_float_ty(t),
            Expr::IndexPtr(_, _, t) => Self::is_float_ty(t),
            Expr::Conditional(_, a, b) => self.expr_is_float(a) || self.expr_is_float(b),
            _ => false,
        }
    }

    /// cvtsi2sd xmm0, rax — entero (rax) → double (xmm0).
    fn emit_int_to_double(&mut self) {
        self.code.extend_from_slice(&[0xF2, 0x48, 0x0F, 0x2A, 0xC0]);
    }

    /// modrm+disp para `<sse> xmm0, [rbp+off]` / `[rbp+off], xmm0` (reg field = 0).
    fn emit_rbp_disp(&mut self, off: i32) {
        if off >= -128 && off <= 127 {
            self.code.push(0x45);           // mod=01, reg=0, rm=101 (rbp) + disp8
            self.code.push(off as u8);
        } else {
            self.code.push(0x85);           // mod=10 + disp32
            self.code.extend_from_slice(&off.to_le_bytes());
        }
    }

    /// Carga una variable float/double del stack a xmm0 (siempre como double).
    fn emit_load_float_var(&mut self, name: &str) {
        if let Some(&(off, ref typ)) = self.var_offsets.get(name) {
            let is_f32 = matches!(typ, TypeSpec::Float);
            let off = off;
            if is_f32 {
                self.code.extend_from_slice(&[0xF3, 0x0F, 0x10]); // movss xmm0,[rbp+off]
                self.emit_rbp_disp(off);
                self.code.extend_from_slice(&[0xF3, 0x0F, 0x5A, 0xC0]); // cvtss2sd xmm0,xmm0
            } else {
                self.code.extend_from_slice(&[0xF2, 0x0F, 0x10]); // movsd xmm0,[rbp+off]
                self.emit_rbp_disp(off);
            }
        } else {
            // global float: pendiente (locales primero) → xmm0 = 0
            self.code.extend_from_slice(&[0x66, 0x0F, 0x57, 0xC0]); // xorpd xmm0,xmm0
            self.errors.push(format!("variable float global '{name}' aun no soportada (usa locales)"));
        }
    }

    /// Guarda xmm0 (double) en una variable float/double del stack.
    fn store_float_var(&mut self, name: &str) {
        if let Some(&(off, ref typ)) = self.var_offsets.get(name) {
            let is_f32 = matches!(typ, TypeSpec::Float);
            let off = off;
            if is_f32 {
                self.code.extend_from_slice(&[0xF2, 0x0F, 0x5A, 0xC0]); // cvtsd2ss xmm0,xmm0
                self.code.extend_from_slice(&[0xF3, 0x0F, 0x11]);       // movss [rbp+off],xmm0
                self.emit_rbp_disp(off);
            } else {
                self.code.extend_from_slice(&[0xF2, 0x0F, 0x11]);       // movsd [rbp+off],xmm0
                self.emit_rbp_disp(off);
            }
        } else {
            self.errors.push(format!("variable float global '{name}' aun no soportada (usa locales)"));
        }
    }

    /// Evalúa `e` a xmm0 como double, convirtiendo enteros si hace falta.
    fn emit_fexpr_operand(&mut self, e: &Expr) {
        if self.expr_is_float(e) {
            self.emit_fexpr(e);
        } else {
            self.emit_expr(e);          // rax = valor entero
            self.emit_int_to_double();  // xmm0 = (double) rax
        }
    }

    /// a OP b en double: resultado en xmm0. `op` = bytes de `<opsd> xmm0,xmm1`.
    fn emit_fbinop(&mut self, a: &Expr, b: &Expr, op: &[u8]) {
        self.emit_fexpr_operand(a);
        self.code.extend_from_slice(&[0x48, 0x83, 0xEC, 0x08]);       // sub rsp,8
        self.code.extend_from_slice(&[0xF2, 0x0F, 0x11, 0x04, 0x24]); // movsd [rsp],xmm0  (spill a)
        self.emit_fexpr_operand(b);
        self.code.extend_from_slice(&[0xF2, 0x0F, 0x10, 0xC8]);       // movsd xmm1,xmm0  (xmm1=b)
        self.code.extend_from_slice(&[0xF2, 0x0F, 0x10, 0x04, 0x24]); // movsd xmm0,[rsp] (xmm0=a)
        self.code.extend_from_slice(&[0x48, 0x83, 0xC4, 0x08]);       // add rsp,8
        self.code.extend_from_slice(op);                             // op xmm0,xmm1
    }

    /// Evalúa una expresión FLOTANTE dejando el resultado (double) en xmm0.
    fn emit_fexpr(&mut self, e: &Expr) {
        match e {
            Expr::FloatLit(f) => {
                let bits = f.to_bits();
                self.code.extend_from_slice(&[0x48, 0xB8]);            // mov rax, imm64
                self.code.extend_from_slice(&bits.to_le_bytes());
                self.code.extend_from_slice(&[0x66, 0x48, 0x0F, 0x6E, 0xC0]); // movq xmm0, rax
            }
            Expr::Var(n) => self.emit_load_float_var(n),
            Expr::Cast(t, inner) if Self::is_float_ty(t) => {
                // (double)algo — si algo ya es float, no-op; si es entero, convierte
                self.emit_fexpr_operand(inner);
            }
            Expr::Neg(a) => {
                self.emit_fexpr(a);
                // xorpd xmm0, sign-bit → negación
                self.code.extend_from_slice(&[0x48, 0xB8]);
                self.code.extend_from_slice(&0x8000_0000_0000_0000u64.to_le_bytes());
                self.code.extend_from_slice(&[0x66, 0x48, 0x0F, 0x6E, 0xC8]); // movq xmm1, rax
                self.code.extend_from_slice(&[0x66, 0x0F, 0x57, 0xC1]);       // xorpd xmm0, xmm1
            }
            Expr::Add(a, b) => self.emit_fbinop(a, b, &[0xF2, 0x0F, 0x58, 0xC1]), // addsd
            Expr::Sub(a, b) => self.emit_fbinop(a, b, &[0xF2, 0x0F, 0x5C, 0xC1]), // subsd
            Expr::Mul(a, b) => self.emit_fbinop(a, b, &[0xF2, 0x0F, 0x59, 0xC1]), // mulsd
            Expr::Div(a, b) => self.emit_fbinop(a, b, &[0xF2, 0x0F, 0x5E, 0xC1]), // divsd
            // cualquier otra cosa: es entera → convertir a double
            _ => self.emit_fexpr_operand(e),
        }
    }

    /// Comparación de floats: a CMP b → 0/1 en rax. `setcc` es el opcode
    /// SETcc estilo UNSIGNED (comisd fija CF/ZF como comparación sin signo).
    fn emit_fcmp(&mut self, a: &Expr, b: &Expr, setcc: u8) {
        self.emit_fexpr_operand(a);
        self.code.extend_from_slice(&[0x48, 0x83, 0xEC, 0x08]);       // sub rsp,8
        self.code.extend_from_slice(&[0xF2, 0x0F, 0x11, 0x04, 0x24]); // movsd [rsp],xmm0
        self.emit_fexpr_operand(b);
        self.code.extend_from_slice(&[0xF2, 0x0F, 0x10, 0xC8]);       // movsd xmm1,xmm0 (b)
        self.code.extend_from_slice(&[0xF2, 0x0F, 0x10, 0x04, 0x24]); // movsd xmm0,[rsp] (a)
        self.code.extend_from_slice(&[0x48, 0x83, 0xC4, 0x08]);       // add rsp,8
        self.code.extend_from_slice(&[0x66, 0x0F, 0x2F, 0xC1]);       // comisd xmm0,xmm1
        self.code.extend_from_slice(&[0x0F, setcc, 0xC0]);            // setcc al
        self.code.extend_from_slice(&[0x0F, 0xB6, 0xC0]);            // movzx eax, al
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
            Expr::IndexPtr(base, index, elem) => {
                self.emit_index_ptr_addr(base, index, elem);
            }
            _ => self.emit_expr(expr),
        }
    }

    /// Tipo al que apunta una expresión de dirección, si se puede deducir.
    ///
    /// Cubre lo que aparece en la práctica: una variable puntero o array,
    /// aritmética de punteros (`p + 1`), y un cast explícito. Cuando no se
    /// puede deducir se devuelve `None` y el `deref` lee 8 bytes, que es el
    /// comportamiento anterior.
    fn pointee_type(&self, expr: &Expr) -> Option<TypeSpec> {
        match expr {
            Expr::Var(name) => match self.var_type_of(name) {
                Some(TypeSpec::Ptr(inner)) | Some(TypeSpec::Array(inner, _)) => Some(*inner),
                _ => None,
            },
            Expr::Cast(TypeSpec::Ptr(inner), _) => Some((**inner).clone()),
            Expr::Add(a, b) | Expr::Sub(a, b) => {
                self.pointee_type(a).or_else(|| self.pointee_type(b))
            }
            _ => None,
        }
    }

    /// Cuantos bytes avanza `+1` sobre esta expresion, si es un puntero.
    /// `None` cuando no lo es o cuando el elemento mide 1 byte (no hace
    /// falta escalar).
    fn pointer_scale(&self, expr: &Expr) -> Option<u32> {
        let size = self.pointee_type(expr)?.stack_size();
        if size > 1 { Some(size) } else { None }
    }

    /// Convierte `rax` en 0 o 1, que es lo que valen `&&`, `||` y `!`.
    fn emit_normalize_bool(&mut self) {
        self.code.extend_from_slice(&[0x48, 0x85, 0xC0]);       // test rax, rax
        self.code.extend_from_slice(&[0x0F, 0x95, 0xC0]);       // setne al
        self.code.extend_from_slice(&[0x48, 0x0F, 0xB6, 0xC0]); // movzx rax, al
    }

    fn emit_binop(&mut self, a: &Expr, b: &Expr, op: &[u8]) {
        self.emit_expr(a);
        self.code.push(0x50);
        self.emit_expr(b);
        self.code.push(0x5A);
        self.code.extend_from_slice(op);
    }

    /// Comparación entera `a <op> b` → 0 o 1 en `rax`.
    ///
    /// `setcc` es el segundo byte del opcode: `0x94`=sete, `0x95`=setne,
    /// `0x9C`=setl, `0x9D`=setge, `0x9E`=setle, `0x9F`=setg.
    ///
    /// El `movzx` del final NO es decorativo: `setcc` solo escribe `al`, así
    /// que sin él los 56 bits altos de `rax` conservan el valor del operando
    /// derecho. Con operandos chicos el resultado parecía correcto de puro
    /// milagro; `printf("%d", x == y)` con una `x` grande imprimía basura.
    fn emit_cmp(&mut self, a: &Expr, b: &Expr, setcc: u8) {
        self.emit_expr(a);
        self.code.push(0x50); // push rax (izquierdo)
        self.emit_expr(b); // rax = derecho
        self.code.push(0x5A); // pop rdx (izquierdo)
        self.code.extend_from_slice(&[0x48, 0x39, 0xC2]); // cmp rdx, rax → a - b
        self.code.extend_from_slice(&[0x0F, setcc, 0xC0]); // setcc al
        self.code.extend_from_slice(&[0x48, 0x0F, 0xB6, 0xC0]); // movzx rax, al
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
        code_sec.alignment = 4096;
        b.add_section(code_sec);

        if !rodata_bytes.is_empty() {
            let mut rodata_sec = BefSection::rodata(rodata_bytes.to_vec());
            rodata_sec.alignment = 4096;
            b.add_section(rodata_sec);
        }

        if !data_bytes.is_empty() {
            let mut data_sec = BefSection::data(data_bytes.to_vec());
            data_sec.alignment = 4096;
            b.add_section(data_sec);
        }

        b.entry_offset = self.entry_offset as u64;
        b.build().unwrap_or_default()
    }
}
