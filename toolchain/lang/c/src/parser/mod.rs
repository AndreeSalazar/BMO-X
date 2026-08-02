//! C Parser -- tokens a AST (gramatica completa) + preprocesador.

pub mod preprocessor;
/// Las listas `{ … }`, en su propio fichero. Ver su cabecera para el porqué del
/// reparto y para qué hicieron GCC, Clang, chibicc, TCC y MSVC con esto mismo.
mod inicializador;

use std::collections::HashMap;
use std::path::PathBuf;
use crate::ast::*;
use crate::lexer::Token;
use crate::module;
use crate::{CError, StandardFeatures};

/// Lo que puede haber en el nivel de fichero cuando algo empieza como una
/// función.
///
/// Eran dos casos (`Some`/`None`) y hacen falta **tres**: un prototipo no es
/// una función —no tiene cuerpo que emitir— pero tampoco es "esto no era una
/// función", porque sus tokens ya se han consumido. Devolver `None` ahí hacía
/// que el llamante rebobinara y lo intentara leer como una variable global,
/// y el error que salía acusaba de lo que no era.
enum Tope {
    Funcion(Function),
    /// `int f(int);` — declarada, no definida. Consumida.
    Prototipo,
    /// No era una función. Los tokens están rebobinados.
    NoEsFuncion,
}

pub(crate) struct Parser {
    tokens: Vec<Token>,
    token_lines: Vec<usize>,
    pos: usize,
    var_types: HashMap<String, TypeSpec>,
    struct_fields: HashMap<String, Vec<(String, u32, u32)>>,
    struct_sizes: HashMap<String, u32>,
    // (struct_name, field_name) -> tipo del campo. Necesario para resolver
    // offsets de accesos anidados (a->b->c, a.b.c) sin adivinar.
    field_types: HashMap<(String, String), TypeSpec>,
    usings: Vec<String>,
    typedefs: HashMap<String, TypeSpec>,
    /// Constantes de `enum` con su VALOR.
    ///
    /// Antes el parser calculaba el valor y lo tiraba: registraba el nombre
    /// como si fuera una variable `int` y nunca guardaba a qué equivalia,
    /// asi que `enum { ROJO, VERDE }` dejaba `VERDE` como una variable sin
    /// definir. El aviso `value assigned to val is never read` del propio
    /// compilador estaba senalando justo este bug.
    enum_constants: HashMap<String, i64>,
    /// ★ Locales `static`: nombre visible → nombre real de la global.
    ///
    /// Una `static` dentro de una función **no es una local**: sobrevive entre
    /// llamadas, así que vive donde viven las globales. Pero su NOMBRE sólo se
    /// ve dentro de su función, y dos funciones pueden tener cada una su
    /// `static int n`. Se resuelve renombrando en la declaración y traduciendo
    /// en el único sitio donde un identificador se vuelve variable.
    ///
    /// Se vacía al empezar cada función: ese ámbito es justo lo que este mapa
    /// representa.
    static_alias: HashMap<String, String>,
    /// Globales que una función ha ido creando al declarar sus `static`.
    /// `parse_program` las recoge al terminar cada función.
    globales_pendientes: Vec<GlobalDecl>,
    syscalls: HashMap<String, SyscallDef>,
    /// Lo que el LEXER no pudo leer. Se comprueba antes de parsear: seguir
    /// con un token inventado produce un programa que compila y no dice lo
    /// que está escrito.
    lex_errores: Vec<CError>,
    pub(crate) features: StandardFeatures,
}

impl Parser {
    pub(crate) fn new(source: &str) -> Self {
        let (tokens, token_lines, lex_errores) = crate::lexer::tokenize(source);
        Self {
            tokens, token_lines, pos: 0, lex_errores,
            var_types: HashMap::new(),
            struct_fields: HashMap::new(),
            struct_sizes: HashMap::new(),
            field_types: HashMap::new(),
            usings: Vec::new(),
            typedefs: HashMap::new(),
            enum_constants: HashMap::new(),
            static_alias: HashMap::new(),
            globales_pendientes: Vec::new(),
            syscalls: HashMap::new(),
            features: StandardFeatures::default(),
        }
    }

    #[cfg(test)]
    pub(crate) fn tokenize_for_test(source: &str) -> Vec<Token> { crate::lexer::tokenize(source).0 }


    fn peek(&self) -> &Token { &self.tokens[self.pos] }
    fn advance(&mut self) -> Token { let t = self.tokens[self.pos].clone(); self.pos += 1; t }

    /// Línea del token actual (para errores con ubicación REAL, no "línea 1").
    fn line(&self) -> usize {
        self.token_lines.get(self.pos.min(self.token_lines.len().saturating_sub(1)))
            .copied().unwrap_or(1)
    }

    fn get_field_offset(&self, struct_name: &str, field: &str) -> Option<u32> {
        self.struct_fields.get(struct_name).and_then(|fields| {
            fields.iter().find(|(n, _, _)| n == field).map(|(_, off, _)| *off)
        })
    }

    /// Nombre del struct/union del que un TypeSpec ES valor directo.
    fn struct_of(t: &TypeSpec) -> Option<&str> {
        match t {
            TypeSpec::StructRef(s) | TypeSpec::UnionRef(s) => Some(s),
            _ => None,
        }
    }

    /// Nombre del struct/union al que un TypeSpec APUNTA (un nivel de *).
    fn pointee_struct_of(t: &TypeSpec) -> Option<&str> {
        match t {
            TypeSpec::Ptr(base) => Self::struct_of(base),
            _ => None,
        }
    }

    /// Tipo estático de una expresión, hasta donde el parser puede saberlo.
    /// Devuelve None si no es resoluble (y el offset caerá a 0 — visible en tests).
    fn resolve_expr_type(&self, expr: &Expr) -> Option<TypeSpec> {
        match expr {
            Expr::Var(n) => self.var_types.get(n).cloned(),
            Expr::Subscript(n, _, _) => {
                // arr[i]: el tipo del elemento
                match self.var_types.get(n)? {
                    TypeSpec::Ptr(base) => Some(base.as_ref().clone()),
                    TypeSpec::Array(base, _) => Some(base.as_ref().clone()),
                    t => Some(t.clone()),
                }
            }
            Expr::Deref(inner) => {
                match self.resolve_expr_type(inner)? {
                    TypeSpec::Ptr(base) => Some(*base),
                    _ => None,
                }
            }
            Expr::AddrOf(inner) => {
                let t = self.resolve_expr_type(inner)?;
                Some(TypeSpec::Ptr(Box::new(t)))
            }
            Expr::Field(base, fname, _, _) => {
                // base.f: tipo del campo f en el struct de base
                let s = self.resolve_struct_type(base)?;
                self.field_types.get(&(s, fname.clone())).cloned()
            }
            Expr::Arrow(base, fname, _, _) => {
                // base->f: tipo del campo f en el struct APUNTADO por base
                let t = self.resolve_expr_type(base)?;
                let s = Self::pointee_struct_of(&t)?.to_string();
                self.field_types.get(&(s, fname.clone())).cloned()
            }
            _ => None,
        }
    }

    /// Struct/union del que la expresión ES valor (para `expr.field`).
    fn resolve_struct_type(&self, expr: &Expr) -> Option<String> {
        let t = self.resolve_expr_type(expr)?;
        match &t {
            TypeSpec::StructRef(s) | TypeSpec::UnionRef(s) => Some(s.clone()),
            // permisivo histórico: p[i] con p: struct* ya cae en resolve_expr_type
            _ => None,
        }
    }

    fn resolve_field_expr_offset(&self, expr: &Expr, field: &str) -> u32 {
        self.resolve_struct_type(expr)
            .and_then(|s| self.get_field_offset(&s, field))
            .unwrap_or(0)
    }

    /// Tipo del campo para `expr.field` (base por valor).
    fn field_type_via_value(&self, expr: &Expr, field: &str) -> TypeSpec {
        self.resolve_struct_type(expr)
            .and_then(|s| self.field_types.get(&(s, field.to_string())).cloned())
            .unwrap_or(TypeSpec::Long)
    }

    /// Tipo del campo para `expr->field` (base puntero).
    fn field_type_via_pointer(&self, expr: &Expr, field: &str) -> TypeSpec {
        self.resolve_expr_type(expr)
            .and_then(|t| Self::pointee_struct_of(&t).map(str::to_string))
            .and_then(|s| self.field_types.get(&(s, field.to_string())).cloned())
            .unwrap_or(TypeSpec::Long)
    }

    fn resolve_arrow_expr_offset(&self, expr: &Expr, field: &str) -> u32 {
        // expr->field: expr es puntero a struct; funciona ANIDADO (a->b->c)
        // porque resolve_expr_type sigue los tipos de campo registrados.
        self.resolve_expr_type(expr)
            .and_then(|t| Self::pointee_struct_of(&t).map(str::to_string))
            .and_then(|s| self.get_field_offset(&s, field))
            .unwrap_or(0)
    }

    /// Tamaño del elemento apuntado/contenido por `base` (para escalar subíndices).
    fn pointee_size(&self, base: &TypeSpec) -> u8 {
        match base {
            TypeSpec::Char | TypeSpec::UnsignedChar => 1,
            TypeSpec::Short | TypeSpec::UnsignedShort => 2,
            TypeSpec::Int | TypeSpec::UnsignedInt => 4,
            TypeSpec::Float => 4, TypeSpec::Double => 8,
            TypeSpec::Void => 1,
            TypeSpec::StructRef(s) | TypeSpec::UnionRef(s) => *self.struct_sizes.get(s.as_str()).unwrap_or(&8) as u8,
            _ => 8,
        }
    }

    fn element_size(&self, name: &str) -> u8 {
        if let Some(typ) = self.var_types.get(name) {
            match typ {
                TypeSpec::Char => 1, TypeSpec::UnsignedChar => 1,
                TypeSpec::Short => 2, TypeSpec::UnsignedShort => 2,
                TypeSpec::Int => 4, TypeSpec::UnsignedInt => 4,
                TypeSpec::Long | TypeSpec::UnsignedLong => 8,
                TypeSpec::Ptr(ref base) => self.pointee_size(base),
                TypeSpec::Array(ref base, _) => self.pointee_size(base),
                _ => 8,
            }
        } else { 8 }
    }

    fn compute_struct_layout(&mut self, name: &str, members: &[StructMember]) {
        let mut layout = Vec::new();
        let mut offset = 0u32;
        for m in members {
            let sz = m.typ.stack_size();
            let align = sz.min(8).max(1);
            offset = (offset + align - 1) / align * align;
            layout.push((m.name.clone(), offset, sz));
            self.field_types.insert((name.to_string(), m.name.clone()), m.typ.clone());
            offset += sz;
        }
        let max_align = members.iter().map(|m| m.typ.stack_size().min(8).max(1)).max().unwrap_or(1);
        let total = (offset + max_align - 1) / max_align * max_align;
        self.struct_fields.insert(name.to_string(), layout);
        self.struct_sizes.insert(name.to_string(), total);
    }

    fn compute_union_layout(&mut self, name: &str, members: &[StructMember]) {
        let mut layout = Vec::new();
        let mut max_sz = 0u32;
        for m in members {
            let sz = m.typ.stack_size();
            layout.push((m.name.clone(), 0u32, sz));
            self.field_types.insert((name.to_string(), m.name.clone()), m.typ.clone());
            if sz > max_sz { max_sz = sz; }
        }
        self.struct_fields.insert(name.to_string(), layout);
        self.struct_sizes.insert(name.to_string(), max_sz);
    }

    fn expect(&mut self, expected: &Token) -> Result<Token, CError> {
        if *self.peek() != *expected {
            return Err(CError::new(self.line(),format!("expected {:?}, got {:?}", expected, self.peek())));
        }
        Ok(self.advance())
    }

    fn skip_semicolon(&mut self) {
        if *self.peek() == Token::Semicolon { self.advance(); }
    }

    // ---- Program ----
    pub(crate) fn parse_program(&mut self) -> Result<Program, CError> {
        if let Some(e) = self.lex_errores.first() {
            return Err(e.clone());
        }
        let mut globals = Vec::new();
        let mut functions = Vec::new();
        while *self.peek() != Token::Eof {
        // ★ Una directiva del preprocesador. No hay preprocesador, y hasta
        // ahora eso no se decia: el `#` se lo tragaba el lexer, asi que un
        // `#define X 5` dentro de una funcion compilaba **y se ignoraba en
        // silencio** — el programa corria con X sin sustituir. Un no-op
        // callado es peor que un "no".
        if *self.peek() == Token::Hash {
            return Err(CError::new(
                self.line(),
                "aqui no hay preprocesador todavia: '#define', '#include' y '#ifdef' no se procesan. Usa 'const int' o 'enum' para las constantes",
            ));
        }
            if *self.peek() == Token::Struct || *self.peek() == Token::Union {
                let is_union = *self.peek() == Token::Union;
                self.advance();
                let name = match self.advance() {
                    Token::Ident(n) => n,
                    t => return Err(CError::new(self.line(),format!("expected struct name, got {:?}", t))),
                };
                if *self.peek() == Token::OpenBrace {
                    self.advance();
                    let mut members = Vec::new();
                    while *self.peek() != Token::CloseBrace && *self.peek() != Token::Eof {
                        let mtype = self.parse_type_spec()?;
                        let mname = match self.advance() {
                            Token::Ident(n) => n,
                            t => return Err(CError::new(self.line(),format!("expected member name, got {:?}", t))),
                        };
                        self.skip_semicolon();
                        members.push(StructMember { typ: mtype, name: mname });
                    }
                    self.expect(&Token::CloseBrace)?;
                    self.skip_semicolon();
                    if is_union {
                        self.compute_union_layout(&name, &members);
                        globals.push(GlobalDecl::Union(name, members));
                    } else {
                        self.compute_struct_layout(&name, &members);
                        globals.push(GlobalDecl::Struct(name, members));
                    }
                } else {
                    // `struct P nombre` — o una variable global, o una función
                    // que DEVUELVE el struct.
                    if let Token::Ident(vname) = self.advance() {
                        // ★ Devolver un agregado por valor es un mecanismo
                        // aparte (puntero oculto) y todavía no está. Decirlo
                        // AQUÍ y con el nombre delante: si se deja caer, el
                        // parser encuentra el `(` donde esperaba un `;` y suelta
                        // "expected type, got OpenParen", que manda a mirar el
                        // tipo — y el tipo está perfectamente.
                        if *self.peek() == Token::OpenParen {
                            return Err(CError::new(
                                self.line(),
                                format!(
                                    "'{vname}' devuelve un struct por valor, y eso aun no se \
                                     compila: pasa un puntero al destino como parametro"
                                ),
                            ));
                        }
                        let typ = if is_union { TypeSpec::UnionRef(name) } else { TypeSpec::StructRef(name) };
                        self.skip_semicolon();
                        globals.push(GlobalDecl::Var(typ.clone(), vname.clone(), None));
                        self.var_types.insert(vname, typ);
                    }
                }
                continue;
            }
            if *self.peek() == Token::Enum {
                self.advance();
                let _name = match self.advance() {
                    Token::Ident(n) => n,
                    t => return Err(CError::new(self.line(),format!("expected enum name, got {:?}", t))),
                };
                self.expect(&Token::OpenBrace)?;
                let mut val = 0i64;
                loop {
                    match self.advance() {
                        Token::Ident(en) => {
                            if *self.peek() == Token::Assign {
                                self.advance();
                                let assigned = match self.advance() {
                                    Token::IntLit(n) => n,
                                    t => return Err(CError::new(self.line(),format!("expected int in enum, got {:?}", t))),
                                };
                                val = assigned;
                            }
                            // La constante se resuelve a su VALOR al usarla
                            // (ver parse_primary); el tipo sigue siendo int.
                            self.var_types.insert(en.clone(), TypeSpec::Int);
                            self.enum_constants.insert(en.clone(), val);
                        }
                        Token::CloseBrace => { break; }
                        t => return Err(CError::new(self.line(),format!("expected enum constant, got {:?}", t))),
                    }
                    val += 1;
                    if *self.peek() == Token::Comma { self.advance(); }
                }
                self.skip_semicolon();
                continue;
            }
            if *self.peek() == Token::Use {
                self.advance();
                let path = match self.advance() {
                    Token::StringLit(s) => s,
                    t => return Err(CError::new(self.line(),format!("expected module path string, got {:?}", t))),
                };
                self.skip_semicolon();
                self.usings.push(path);
                continue;
            }
            if *self.peek() == Token::Extern {
                self.advance();
                let (typ, name) = self.parse_type_and_name()?;
                self.skip_semicolon();
                self.var_types.insert(name.clone(), typ.clone());
                globals.push(GlobalDecl::Var(typ, name, None));
                continue;
            }
            // ★ `static` EN EL NIVEL DE FICHERO: se acepta y se sigue de largo.
            //
            // Ahí `static` significa "este nombre no sale de esta unidad de
            // traducción" — enlace interno. BMO C compila **una** unidad, así
            // que no hay nadie de quien esconderse: una global static y una
            // global normal se comportan exactamente igual.
            //
            // No es tragárselo por comodidad: es que aquí no cambia lo que el
            // programa hace, y emitir algo distinto sería inventarse una
            // diferencia. El día que haya compilación separada, esta línea es
            // el sitio donde ponerle el enlace interno de verdad.
            //
            // Dentro de una función es OTRA COSA y sí cambia el programa: ver
            // `terminar_declaracion_static`.
            if *self.peek() == Token::Static {
                self.advance();
                continue;
            }
            if *self.peek() == Token::Typedef {
                self.advance();
                let typ = self.parse_type_spec()?;
                let name = match self.advance() {
                    Token::Ident(n) => n,
                    t => return Err(CError::new(self.line(),format!("expected typedef name, got {:?}", t))),
                };
                self.skip_semicolon();
                self.typedefs.insert(name, typ);
                continue;
            }
            match self.try_parse_function()? {
                Tope::Funcion(f) => {
                    functions.push(f);
                    // Las `static` que esa función haya declarado ya no son
                    // suyas: son globales con nombre propio.
                    globals.append(&mut self.globales_pendientes);
                    continue;
                }
                // Un PROTOTIPO: `int f(int);`. No emite nada — sólo dice que
                // esa función existirá. Ya se consumió, así que se sigue.
                Tope::Prototipo => continue,
                Tope::NoEsFuncion => {}
            }
            {
                let (typ, name) = self.parse_type_and_name()?;
                let init = if *self.peek() == Token::Assign {
                    self.advance();
                    Some(self.parse_assign()?)
                } else {
                    None
                };
                self.skip_semicolon();
                self.var_types.insert(name.clone(), typ.clone());
                globals.push(GlobalDecl::Var(typ, name, init));
            }
        }
        Ok(Program { globals, functions, exported: Vec::new() })
    }

    /// Parse with module resolution. Returns merged Program with all dependency sources.
    /// If `asm_paths` is provided, also loads Semantic_ASM .toml files for each `use` directive.
    pub(crate) fn parse_program_with_modules(
        &mut self,
        resolver: &mut module::ModuleResolver,
        asm_paths: Option<Vec<PathBuf>>,
    ) -> Result<Program, CError> {
        let mut program = self.parse_program()?;
        // Syscall defs and module manifests are loaded AFTER parse_program().
        // We must post-process the AST to convert Expr::Call â†’ Expr::Syscall
        // for any function names that match a loaded syscall definition.
        let usings = std::mem::take(&mut self.usings);
        for path in &usings {
            // Load module sources (optional â€” module may not exist for syscall-only paths)
            if let Ok(manifest) = resolver.find_manifest(path) {
                let mod_dir = resolver.find_base_dir(path);
                for src_file in &manifest.source_files {
                    let full_path = mod_dir.join(src_file);
                    let source = std::fs::read_to_string(&full_path)
                        .map_err(|e| CError::new(0, format!("cannot read module source {}: {e}", full_path.display())))?;
                    let mut sub = Parser::new(&source);
                    let sub_prog = sub.parse_program()?;
                    for f in sub_prog.functions {
                        if !program.functions.iter().any(|pf| pf.name == f.name) {
                            program.functions.push(f);
                        }
                    }
                    for g in sub_prog.globals {
                        if !program.globals.iter().any(|pg| std::mem::discriminant(pg) == std::mem::discriminant(&g)) {
                            program.globals.push(g);
                        }
                    }
                    for (k, v) in sub.struct_fields {
                        self.struct_fields.entry(k).or_insert(v);
                    }
                    for (k, v) in sub.struct_sizes {
                        self.struct_sizes.entry(k).or_insert(v);
                    }
                    for (k, v) in sub.field_types {
                        self.field_types.entry(k).or_insert(v);
                    }
                    for (k, v) in sub.var_types {
                        self.var_types.entry(k).or_insert(v);
                    }
                    for (k, v) in sub.typedefs {
                        self.typedefs.entry(k).or_insert(v);
                    }
                }
                program.exported.extend(manifest.exports);
            }

            // Load syscall definitions from embedded registry
            if self.syscalls.is_empty() {
                for d in bmo_abi::asm::defs::syscalls() {
                    self.syscalls.entry(d.name.clone()).or_insert(SyscallDef { name: d.name, nr: d.nr, arg_count: d.arg_count });
                }
            }
        }
        // Post-process: convert Expr::Call(name,args) â†’ Expr::Syscall(def,args)
        // for any function calls whose name matches a loaded syscall definition.
        self.resolve_syscalls_in_program(&mut program);
        // Validate syscall argument counts
        self.validate_syscall_args(&program)?;
        Ok(program)
    }

    /// Validate that all Expr::Syscall nodes have the correct argument count.
    fn validate_syscall_args(&self, program: &Program) -> Result<(), CError> {
        for func in &program.functions {
            Self::check_syscall_args_in_stmt_slice(&func.body, func.line)?;
        }
        Ok(())
    }

    fn check_syscall_args_in_stmt_slice(stmts: &[Stmt], line: usize) -> Result<(), CError> {
        for stmt in stmts {
            Self::check_syscall_args_in_stmt(stmt, line)?;
        }
        Ok(())
    }

    fn check_syscall_args_in_stmt(stmt: &Stmt, line: usize) -> Result<(), CError> {
        match stmt {
            Stmt::If(cond, t, e) => {
                Self::check_syscall_args_in_expr(cond, line)?;
                Self::check_syscall_args_in_stmt(t, line)?;
                if let Some(el) = e { Self::check_syscall_args_in_stmt(el, line)?; }
            }
            Stmt::While(cond, body) => {
                Self::check_syscall_args_in_expr(cond, line)?;
                Self::check_syscall_args_in_stmt(body, line)?;
            }
            Stmt::DoWhile(body, cond) => {
                Self::check_syscall_args_in_stmt(body, line)?;
                Self::check_syscall_args_in_expr(cond, line)?;
            }
            Stmt::For(init, cond, inc, body) => {
                if let Some(e) = init { Self::check_syscall_args_in_expr(e, line)?; }
                if let Some(e) = cond { Self::check_syscall_args_in_expr(e, line)?; }
                if let Some(e) = inc { Self::check_syscall_args_in_expr(e, line)?; }
                Self::check_syscall_args_in_stmt(body, line)?;
            }
            Stmt::Switch(expr, cases) => {
                Self::check_syscall_args_in_expr(expr, line)?;
                for c in cases { Self::check_syscall_args_in_stmt_slice(&c.stmts, line)?; }
            }
            Stmt::Block(stmts) => Self::check_syscall_args_in_stmt_slice(stmts, line)?,
            Stmt::Expr(e) | Stmt::Return(Some(e)) => Self::check_syscall_args_in_expr(e, line)?,
            Stmt::DeclAssign(_, _, Some(e)) => Self::check_syscall_args_in_expr(e, line)?,
            Stmt::DeclInit(_, _, es) => { for e in es { Self::check_syscall_args_in_expr(&e.valor, line)?; } }
            _ => {}
        }
        Ok(())
    }

    fn check_syscall_args_in_expr(expr: &Expr, line: usize) -> Result<(), CError> {
        match expr {
            Expr::Syscall(def, args) => {
                if args.len() != def.arg_count as usize {
                    return Err(CError::new(line, format!(
                        "syscall {}() expects {} arguments, got {}",
                        def.name, def.arg_count, args.len()
                    )));
                }
                for a in args { Self::check_syscall_args_in_expr(a, line)?; }
            }
            Expr::Neg(a) | Expr::Not(a) | Expr::BitNot(a) | Expr::Deref(a) | Expr::AddrOf(a)
                => Self::check_syscall_args_in_expr(a, line)?,
            Expr::Add(a,b) | Expr::Sub(a,b) | Expr::Mul(a,b) | Expr::Div(a,b) | Expr::Mod(a,b)
                | Expr::Eq(a,b) | Expr::Neq(a,b) | Expr::Lt(a,b) | Expr::Gt(a,b) | Expr::Le(a,b) | Expr::Ge(a,b)
                | Expr::BitAnd(a,b) | Expr::BitXor(a,b) | Expr::BitOr(a,b) | Expr::LAnd(a,b) | Expr::LOr(a,b)
                | Expr::Shl(a,b) | Expr::Shr(a,b) => {
                Self::check_syscall_args_in_expr(a, line)?;
                Self::check_syscall_args_in_expr(b, line)?;
            }
            Expr::Conditional(c,t,f) => {
                Self::check_syscall_args_in_expr(c, line)?;
                Self::check_syscall_args_in_expr(t, line)?;
                Self::check_syscall_args_in_expr(f, line)?;
            }
            Expr::Call(_, args) | Expr::Comma(args) => {
                for a in args { Self::check_syscall_args_in_expr(a, line)?; }
            }
            Expr::Arrow(p,_,_,_) | Expr::AssignArrow(p,_,_,_,_) => Self::check_syscall_args_in_expr(p, line)?,
            Expr::Assign(_, v) | Expr::AssignField(_,_,_,_,v) => Self::check_syscall_args_in_expr(v, line)?,
            Expr::AssignDeref(a, v) => { Self::check_syscall_args_in_expr(a, line)?; Self::check_syscall_args_in_expr(v, line)?; }
            Expr::Field(b,_,_,_) => Self::check_syscall_args_in_expr(b, line)?,
            Expr::Cast(_, a) => Self::check_syscall_args_in_expr(a, line)?,
            Expr::Intrinsic(_, args) => { for a in args { Self::check_syscall_args_in_expr(a, line)?; } }
            Expr::IndexPtr(b, idx, _) => { Self::check_syscall_args_in_expr(b, line)?; Self::check_syscall_args_in_expr(idx, line)?; }
            Expr::AssignIndexPtr(b, idx, _, v) => { Self::check_syscall_args_in_expr(b, line)?; Self::check_syscall_args_in_expr(idx, line)?; Self::check_syscall_args_in_expr(v, line)?; }
            Expr::CallPtr(c, args) => { Self::check_syscall_args_in_expr(c, line)?; for a in args { Self::check_syscall_args_in_expr(a, line)?; } }
            Expr::Subscript(_, idx, _) => Self::check_syscall_args_in_expr(idx, line)?,
            Expr::AssignSubscript(_, idx, _, v) => { Self::check_syscall_args_in_expr(idx, line)?; Self::check_syscall_args_in_expr(v, line)?; }
            _ => {}
        }
        Ok(())
    }

    /// Walk all function bodies and convert Expr::Call â†’ Expr::Syscall for
    /// any function calls whose name matches a loaded syscall definition.
    fn resolve_syscalls_in_program(&self, program: &mut Program) {
        for func in &mut program.functions {
            Self::resolve_syscalls_in_stmt_slice(&self.syscalls, &mut func.body);
        }
    }

    fn resolve_syscalls_in_stmt_slice(syscalls: &HashMap<String, SyscallDef>, stmts: &mut Vec<Stmt>) {
        for stmt in stmts.iter_mut() {
            Self::resolve_syscalls_in_stmt(syscalls, stmt);
        }
    }

    fn resolve_syscalls_in_stmt(syscalls: &HashMap<String, SyscallDef>, stmt: &mut Stmt) {
        match stmt {
            Stmt::If(cond, t, e) => {
                Self::resolve_syscalls_in_expr(syscalls, cond);
                Self::resolve_syscalls_in_stmt(syscalls, t);
                if let Some(el) = e { Self::resolve_syscalls_in_stmt(syscalls, el); }
            }
            Stmt::While(cond, body) => {
                Self::resolve_syscalls_in_expr(syscalls, cond);
                Self::resolve_syscalls_in_stmt(syscalls, body);
            }
            Stmt::DoWhile(body, cond) => {
                Self::resolve_syscalls_in_stmt(syscalls, body);
                Self::resolve_syscalls_in_expr(syscalls, cond);
            }
            Stmt::For(init, cond, inc, body) => {
                if let Some(e) = init { Self::resolve_syscalls_in_expr(syscalls, e); }
                if let Some(e) = cond { Self::resolve_syscalls_in_expr(syscalls, e); }
                if let Some(e) = inc { Self::resolve_syscalls_in_expr(syscalls, e); }
                Self::resolve_syscalls_in_stmt(syscalls, body);
            }
            Stmt::Switch(expr, cases) => {
                Self::resolve_syscalls_in_expr(syscalls, expr);
                for c in cases { Self::resolve_syscalls_in_stmt_slice(syscalls, &mut c.stmts); }
            }
            Stmt::Block(stmts) => Self::resolve_syscalls_in_stmt_slice(syscalls, stmts),
            Stmt::Expr(e) | Stmt::Return(Some(e)) => Self::resolve_syscalls_in_expr(syscalls, e),
            Stmt::DeclAssign(_, _, Some(e)) => Self::resolve_syscalls_in_expr(syscalls, e),
            Stmt::DeclInit(_, _, es) => { for e in es { Self::resolve_syscalls_in_expr(syscalls, &mut e.valor); } }
            _ => {}
        }
    }

    fn resolve_syscalls_in_expr(syscalls: &HashMap<String, SyscallDef>, expr: &mut Expr) {
        match expr {
            Expr::Call(name, args) => {
                let mut new_args = std::mem::take(args);
                // Resolve syscalls in args first (before we potentially move them)
                for a in new_args.iter_mut() {
                    Self::resolve_syscalls_in_expr(syscalls, a);
                }
                if let Some(def) = syscalls.get(name).cloned() {
                    *expr = Expr::Syscall(def, new_args);
                } else {
                    *expr = Expr::Call(std::mem::take(name), new_args);
                }
            }
            Expr::Syscall(_, args) => {
                for a in args.iter_mut() {
                    Self::resolve_syscalls_in_expr(syscalls, a);
                }
            }
            Expr::Neg(a) | Expr::Not(a) | Expr::BitNot(a) | Expr::Deref(a) | Expr::AddrOf(a) => Self::resolve_syscalls_in_expr(syscalls, a),
            Expr::Add(a,b) | Expr::Sub(a,b) | Expr::Mul(a,b) | Expr::Div(a,b) | Expr::Mod(a,b)
                | Expr::Eq(a,b) | Expr::Neq(a,b) | Expr::Lt(a,b) | Expr::Gt(a,b) | Expr::Le(a,b) | Expr::Ge(a,b)
                | Expr::BitAnd(a,b) | Expr::BitXor(a,b) | Expr::BitOr(a,b) | Expr::LAnd(a,b) | Expr::LOr(a,b)
                | Expr::Shl(a,b) | Expr::Shr(a,b) => {
                Self::resolve_syscalls_in_expr(syscalls, a);
                Self::resolve_syscalls_in_expr(syscalls, b);
            }
            Expr::Conditional(c,t,f) => {
                Self::resolve_syscalls_in_expr(syscalls, c);
                Self::resolve_syscalls_in_expr(syscalls, t);
                Self::resolve_syscalls_in_expr(syscalls, f);
            }
            Expr::Arrow(p,_,_,_) | Expr::AssignArrow(p,_,_,_,_) => Self::resolve_syscalls_in_expr(syscalls, p),
            Expr::Assign(_, v) | Expr::AssignField(_,_,_,_,v) => Self::resolve_syscalls_in_expr(syscalls, v),
            Expr::AssignDeref(a, v) => { Self::resolve_syscalls_in_expr(syscalls, a); Self::resolve_syscalls_in_expr(syscalls, v); }
            Expr::Field(b,_,_,_) => Self::resolve_syscalls_in_expr(syscalls, b),
            Expr::Cast(_, a) => Self::resolve_syscalls_in_expr(syscalls, a),
            Expr::Intrinsic(_, args) => { for a in args { Self::resolve_syscalls_in_expr(syscalls, a); } }
            Expr::IndexPtr(b, idx, _) => { Self::resolve_syscalls_in_expr(syscalls, b); Self::resolve_syscalls_in_expr(syscalls, idx); }
            Expr::AssignIndexPtr(b, idx, _, v) => { Self::resolve_syscalls_in_expr(syscalls, b); Self::resolve_syscalls_in_expr(syscalls, idx); Self::resolve_syscalls_in_expr(syscalls, v); }
            Expr::CallPtr(c, args) => { Self::resolve_syscalls_in_expr(syscalls, c); for a in args { Self::resolve_syscalls_in_expr(syscalls, a); } }
            Expr::Subscript(_, idx, _) => Self::resolve_syscalls_in_expr(syscalls, idx),
            Expr::AssignSubscript(_, idx, _, v) => { Self::resolve_syscalls_in_expr(syscalls, idx); Self::resolve_syscalls_in_expr(syscalls, v); }
            Expr::Comma(v) => { for e in v { Self::resolve_syscalls_in_expr(syscalls, e); } }
            _ => {}
        }
    }

    fn try_parse_function(&mut self) -> Result<Tope, CError> {
        let save = self.pos;
        let start_line = self.line();
        // `static int f(){...}` — el `static` de una función es enlace interno,
        // y aquí sólo hay una unidad de traducción. Se acepta y se sigue.
        if *self.peek() == Token::Static {
            self.advance();
        }
        let ret_type = match self.parse_type_spec() {
            Ok(t) => t,
            Err(_) => { self.pos = save; return Ok(Tope::NoEsFuncion); }
        };
        let Token::Ident(name) = self.peek().clone() else { self.pos = save; return Ok(Tope::NoEsFuncion); };
        self.advance();
        if *self.peek() != Token::OpenParen { self.pos = save; return Ok(Tope::NoEsFuncion); }
        self.advance();
        let mut params = Vec::new();
        let mut anonimos = 0usize;
        while *self.peek() != Token::CloseParen && *self.peek() != Token::Eof {
            if *self.peek() == Token::Void && (self.pos + 1 >= self.tokens.len() || self.tokens[self.pos + 1] == Token::CloseParen) {
                self.advance(); break;
            }
            let ptype = self.parse_type_spec()?;
            // ★ El nombre del parámetro es OPCIONAL.
            //
            // `int f(int);` es C legal y es como se escriben los prototipos en
            // las cabeceras de cualquier programa de verdad — DOOM incluido.
            // Aquí se exigía nombre siempre, así que un prototipo moría con
            // "expected param name, got CloseParen": un mensaje que acusa al
            // programa de algo que el estándar permite.
            //
            // Sin nombre no se puede referenciar dentro del cuerpo, y por eso
            // sólo aparece en declaraciones. Se le pone uno inventado para que
            // el resto del compilador no tenga que saber que puede faltar.
            let pname = match self.peek().clone() {
                Token::Ident(n) => { self.advance(); n }
                Token::Comma | Token::CloseParen => {
                    anonimos += 1;
                    format!("_anon{}", anonimos)
                }
                t => return Err(CError::new(self.line(),format!("expected param name, got {:?}", t))),
            };
            // ★ El tipo de un PARÁMETRO también se registra.
            //
            // Sólo se guardaba el de las variables locales, así que dentro de
            // `int suma(struct P p)` el parser no sabía que `p` era un struct:
            // `p.x` salía como un campo de offset 0 y tipo `long`, y los tres
            // campos leían **la misma dirección y ocho bytes**. Daba
            // `0x200000001` — las dos primeras `int` juntas — en vez de 1.
            //
            // Mientras un parámetro sólo pudo ser un escalar esto no se notaba:
            // ningún escalar tiene campos que consultar.
            self.var_types.insert(pname.clone(), ptype.clone());
            params.push(Param { typ: ptype, name: pname });
            if *self.peek() == Token::Comma { self.advance(); }
        }
        self.expect(&Token::CloseParen)?;
        // ★ PROTOTIPO: `int f(int a);` — declarar sin definir.
        //
        // Sin esto no se puede llamar a una función antes de escribirla, y eso
        // no es una comodidad: **la recursión mutua es imposible sin ella**. Un
        // programa de cincuenta ficheros —DOOM son unos cincuenta— está lleno
        // de funciones que se llaman en círculo, y ninguna puede ir "antes" de
        // todas las demás. Era el hueco más caro de los que quedaban, y no se
        // sabía que estaba: el lexer no tiene la culpa de nada aquí.
        //
        // No emite código. Lo único que deja es el tipo de retorno anotado,
        // para que una llamada anterior a la definición sepa qué recibe.
        if *self.peek() == Token::Semicolon {
            self.advance();
            self.var_types.insert(name.clone(), ret_type);
            return Ok(Tope::Prototipo);
        }
        // After expect advances past ), pos should be at {
        if self.pos >= self.tokens.len() || *self.peek() != Token::OpenBrace { self.pos = save; return Ok(Tope::NoEsFuncion); }
        self.advance();
        // Cada función empieza sin `static` heredadas de la anterior: el mapa
        // ES el ámbito.
        self.static_alias.clear();
        let mut var_count = 0u32;
        let mut var_names: Vec<String> = params.iter().map(|p| p.name.clone()).collect();
        let mut body = Vec::new();
        loop {
            match self.peek() {
                Token::CloseBrace => { self.advance(); break; }
                Token::Eof => return Err(CError::new(self.line(),"unexpected eof in function body")),
                _ => {
                    // check for label: ident followed by colon
                    if let Token::Ident(name) = self.peek().clone() {
                        if self.pos + 1 < self.tokens.len() && self.tokens[self.pos + 1] == Token::Colon {
                            self.advance();
                            self.advance();
                            body.push(Stmt::Label(name));
                            continue;
                        }
                    }
                    // ★ Una local `static` NO es una local: se va a las
                    // globales y aquí no queda nada.
                    if *self.peek() == Token::Static {
                        self.advance();
                        let Some((typ, vname)) = self.try_parse_decl()? else {
                            return Err(CError::new(self.line(),
                                "static: esperaba una declaracion de variable"));
                        };
                        self.declarar_static_local(&name, typ, vname)?;
                        continue;
                    }
                    if let Some((typ, name)) = self.try_parse_decl()? {
                        var_count += 1;
                        var_names.push(name.clone());
                        body.push(self.terminar_declaracion(typ, name)?);
                    } else {
                        body.push(self.parse_stmt()?);
                    }
                }
            }
        }
        Ok(Tope::Funcion(Function { ret_type, name, params, var_count, var_names, body, line: start_line }))
    }

    /// **Una `static` dentro de una función.**
    ///
    /// Aquí `static` sí cambia lo que el programa hace, y en dos cosas a la vez:
    ///
    /// 1. **Sobrevive entre llamadas.** No puede vivir en la pila, que se
    ///    deshace al volver: vive donde viven las globales.
    /// 2. **Su inicializador corre UNA vez**, no en cada llamada. Por eso el
    ///    valor viaja con la global y **no se emite ninguna sentencia** en el
    ///    cuerpo — si se emitiera una asignación, un contador `static int n=0`
    ///    se pondría a cero en cada llamada y parecería que no cuenta nada.
    ///
    /// Lo que NO cambia es su ámbito: el nombre sólo se ve dentro de su
    /// función, y dos funciones pueden tener cada una su `static int n`. De ahí
    /// el renombrado: la global se llama `funcion.variable` —con un punto, que
    /// un identificador de C no puede contener, así que no puede chocar con
    /// nada que el programa escriba— y el mapa de alias traduce.
    fn declarar_static_local(
        &mut self,
        funcion: &str,
        typ: TypeSpec,
        nombre: String,
    ) -> Result<(), CError> {
        let real = format!("{}.{}", funcion, nombre);
        let init = if *self.peek() == Token::Assign {
            self.advance();
            Some(self.parse_assign()?)
        } else {
            None
        };
        self.skip_semicolon();
        self.var_types.insert(real.clone(), typ.clone());
        self.static_alias.insert(nombre, real.clone());
        self.globales_pendientes.push(GlobalDecl::Var(typ, real, init));
        Ok(())
    }

    fn try_parse_decl(&mut self) -> Result<Option<(TypeSpec, String)>, CError> {
        let save = self.pos;
        if !self.peek_is_type_start() {
            return Ok(None);
        }
        let mut typ = match self.parse_type_spec() {
            Ok(t) => t,
            Err(_) => { self.pos = save; return Ok(None); }
        };
        // puntero a función: RETTYPE (*name)(params) — variable de tipo puntero.
        // Es lo que sostiene las vtables de C++ y las tablas de drivers.
        if *self.peek() == Token::OpenParen
            && self.tokens.get(self.pos + 1) == Some(&Token::Star)
        {
            match self.parse_fnptr_tail() {
                Ok(fname) => {
                    if *self.peek() != Token::Semicolon && *self.peek() != Token::Assign {
                        self.pos = save; return Ok(None);
                    }
                    return Ok(Some((TypeSpec::Ptr(Box::new(TypeSpec::Void)), fname)));
                }
                Err(_) => { self.pos = save; return Ok(None); }
            }
        }
        let Token::Ident(name) = self.peek().clone() else { self.pos = save; return Ok(None); };
        if self.pos + 1 < self.tokens.len() && self.tokens[self.pos + 1] == Token::OpenParen {
            self.pos = save; return Ok(None);
        }
        self.advance();
        // array declarator: name[size] — el tamaño SE GUARDA (antes se tiraba)
        if *self.peek() == Token::OpenBracket {
            self.advance();
            let size_expr = self.parse_expr()?;
            self.expect(&Token::CloseBracket)?;
            let n = match size_expr { Expr::Int(n) if n > 0 => n as u32, _ => 1 };
            typ = TypeSpec::Array(Box::new(typ), n);
        }
        if *self.peek() != Token::Semicolon && *self.peek() != Token::Assign {
            self.pos = save; return Ok(None);
        }
        Ok(Some((typ, name)))
    }

    /// Consume la cola de un puntero a función: `(*name)(param-types)`.
    /// Asume estar en el `(` inicial. Devuelve el nombre. El tipo del
    /// puntero es opaco (se trata como Ptr): las llamadas son indirectas.
    fn parse_fnptr_tail(&mut self) -> Result<String, CError> {
        self.expect(&Token::OpenParen)?;
        self.expect(&Token::Star)?;
        let name = match self.advance() {
            Token::Ident(n) => n,
            t => return Err(CError::new(self.line(), format!("expected fnptr name, got {:?}", t))),
        };
        self.expect(&Token::CloseParen)?;
        // saltar la lista de parámetros ( ... ) balanceada
        self.expect(&Token::OpenParen)?;
        let mut depth = 1;
        while depth > 0 {
            match self.advance() {
                Token::OpenParen => depth += 1,
                Token::CloseParen => depth -= 1,
                Token::Eof => return Err(CError::new(self.line(), "eof en lista de parametros de fnptr")),
                _ => {}
            }
        }
        Ok(name)
    }

    fn parse_type_and_name(&mut self) -> Result<(TypeSpec, String), CError> {
        let mut typ = self.parse_type_spec()?;
        // puntero a función en globals/params: RETTYPE (*name)(params)
        if *self.peek() == Token::OpenParen
            && self.tokens.get(self.pos + 1) == Some(&Token::Star)
        {
            let fname = self.parse_fnptr_tail()?;
            return Ok((TypeSpec::Ptr(Box::new(TypeSpec::Void)), fname));
        }
        let name = match self.advance() {
            Token::Ident(n) => n,
            t => return Err(CError::new(self.line(),format!("expected identifier, got {:?}", t))),
        };
        // array declarator [size] — el tamaño SE GUARDA (antes se tiraba)
        if *self.peek() == Token::OpenBracket {
            self.advance();
            let size_expr = self.parse_expr()?;
            self.expect(&Token::CloseBracket)?;
            let n = match size_expr { Expr::Int(n) if n > 0 => n as u32, _ => 1 };
            typ = TypeSpec::Array(Box::new(typ), n);
        }
        Ok((typ, name))
    }

    fn peek_is_type_start(&self) -> bool {
        match self.peek() {
            Token::Int | Token::Void | Token::Char | Token::Short | Token::Long |
            Token::Unsigned | Token::Signed | Token::Float | Token::Double |
            Token::Struct | Token::Union | Token::Enum | Token::Const | Token::Volatile => true,
            Token::Ident(name) => self.typedefs.contains_key(name),
            _ => false,
        }
    }

    fn strip_qualifiers(&mut self) {
        loop {
            match self.peek() {
                Token::Const | Token::Volatile => { self.advance(); }
                _ => break,
            }
        }
    }

    fn parse_type_spec(&mut self) -> Result<TypeSpec, CError> {
        self.strip_qualifiers();
        let base = match self.advance() {
            Token::Void => TypeSpec::Void,
            Token::Char => TypeSpec::Char,
            Token::Short => TypeSpec::Short,
            Token::Int => TypeSpec::Int,
            Token::Long => {
                if self.features.long_long && *self.peek() == Token::Long { self.advance(); TypeSpec::LongLong } else { TypeSpec::Long }
            }
            Token::Unsigned => {
                match self.peek() {
                    Token::Char => { self.advance(); TypeSpec::UnsignedChar }
                    Token::Short => { self.advance(); TypeSpec::UnsignedShort }
                    Token::Int => { self.advance(); TypeSpec::UnsignedInt }
                    Token::Long => {
                        self.advance();
                        if *self.peek() == Token::Long { self.advance(); TypeSpec::UnsignedLongLong }
                        else { TypeSpec::UnsignedLong }
                    }
                    _ => TypeSpec::UnsignedInt,
                }
            }
            Token::Signed => { self.advance(); TypeSpec::Int }
            Token::Float => TypeSpec::Float,
            Token::Double => TypeSpec::Double,
            Token::Struct => { 
                let name = match self.advance() {
                    Token::Ident(n) => n,
                    t => return Err(CError::new(self.line(),format!("expected struct name, got {:?}", t))),
                };
                TypeSpec::StructRef(name)
            }
            Token::Union => {
                let name = match self.advance() {
                    Token::Ident(n) => n,
                    t => return Err(CError::new(self.line(),format!("expected union name, got {:?}", t))),
                };
                TypeSpec::UnionRef(name)
            }
            Token::Ident(name) => {
                if let Some(typ) = self.typedefs.get(&name).cloned() {
                    typ
                } else {
                    return Err(CError::new(self.line(),format!("expected type, got {:?}", Token::Ident(name))));
                }
            }
            t => return Err(CError::new(self.line(),format!("expected type, got {:?}", t))),
        };
        // punteros multinivel: int **pp, char ***ppp, ...
        let mut typ = base;
        while *self.peek() == Token::Star {
            self.advance();
            typ = TypeSpec::Ptr(Box::new(typ));
        }
        Ok(typ)
    }

    // ---- Statements ----
    fn parse_stmt(&mut self) -> Result<Stmt, CError> {
        // Lo mismo DENTRO de una funcion, que es donde se colaba callado.
        if *self.peek() == Token::Hash {
            return Err(CError::new(
                self.line(),
                "aqui no hay preprocesador todavia: '#define', '#include' y '#ifdef' no se procesan. Usa 'const int' o 'enum' para las constantes",
            ));
        }
        match self.peek() {
            Token::If => self.parse_if(),
            Token::While => self.parse_while(),
            Token::Do => self.parse_do(),
            Token::For => self.parse_for(),
            Token::Switch => self.parse_switch(),
            Token::Break => { self.advance(); self.skip_semicolon(); Ok(Stmt::Break) }
            Token::Continue => { self.advance(); self.skip_semicolon(); Ok(Stmt::Continue) }
            Token::Return => self.parse_return(),
            Token::OpenBrace => self.parse_block(),
            Token::Goto => {
                self.advance();
                let label = match self.advance() {
                    Token::Ident(s) => s,
                    t => return Err(CError::new(self.line(),format!("expected label name, got {:?}", t))),
                };
                self.skip_semicolon();
                Ok(Stmt::Goto(label))
            }
            Token::Semicolon => { self.advance(); Ok(Stmt::Block(vec![])) }
            _ => {
                // Try to parse as declaration if it starts with a type keyword
                if self.peek_is_type_start() {
                    if let Some((typ, name)) = self.try_parse_decl()? {
                        return self.terminar_declaracion(typ, name);
                    }
                }
                self.parse_expr_stmt()
            }
        }
    }

    fn parse_if(&mut self) -> Result<Stmt, CError> {
        self.advance();
        self.expect(&Token::OpenParen)?;
        let cond = self.parse_expr()?;
        self.expect(&Token::CloseParen)?;
        let then = Box::new(self.parse_stmt()?);
        let else_ = if *self.peek() == Token::Else { self.advance(); Some(Box::new(self.parse_stmt()?)) } else { None };
        Ok(Stmt::If(cond, then, else_))
    }

    fn parse_while(&mut self) -> Result<Stmt, CError> {
        self.advance();
        self.expect(&Token::OpenParen)?;
        let cond = self.parse_expr()?;
        self.expect(&Token::CloseParen)?;
        let body = Box::new(self.parse_stmt()?);
        Ok(Stmt::While(cond, body))
    }

    fn parse_do(&mut self) -> Result<Stmt, CError> {
        self.advance();
        let body = Box::new(self.parse_stmt()?);
        self.expect(&Token::While)?;
        self.expect(&Token::OpenParen)?;
        let cond = self.parse_expr()?;
        self.expect(&Token::CloseParen)?;
        self.skip_semicolon();
        Ok(Stmt::DoWhile(body, cond))
    }

    fn parse_for(&mut self) -> Result<Stmt, CError> {
        self.advance();
        self.expect(&Token::OpenParen)?;
        // check for declaration: for(int i = 0; ...)
        let has_decl = match self.peek() {
            Token::Int | Token::Char | Token::Short | Token::Long |
            Token::Void | Token::Unsigned | Token::Signed | Token::Float | Token::Double |
            Token::Struct | Token::Union | Token::Const | Token::Volatile => true,
            _ => false,
        };
        if has_decl {
            let save = self.pos;
            self.strip_qualifiers();
            let _typ = match self.parse_type_spec() {
                Ok(t) => t,
                Err(_) => { self.pos = save; return self.parse_for_expr(); }
            };
            let name = match self.advance() {
                Token::Ident(n) => n,
                _ => { self.pos = save; return self.parse_for_expr(); }
            };
            let init = if *self.peek() == Token::Assign { self.advance(); Some(self.parse_expr()?) } else { None };
            self.skip_semicolon();
            self.var_types.insert(name.clone(), _typ.clone());
            // wrap in Block: { type name = init; for(; cond; inc) body }
            let mut stmts = Vec::new();
            stmts.push(Stmt::DeclAssign(_typ, name, init));
            let cond = if *self.peek() == Token::Semicolon { None } else { Some(self.parse_expr()?) };
            self.skip_semicolon();
            let inc = if *self.peek() == Token::CloseParen { None } else { Some(self.parse_expr()?) };
            self.expect(&Token::CloseParen)?;
            let body = self.parse_stmt()?;
            stmts.push(Stmt::For(None, cond, inc, Box::new(body)));
            return Ok(Stmt::Block(stmts));
        }
        self.parse_for_expr()
    }

    fn parse_for_expr(&mut self) -> Result<Stmt, CError> {
        let init = if *self.peek() == Token::Semicolon { None } else { Some(self.parse_expr()?) };
        self.skip_semicolon();
        let cond = if *self.peek() == Token::Semicolon { None } else { Some(self.parse_expr()?) };
        self.skip_semicolon();
        let inc = if *self.peek() == Token::CloseParen { None } else { Some(self.parse_expr()?) };
        self.expect(&Token::CloseParen)?;
        let body = Box::new(self.parse_stmt()?);
        Ok(Stmt::For(init, cond, inc, body))
    }

    fn parse_switch(&mut self) -> Result<Stmt, CError> {
        self.advance();
        self.expect(&Token::OpenParen)?;
        let expr = self.parse_expr()?;
        self.expect(&Token::CloseParen)?;
        self.expect(&Token::OpenBrace)?;
        let mut cases = Vec::new();
        let mut current = Vec::new();
        let mut current_val = None;
        loop {
            match self.peek() {
                Token::Case => {
                    if !current.is_empty() { cases.push(Case { value: current_val, stmts: std::mem::take(&mut current) }); }
                    self.advance();
                    let val = match self.advance() {
                        Token::IntLit(n) => n,
                        // Una constante de enum es una constante entera, y
                        // el estándar la admite como etiqueta de `case`.
                        // Es el uso más natural de un enum: `switch (fase)`.
                        Token::Ident(name) if self.enum_constants.contains_key(&name) => {
                            self.enum_constants[&name]
                        }
                        Token::CharLit(c) => c as i64,
                        t => return Err(CError::new(self.line(),format!("expected int in case, got {:?}", t))),
                    };
                    current_val = Some(val);
                    self.expect(&Token::Colon)?;
                }
                Token::Default => {
                    if !current.is_empty() { cases.push(Case { value: current_val, stmts: std::mem::take(&mut current) }); }
                    self.advance();
                    current_val = None;
                    self.expect(&Token::Colon)?;
                }
                Token::CloseBrace => { self.advance(); break; }
                Token::Eof => return Err(CError::new(self.line(),"unexpected eof in switch")),
                _ => { current.push(self.parse_stmt()?); }
            }
        }
        if !current.is_empty() { cases.push(Case { value: current_val, stmts: current }); }
        Ok(Stmt::Switch(expr, cases))
    }

    fn parse_return(&mut self) -> Result<Stmt, CError> {
        self.advance();
        if *self.peek() == Token::Semicolon { self.advance(); Ok(Stmt::Return(None)) }
        else { let e = self.parse_expr()?; self.skip_semicolon(); Ok(Stmt::Return(Some(e))) }
    }

    fn parse_block(&mut self) -> Result<Stmt, CError> {
        self.advance();
        let mut stmts = Vec::new();
        loop {
            match self.peek() {
                Token::CloseBrace => { self.advance(); break; }
                Token::Eof => return Err(CError::new(self.line(),"unexpected eof in block")),
                // ★ La directiva se caza AQUI, antes que nada. Es donde se
                // colaba: `try_parse_decl` miraba el `#`, decia "esto no es una
                // declaracion" y devolvia None sin consumirlo, y el bucle
                // seguia adelante — asi que un `#define X 5` dentro de una
                // funcion compilaba y se ignoraba EN SILENCIO. El programa
                // corria con la X sin sustituir y nadie decia nada.
                Token::Hash => {
                    return Err(CError::new(
                        self.line(),
                        "aqui no hay preprocesador todavia: '#define', '#include' y '#ifdef' \
                         no se procesan. Usa 'const int' o 'enum' para las constantes",
                    ))
                }
                _ => {
                    // check for label: ident followed by colon
                    if let Token::Ident(name) = self.peek().clone() {
                        if self.pos + 1 < self.tokens.len() && self.tokens[self.pos + 1] == Token::Colon {
                            self.advance(); // consume ident
                            self.advance(); // consume colon
                            stmts.push(Stmt::Label(name));
                            continue;
                        }
                    }
                    if let Some((typ, name)) = self.try_parse_decl()? {
                        stmts.push(self.terminar_declaracion(typ, name)?);
                        continue;
                    } else {
                        stmts.push(self.parse_stmt()?);
                    }
                }
            }
        }
        Ok(Stmt::Block(stmts))
    }

    fn parse_expr_stmt(&mut self) -> Result<Stmt, CError> {
        let expr = self.parse_expr()?;
        self.skip_semicolon();
        match &expr {
            // Atajo para `printf("literal")` SIN argumentos variádicos: baja
            // directo a la puerta de consola, sin runtime ni imports.
            //
            // El `args.len() == 1` es la condición que faltaba: antes
            // `printf("%d\n", x)` también entraba aquí y los argumentos se
            // DESCARTABAN en silencio — el programa imprimía literalmente
            // "%d". Con más de un argumento debe seguir por la ruta
            // variádica, que sí los formatea.
            Expr::Call(name, args) if name == "printf" && args.len() == 1 => {
                if let Some(Expr::StringLit(s)) = args.first() {
                    return Ok(if s.ends_with('\n') { let mut t = s.clone(); t.pop(); Stmt::PrintfLn(t) } else { Stmt::Printf(s.clone()) });
                }
            }
            _ => {}
        }
        Ok(Stmt::Expr(expr))
    }

    // ---- Expressions (precedence climbing) ----
    fn parse_expr(&mut self) -> Result<Expr, CError> {
        self.parse_comma()
    }

    fn parse_comma(&mut self) -> Result<Expr, CError> {
        let mut exprs = vec![self.parse_assign()?];
        while *self.peek() == Token::Comma { self.advance(); exprs.push(self.parse_assign()?); }
        if exprs.len() == 1 { Ok(exprs.into_iter().next().unwrap()) } else { Ok(Expr::Comma(exprs)) }
    }

    fn parse_assign(&mut self) -> Result<Expr, CError> {
        let expr = self.parse_conditional()?;
        let assign_op = |n: String, val: Expr, op: fn(Box<Expr>, Box<Expr>) -> Expr| {
            let n2 = n.clone(); Expr::Assign(n, Box::new(op(Box::new(Expr::Var(n2)), Box::new(val))))
        };
        let field_assign_op = |e: Expr, f: String, off: u32, ft: TypeSpec, val: Expr, op: fn(Box<Expr>, Box<Expr>) -> Expr| {
            let lhs = Expr::Field(Box::new(e.clone()), f.clone(), off, ft.clone());
            Expr::AssignField(Box::new(e), f, off, ft, Box::new(op(Box::new(lhs), Box::new(val))))
        };
        let arrow_assign_op = |e: Box<Expr>, f: String, off: u32, ft: TypeSpec, val: Expr, op: fn(Box<Expr>, Box<Expr>) -> Expr| {
            Expr::AssignArrow(e.clone(), f.clone(), off, ft.clone(), Box::new(op(Box::new(Expr::Arrow(e, f, off, ft)), Box::new(val))))
        };
        let sub_assign_op = |n: String, idx: Box<Expr>, sc: u8, val: Expr, op: fn(Box<Expr>, Box<Expr>) -> Expr| {
            let lhs = Expr::Subscript(n.clone(), idx.clone(), sc);
            Expr::AssignSubscript(n, idx, sc, Box::new(op(Box::new(lhs), Box::new(val))))
        };
        let idxptr_assign_op = |b: Box<Expr>, idx: Box<Expr>, ty: TypeSpec, val: Expr, op: fn(Box<Expr>, Box<Expr>) -> Expr| {
            let lhs = Expr::IndexPtr(b.clone(), idx.clone(), ty.clone());
            Expr::AssignIndexPtr(b, idx, ty, Box::new(op(Box::new(lhs), Box::new(val))))
        };
        match self.peek() {
            Token::Assign => { self.advance(); let val = self.parse_assign()?; match expr {
                Expr::Var(n) => Ok(Expr::Assign(n, Box::new(val))),
                Expr::Deref(a) => Ok(Expr::AssignDeref(a, Box::new(val))),
                Expr::Field(e, f, off, ft) => Ok(Expr::AssignField(e, f, off, ft, Box::new(val))),
                Expr::Arrow(e, f, off, ft) => Ok(Expr::AssignArrow(e, f, off, ft, Box::new(val))),
                Expr::Subscript(n, idx, sc) => Ok(Expr::AssignSubscript(n, idx, sc, Box::new(val))),
                Expr::IndexPtr(b, idx, ty) => Ok(Expr::AssignIndexPtr(b, idx, ty, Box::new(val))),
                _ => Ok(val),
            }}
            Token::AddAssign => { self.advance(); let val = self.parse_assign()?; match expr {
                Expr::Var(n) => Ok(assign_op(n, val, Expr::Add)),
                Expr::Field(e, f, off, ft) => Ok(field_assign_op(*e, f, off, ft, val, Expr::Add)),
                Expr::Arrow(e, f, off, ft) => Ok(arrow_assign_op(e, f, off, ft, val, Expr::Add)),
                Expr::Subscript(n, idx, sc) => Ok(sub_assign_op(n, idx, sc, val, Expr::Add)),
                Expr::IndexPtr(b, idx, ty) => Ok(idxptr_assign_op(b, idx, ty, val, Expr::Add)),
                _ => Ok(val),
            }}
            Token::SubAssign => { self.advance(); let val = self.parse_assign()?; match expr {
                Expr::Var(n) => Ok(assign_op(n, val, Expr::Sub)),
                Expr::Field(e, f, off, ft) => Ok(field_assign_op(*e, f, off, ft, val, Expr::Sub)),
                Expr::Arrow(e, f, off, ft) => Ok(arrow_assign_op(e, f, off, ft, val, Expr::Sub)),
                Expr::Subscript(n, idx, sc) => Ok(sub_assign_op(n, idx, sc, val, Expr::Sub)),
                Expr::IndexPtr(b, idx, ty) => Ok(idxptr_assign_op(b, idx, ty, val, Expr::Sub)),
                _ => Ok(val),
            }}
            Token::MulAssign => { self.advance(); let val = self.parse_assign()?; match expr {
                Expr::Var(n) => Ok(assign_op(n, val, Expr::Mul)),
                Expr::Field(e, f, off, ft) => Ok(field_assign_op(*e, f, off, ft, val, Expr::Mul)),
                Expr::Arrow(e, f, off, ft) => Ok(arrow_assign_op(e, f, off, ft, val, Expr::Mul)),
                Expr::Subscript(n, idx, sc) => Ok(sub_assign_op(n, idx, sc, val, Expr::Mul)),
                Expr::IndexPtr(b, idx, ty) => Ok(idxptr_assign_op(b, idx, ty, val, Expr::Mul)),
                _ => Ok(val),
            }}
            Token::DivAssign => { self.advance(); let val = self.parse_assign()?; match expr {
                Expr::Var(n) => Ok(assign_op(n, val, Expr::Div)),
                Expr::Field(e, f, off, ft) => Ok(field_assign_op(*e, f, off, ft, val, Expr::Div)),
                Expr::Arrow(e, f, off, ft) => Ok(arrow_assign_op(e, f, off, ft, val, Expr::Div)),
                Expr::Subscript(n, idx, sc) => Ok(sub_assign_op(n, idx, sc, val, Expr::Div)),
                Expr::IndexPtr(b, idx, ty) => Ok(idxptr_assign_op(b, idx, ty, val, Expr::Div)),
                _ => Ok(val),
            }}
            Token::ModAssign => { self.advance(); let val = self.parse_assign()?; match expr {
                Expr::Var(n) => Ok(assign_op(n, val, Expr::Mod)),
                Expr::Field(e, f, off, ft) => Ok(field_assign_op(*e, f, off, ft, val, Expr::Mod)),
                Expr::Arrow(e, f, off, ft) => Ok(arrow_assign_op(e, f, off, ft, val, Expr::Mod)),
                Expr::Subscript(n, idx, sc) => Ok(sub_assign_op(n, idx, sc, val, Expr::Mod)),
                Expr::IndexPtr(b, idx, ty) => Ok(idxptr_assign_op(b, idx, ty, val, Expr::Mod)),
                _ => Ok(val),
            }}
            Token::ShlAssign => { self.advance(); let val = self.parse_assign()?; match expr {
                Expr::Var(n) => Ok(assign_op(n, val, Expr::Shl)),
                Expr::Field(e, f, off, ft) => Ok(field_assign_op(*e, f, off, ft, val, Expr::Shl)),
                Expr::Arrow(e, f, off, ft) => Ok(arrow_assign_op(e, f, off, ft, val, Expr::Shl)),
                Expr::Subscript(n, idx, sc) => Ok(sub_assign_op(n, idx, sc, val, Expr::Shl)),
                Expr::IndexPtr(b, idx, ty) => Ok(idxptr_assign_op(b, idx, ty, val, Expr::Shl)),
                _ => Ok(val),
            }}
            Token::ShrAssign => { self.advance(); let val = self.parse_assign()?; match expr {
                Expr::Var(n) => Ok(assign_op(n, val, Expr::Shr)),
                Expr::Field(e, f, off, ft) => Ok(field_assign_op(*e, f, off, ft, val, Expr::Shr)),
                Expr::Arrow(e, f, off, ft) => Ok(arrow_assign_op(e, f, off, ft, val, Expr::Shr)),
                Expr::Subscript(n, idx, sc) => Ok(sub_assign_op(n, idx, sc, val, Expr::Shr)),
                Expr::IndexPtr(b, idx, ty) => Ok(idxptr_assign_op(b, idx, ty, val, Expr::Shr)),
                _ => Ok(val),
            }}
            Token::AndAssign => { self.advance(); let val = self.parse_assign()?; match expr {
                Expr::Var(n) => Ok(assign_op(n, val, Expr::BitAnd)),
                Expr::Field(e, f, off, ft) => Ok(field_assign_op(*e, f, off, ft, val, Expr::BitAnd)),
                Expr::Arrow(e, f, off, ft) => Ok(arrow_assign_op(e, f, off, ft, val, Expr::BitAnd)),
                Expr::Subscript(n, idx, sc) => Ok(sub_assign_op(n, idx, sc, val, Expr::BitAnd)),
                Expr::IndexPtr(b, idx, ty) => Ok(idxptr_assign_op(b, idx, ty, val, Expr::BitAnd)),
                _ => Ok(val),
            }}
            Token::XorAssign => { self.advance(); let val = self.parse_assign()?; match expr {
                Expr::Var(n) => Ok(assign_op(n, val, Expr::BitXor)),
                Expr::Field(e, f, off, ft) => Ok(field_assign_op(*e, f, off, ft, val, Expr::BitXor)),
                Expr::Arrow(e, f, off, ft) => Ok(arrow_assign_op(e, f, off, ft, val, Expr::BitXor)),
                Expr::Subscript(n, idx, sc) => Ok(sub_assign_op(n, idx, sc, val, Expr::BitXor)),
                Expr::IndexPtr(b, idx, ty) => Ok(idxptr_assign_op(b, idx, ty, val, Expr::BitXor)),
                _ => Ok(val),
            }}
            Token::OrAssign => { self.advance(); let val = self.parse_assign()?; match expr {
                Expr::Var(n) => Ok(assign_op(n, val, Expr::BitOr)),
                Expr::Field(e, f, off, ft) => Ok(field_assign_op(*e, f, off, ft, val, Expr::BitOr)),
                Expr::Arrow(e, f, off, ft) => Ok(arrow_assign_op(e, f, off, ft, val, Expr::BitOr)),
                Expr::Subscript(n, idx, sc) => Ok(sub_assign_op(n, idx, sc, val, Expr::BitOr)),
                Expr::IndexPtr(b, idx, ty) => Ok(idxptr_assign_op(b, idx, ty, val, Expr::BitOr)),
                _ => Ok(val),
            }}
            _ => Ok(expr),
        }
    }

    fn parse_conditional(&mut self) -> Result<Expr, CError> {
        let mut expr = self.parse_lor()?;
        if *self.peek() == Token::Question {
            self.advance();
            let t = self.parse_expr()?;
            self.expect(&Token::Colon)?;
            let f = self.parse_conditional()?;
            expr = Expr::Conditional(Box::new(expr), Box::new(t), Box::new(f));
        }
        Ok(expr)
    }

    fn parse_lor(&mut self) -> Result<Expr, CError> { let mut l = self.parse_land()?; while *self.peek() == Token::LOr { self.advance(); let r = self.parse_land()?; l = Expr::LOr(Box::new(l), Box::new(r)); } Ok(l) }
    fn parse_land(&mut self) -> Result<Expr, CError> { let mut l = self.parse_bitor()?; while *self.peek() == Token::LAnd { self.advance(); let r = self.parse_bitor()?; l = Expr::LAnd(Box::new(l), Box::new(r)); } Ok(l) }
    fn parse_bitor(&mut self) -> Result<Expr, CError> { let mut l = self.parse_bitxor()?; while *self.peek() == Token::Or { self.advance(); let r = self.parse_bitxor()?; l = Expr::BitOr(Box::new(l), Box::new(r)); } Ok(l) }
    fn parse_bitxor(&mut self) -> Result<Expr, CError> { let mut l = self.parse_bitand()?; while *self.peek() == Token::Xor { self.advance(); let r = self.parse_bitand()?; l = Expr::BitXor(Box::new(l), Box::new(r)); } Ok(l) }
    fn parse_bitand(&mut self) -> Result<Expr, CError> { let mut l = self.parse_equality()?; while *self.peek() == Token::And { self.advance(); let r = self.parse_equality()?; l = Expr::BitAnd(Box::new(l), Box::new(r)); } Ok(l) }

    fn parse_equality(&mut self) -> Result<Expr, CError> {
        let mut l = self.parse_relational()?;
        loop {
            match self.peek() {
                Token::EqEq => { self.advance(); let r = self.parse_relational()?; l = Expr::Eq(Box::new(l), Box::new(r)); }
                Token::Neq => { self.advance(); let r = self.parse_relational()?; l = Expr::Neq(Box::new(l), Box::new(r)); }
                _ => break,
            }
        }
        Ok(l)
    }

    fn parse_relational(&mut self) -> Result<Expr, CError> {
        let mut l = self.parse_shift()?;
        loop {
            match self.peek() {
                Token::Lt => { self.advance(); let r = self.parse_shift()?; l = Expr::Lt(Box::new(l), Box::new(r)); }
                Token::Gt => { self.advance(); let r = self.parse_shift()?; l = Expr::Gt(Box::new(l), Box::new(r)); }
                Token::Le => { self.advance(); let r = self.parse_shift()?; l = Expr::Le(Box::new(l), Box::new(r)); }
                Token::Ge => { self.advance(); let r = self.parse_shift()?; l = Expr::Ge(Box::new(l), Box::new(r)); }
                _ => break,
            }
        }
        Ok(l)
    }

    fn parse_shift(&mut self) -> Result<Expr, CError> {
        let mut l = self.parse_add()?;
        loop {
            match self.peek() {
                Token::Shl => { self.advance(); let r = self.parse_add()?; l = Expr::Shl(Box::new(l), Box::new(r)); }
                Token::Shr => { self.advance(); let r = self.parse_add()?; l = Expr::Shr(Box::new(l), Box::new(r)); }
                _ => break,
            }
        }
        Ok(l)
    }

    fn parse_add(&mut self) -> Result<Expr, CError> {
        let mut l = self.parse_mul()?;
        loop {
            match self.peek() {
                Token::Plus => { self.advance(); let r = self.parse_mul()?; l = Expr::Add(Box::new(l), Box::new(r)); }
                Token::Minus => { self.advance(); let r = self.parse_mul()?; l = Expr::Sub(Box::new(l), Box::new(r)); }
                _ => break,
            }
        }
        Ok(l)
    }

    fn parse_mul(&mut self) -> Result<Expr, CError> {
        let mut l = self.parse_unary()?;
        loop {
            match self.peek() {
                Token::Star => { self.advance(); let r = self.parse_unary()?; l = Expr::Mul(Box::new(l), Box::new(r)); }
                Token::Slash => { self.advance(); let r = self.parse_unary()?; l = Expr::Div(Box::new(l), Box::new(r)); }
                Token::Percent => { self.advance(); let r = self.parse_unary()?; l = Expr::Mod(Box::new(l), Box::new(r)); }
                _ => break,
            }
        }
        Ok(l)
    }

    fn parse_unary(&mut self) -> Result<Expr, CError> {
        match self.peek() {
            Token::Minus => { self.advance(); let e = self.parse_unary()?; Ok(Expr::Neg(Box::new(e))) }
            Token::Not => { self.advance(); let e = self.parse_unary()?; Ok(Expr::Not(Box::new(e))) }
            Token::Tilde => { self.advance(); let e = self.parse_unary()?; Ok(Expr::BitNot(Box::new(e))) }
            Token::PlusPlus => { self.advance(); match &self.peek() { Token::Ident(n) => { let name = n.clone(); self.advance(); Ok(Expr::PreInc(name)) } _ => Err(CError::new(self.line(),"expected variable after ++")) } }
            Token::MinusMinus => { self.advance(); match &self.peek() { Token::Ident(n) => { let name = n.clone(); self.advance(); Ok(Expr::PreDec(name)) } _ => Err(CError::new(self.line(),"expected variable after --")) } }
            Token::And => { self.advance(); let expr = self.parse_unary()?; Ok(Expr::AddrOf(Box::new(expr))) }
            Token::Star => { self.advance(); let e = self.parse_unary()?; Ok(Expr::Deref(Box::new(e))) }
            Token::Sizeof => { self.advance(); self.expect(&Token::OpenParen)?; let t = self.parse_type_spec()?; self.expect(&Token::CloseParen)?; Ok(Expr::Int(t.stack_size() as i64)) }
            Token::OpenParen => {
                let save = self.pos;
                self.advance();
                // Try to parse as cast: (type)expr
                let is_cast = self.peek_is_type_start();
                if is_cast {
                    if let Ok(typ) = self.parse_type_spec() {
                        if *self.peek() == Token::CloseParen {
                            self.advance();
                            let expr = self.parse_unary()?;
                            // cast REAL: codegen trunca/extiende al tamaño del tipo
                            return Ok(Expr::Cast(typ, Box::new(expr)));
                        }
                    }
                }
                self.pos = save;
                self.parse_postfix()
            }
            _ => self.parse_postfix(),
        }
    }

    fn parse_postfix(&mut self) -> Result<Expr, CError> {
        let mut expr = self.parse_primary()?;
        loop {
            match self.peek() {
                Token::PlusPlus => { self.advance(); match expr { Expr::Var(ref n) => expr = Expr::PostInc(n.clone()), _ => {} } }
                Token::MinusMinus => { self.advance(); match expr { Expr::Var(ref n) => expr = Expr::PostDec(n.clone()), _ => {} } }
                Token::OpenParen => {
                    // (*fp)(args) — llamada a través de un puntero CALCULADO.
                    // (fp(args) con fp variable ya lo maneja parse_primary.)
                    self.advance();
                    let mut args = Vec::new();
                    while *self.peek() != Token::CloseParen && *self.peek() != Token::Eof {
                        args.push(self.parse_assign()?);
                        if *self.peek() == Token::Comma { self.advance(); }
                    }
                    self.expect(&Token::CloseParen)?;
                    expr = Expr::CallPtr(Box::new(expr), args);
                }
                Token::OpenBracket => {
                    self.advance();
                    let index = self.parse_expr()?;
                    self.expect(&Token::CloseBracket)?;
                    match &expr {
                        Expr::Var(n) => {
                            let scale = self.element_size(n);
                            let n2 = n.clone();
                            expr = Expr::Subscript(n2, Box::new(index), scale);
                        }
                        // base compuesta (p->arr[i], (a+1)[i]): el elemento sale
                        // del tipo de la base. Antes se rechazaba en seco.
                        _ => {
                            let elem = self.resolve_expr_type(&expr)
                                .and_then(|t| match t {
                                    TypeSpec::Ptr(inner) | TypeSpec::Array(inner, _) => Some(*inner),
                                    _ => None,
                                })
                                .unwrap_or(TypeSpec::Long);
                            expr = Expr::IndexPtr(Box::new(expr), Box::new(index), elem);
                        }
                    }
                }
                Token::Dot => {
                    self.advance();
                    let field = match self.advance() {
                        Token::Ident(s) => s,
                        t => return Err(CError::new(self.line(),format!("expected field name, got {:?}", t))),
                    };
                    let offset = self.resolve_field_expr_offset(&expr, &field);
                    let ftyp = self.field_type_via_value(&expr, &field);
                    expr = Expr::Field(Box::new(expr), field, offset, ftyp);
                }
                Token::Arrow => {
                    self.advance();
                    let field = match self.advance() {
                        Token::Ident(s) => s,
                        t => return Err(CError::new(self.line(),format!("expected field name, got {:?}", t))),
                    };
                    let offset = self.resolve_arrow_expr_offset(&expr, &field);
                    let ftyp = self.field_type_via_pointer(&expr, &field);
                    expr = Expr::Arrow(Box::new(expr), field, offset, ftyp);
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, CError> {
        let tok_line = self.line(); // línea del token que vamos a consumir
        let tok = self.advance();
        match tok {
            Token::IntLit(n) => Ok(Expr::Int(n)),
            Token::FloatLit(f) => Ok(Expr::FloatLit(f)),
            Token::StringLit(s) => Ok(Expr::StringLit(s)),
            Token::CharLit(c) => Ok(Expr::CharLit(c)),
            Token::Ident(name) => {
                if *self.peek() == Token::OpenParen {
                    self.advance();
                    let mut args = Vec::new();
                    while *self.peek() != Token::CloseParen && *self.peek() != Token::Eof {
                        // Use parse_assign (not parse_expr) to avoid the comma operator
                        // consuming argument separators â€” C grammar requires
                        // argument_expression_list: assignment_expression (',' assignment_expression)*
                        args.push(self.parse_assign()?);
                        if *self.peek() == Token::Comma { self.advance(); }
                    }
                    self.expect(&Token::CloseParen)?;
                    // Check if this function name matches a known syscall definition
                    if let Some(def) = self.syscalls.get(&name).cloned() {
                        if args.len() != def.arg_count as usize {
                            return Err(CError::new(self.line(),format!(
                                "syscall {}() expects {} arguments, got {}",
                                def.name, def.arg_count, args.len()
                            )));
                        }
                        Ok(Expr::Syscall(def, args))
                    } else if let Some(stripped) = name.strip_prefix("__") {
                        // FUSIÓN sem-asm↔C: __hlt(), __outb(p,v), __rdtsc()... =
                        // instrucción de la tabla como función. El namespace __
                        // es reservado a la implementación — aquí ES la
                        // implementación. La aridad la valida el codegen contra
                        // la tabla (donde vive la verdad de cada intrínseco).
                        Ok(Expr::Intrinsic(stripped.to_string(), args))
                    } else {
                        Ok(Expr::Call(name, args))
                    }
                } else if let Some(&value) = self.enum_constants.get(&name) {
                    // Una constante de enum ES su valor, no una variable: no
                    // tiene direccion ni hueco en la pila.
                    Ok(Expr::Int(value))
                } else if let Some(real) = self.static_alias.get(&name) {
                    // ★ El ÚNICO sitio donde un identificador se vuelve
                    // variable, y por eso el único que hace falta tocar para
                    // que las `static` locales funcionen. Si hubiera dos
                    // caminos, uno se quedaría sin traducir y el bug sería
                    // "a veces la static es la global de otro".
                    Ok(Expr::Var(real.clone()))
                } else {
                    Ok(Expr::Var(name))
                }
            }
            Token::OpenParen => {
                let expr = self.parse_expr()?;
                self.expect(&Token::CloseParen)?;
                Ok(expr)
            }
            t => Err(CError::new(tok_line, format!("unexpected token: {:?}", t))),
        }
    }
}
