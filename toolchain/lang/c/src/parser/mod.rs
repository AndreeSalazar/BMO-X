//! C Parser -- tokens a AST (gramatica completa) + preprocesador.

pub mod preprocessor;
/// Las listas `{ ... }`, en su propio fichero. Ver su cabecera para el porque del
/// reparto y para que hicieron GCC, Clang, chibicc, TCC y MSVC con esto mismo.
mod inicializador;

/// TYPE AND LAYOUT RESOLUTION: what a name refers to and where in a struct it
/// lives. A type checker's job living inside a parser, because `Expr::Field`
/// carries its byte offset.
mod types;
/// THE SYSCALL PASS: validate then resolve, walking an already-built tree. Not
/// grammar -- a check against the frozen kernel surface.
mod syscalls;
/// DECLARATIONS AND DECLARATORS: the hard half. A C type is built inside-out
/// around its name, so every wrap needs its own method.
mod declarations;
/// STATEMENTS: nine regular forms. The smallest half of the grammar.
mod statements;
/// EXPRESSIONS: the precedence ladder. Seventeen methods that are ONE
/// algorithm -- the call chain IS C's precedence table.
mod expressions;

use std::collections::HashMap;
use std::path::PathBuf;
use crate::ast::*;
use crate::lexer::Token;
use crate::module;
use crate::{CError, StandardFeatures};

/// Lo que puede haber en el nivel de fichero cuando algo empieza como una
/// funcion.
///
/// Eran dos casos (`Some`/`None`) y hacen falta **tres**: un prototipo no es
/// una funcion --no tiene cuerpo que emitir-- pero tampoco es "esto no era una
/// funcion", porque sus tokens ya se han consumido. Devolver `None` ahi hacia
/// que el llamante rebobinara y lo intentara leer como una variable global,
/// y el error que salia acusaba de lo que no era.
enum Tope {
    Funcion(Function),
    /// `int f(int);` -- declarada, no definida. Consumida.
    Prototipo,
    /// No era una funcion. Los tokens estan rebobinados.
    NoEsFuncion,
}

pub(crate) struct Parser {
    tokens: Vec<Token>,
    token_lines: Vec<usize>,
    pos: usize,
    var_types: HashMap<String, TypeSpec>,
    struct_fields: HashMap<String, Vec<(String, u32, u32)>>,
    struct_sizes: HashMap<String, u32>,
    /// The alignment of each aggregate. See `type_align`: it does not follow
    /// from the size, so it has to be remembered when the aggregate is laid
    /// out.
    struct_aligns: HashMap<String, u32>,
    // (struct_name, field_name) -> tipo del campo. Necesario para resolver
    // offsets de accesos anidados (a->b->c, a.b.c) sin adivinar.
    field_types: HashMap<(String, String), TypeSpec>,
    usings: Vec<String>,
    typedefs: HashMap<String, TypeSpec>,
    /// Constantes de `enum` con su VALOR.
    ///
    /// Antes el parser calculaba el valor y lo tiraba: registraba el nombre
    /// como si fuera una variable `int` y nunca guardaba a que equivalia,
    /// asi que `enum { ROJO, VERDE }` dejaba `VERDE` como una variable sin
    /// definir. El aviso `value assigned to val is never read` del propio
    /// compilador estaba senalando justo este bug.
    enum_constants: HashMap<String, i64>,
    /// * Locales `static`: nombre visible -> nombre real de la global.
    ///
    /// Una `static` dentro de una funcion **no es una local**: sobrevive entre
    /// llamadas, asi que vive donde viven las globales. Pero su NOMBRE solo se
    /// ve dentro de su funcion, y dos funciones pueden tener cada una su
    /// `static int n`. Se resuelve renombrando en la declaracion y traduciendo
    /// en el unico sitio donde un identificador se vuelve variable.
    ///
    /// Se vacia al empezar cada funcion: ese ambito es justo lo que este mapa
    /// representa.
    static_alias: HashMap<String, String>,
    /// Tipo base del ultimo declarador parseado, para los que vengan detras de
    /// una coma. Ver `declaradores_tras_coma`.
    base_del_declarador: TypeSpec,
    /// Globales que una funcion ha ido creando al declarar sus `static`.
    /// `parse_program` las recoge al terminar cada funcion.
    globales_pendientes: Vec<GlobalDecl>,
    /// How many untagged struct/union bodies have been seen. Only used to make
    /// their generated tags unique.
    anon_aggregates: u32,
    /// La funcion cuyo cuerpo se esta leyendo. La necesita una `static` local
    /// para su nombre global, y sin ella los bloques ANIDADOS no podian tener
    /// una -- ver `parse_block`.
    funcion_actual: String,
    syscalls: HashMap<String, SyscallDef>,
    /// Lo que el LEXER no pudo leer. Se comprueba antes de parsear: seguir
    /// con un token inventado produce un programa que compila y no dice lo
    /// que esta escrito.
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
            struct_aligns: HashMap::new(),
            field_types: HashMap::new(),
            usings: Vec::new(),
            typedefs: HashMap::new(),
            enum_constants: HashMap::new(),
            static_alias: HashMap::new(),
            base_del_declarador: TypeSpec::Int,
            globales_pendientes: Vec::new(),
            anon_aggregates: 0,
            funcion_actual: String::new(),
            syscalls: HashMap::new(),
            features: StandardFeatures::default(),
        }
    }

    #[cfg(test)]
    pub(crate) fn tokenize_for_test(source: &str) -> Vec<Token> { crate::lexer::tokenize(source).0 }


    fn peek(&self) -> &Token { &self.tokens[self.pos] }
    fn advance(&mut self) -> Token { let t = self.tokens[self.pos].clone(); self.pos += 1; t }

    /// Linea del token actual (para errores con ubicacion REAL, no "linea 1").
    fn line(&self) -> usize {
        self.token_lines.get(self.pos.min(self.token_lines.len().saturating_sub(1)))
            .copied().unwrap_or(1)
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
            // An untagged aggregate is defined wherever its TYPE is written --
            // inside a typedef, a parameter list, a local. `parse_type_spec`
            // cannot reach `globals` from there, so it leaves the definition
            // here and this is where it is collected, once per construct.
            globals.append(&mut self.globales_pendientes);
        // * Una directiva del preprocesador. No hay preprocesador, y hasta
        // ahora eso no se decia: el `#` se lo tragaba el lexer, asi que un
        // `#define X 5` dentro de una funcion compilaba **y se ignoraba en
        // silencio** -- el programa corria con X sin sustituir. Un no-op
        // callado es peor que un "no".
        if *self.peek() == Token::Hash {
            return Err(CError::new(
                self.line(),
                "aqui no hay preprocesador todavia: '#define', '#include' y '#ifdef' no se procesan. Usa 'const int' o 'enum' para las constantes",
            ));
        }
            // * TAGGED aggregates only. The untagged ones fall through.
            //
            // `struct P { ... };` has its own path here because a bare
            // definition declares nothing and the generic declarator path
            // would demand a name after it. `typedef struct { ... } P;` and
            // `struct { ... } g;` have no tag, and they are handled where the
            // TYPE is parsed (`parse_type_spec`) -- one place that knows the
            // whole shape, instead of two that have to agree.
            if (*self.peek() == Token::Struct || *self.peek() == Token::Union)
                && matches!(self.tokens.get(self.pos + 1), Some(Token::Ident(_)))
            {
                let is_union = *self.peek() == Token::Union;
                self.advance();
                let name = match self.advance() {
                    Token::Ident(n) => n,
                    t => return Err(CError::new(self.line(),format!("expected struct name, got {:?}", t))),
                };
                if *self.peek() == Token::OpenBrace {
                    let members = self.parse_aggregate_body()?;
                    self.skip_semicolon();
                    if is_union {
                        self.compute_union_layout(&name, &members);
                        globals.push(GlobalDecl::Union(name, members));
                    } else {
                        self.compute_struct_layout(&name, &members);
                        globals.push(GlobalDecl::Struct(name, members));
                    }
                } else {
                    // `struct P name` -- o una variable global, o una funcion
                    // que DEVUELVE el struct.
                    if let Token::Ident(vname) = self.advance() {
                        // * Devolver un agregado por valor es un mecanismo
                        // aparte (puntero oculto) y todavia no esta. Decirlo
                        // AQUI y con el nombre delante: si se deja caer, el
                        // parser encuentra el `(` donde esperaba un `;` y suelta
                        // "expected type, got OpenParen", que manda a mirar el
                        // tipo -- y el tipo esta perfectamente.
                        if *self.peek() == Token::OpenParen {
                            return Err(CError::new(
                                self.line(),
                                format!(
                                    "'{vname}' devuelve un struct por valor, y eso aun no se \
                                     compila: pasa un puntero al destino como parametro"
                                ),
                            ));
                        }
                        let mut typ = if is_union { TypeSpec::UnionRef(name) } else { TypeSpec::StructRef(name) };
                        // * `struct P tabla[N]` -- el declarador de array se
                        // IGNORABA en esta rama, asi que una tabla de N structs
                        // se declaraba como UNO SOLO y el parser reventaba al
                        // encontrarse el `[` suelto donde esperaba un tipo
                        // ("expected type, got OpenBracket").
                        //
                        // `parse_type_and_name` si lo hacia; esta rama es un
                        // camino aparte para `struct`, y se quedo sin el.
                        if *self.peek() == Token::OpenBracket {
                            self.advance();
                            let size_expr = self.parse_expr()?;
                            self.expect(&Token::CloseBracket)?;
                            let n = match size_expr {
                                Expr::Int(k) if k > 0 => k as u32,
                                _ => 1,
                            };
                            typ = TypeSpec::Array(Box::new(typ), n);
                        }
                        // Y su lista: `struct estado estados[2] = {{4,1},{8,0}}`,
                        // que es LA forma de las tablas de DOOM.
                        if *self.peek() == Token::Assign
                            && self.tokens.get(self.pos + 1) == Some(&Token::OpenBrace)
                        {
                            self.advance(); // el `=`
                            let escrituras = self.parse_inicializador(&typ)?;
                            self.skip_semicolon();
                            globals.push(GlobalDecl::VarLista(
                                typ.clone(),
                                vname.clone(),
                                escrituras,
                            ));
                        } else {
                            self.skip_semicolon();
                            globals.push(GlobalDecl::Var(typ.clone(), vname.clone(), None));
                        }
                        self.var_types.insert(vname, typ);
                    }
                }
                continue;
            }
            if *self.peek() == Token::Enum {
                self.parse_enum_spec()?;
                // * `enum { ... } main_e;` -- el enum DECLARA una variable.
                //
                // Es una definicion y un declarador en la misma frase, y
                // `m_menu.c` la usa para todos sus menus. Se leia el cuerpo, se
                // buscaba el `;` y **el nombre se quedaba suelto**: la vuelta
                // siguiente del bucle lo veia como el principio de otra
                // declaracion y contestaba "expected type, got Ident(main_e)",
                // que manda a buscar un typedef que nunca existio.
                //
                // El tipo es `int`, que es lo que un enum es aqui.
                if let Token::Ident(vname) = self.peek().clone() {
                    self.advance();
                    let mut typ = TypeSpec::Int;
                    if *self.peek() == Token::OpenBracket {
                        typ = self.parse_array_suffix(typ)?;
                    }
                    self.var_types.insert(vname.clone(), typ.clone());
                    globals.push(GlobalDecl::Var(typ, vname, None));
                    let base = TypeSpec::Int;
                    self.declaradores_globales_tras_coma(&base, &mut globals)?;
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
                // A prototype is also `extern`: `extern int f(int);`. It
                // declares nothing to allocate, and reading it as a variable
                // would stop at the parenthesis.
                match self.try_parse_function()? {
                    Tope::Prototipo => continue,
                    Tope::Funcion(f) => {
                        functions.push(f);
                        globals.append(&mut self.globales_pendientes);
                        continue;
                    }
                    Tope::NoEsFuncion => {}
                }
                let (typ, name) = self.parse_type_and_name()?;
                // Same comma rule as any other declaration. A header full of
                // `extern int a, b;` is the normal way C shares globals, and
                // this branch was the one place that did not know it.
                let base = self.base_del_declarador.clone();
                self.declaradores_globales_tras_coma(&base, &mut globals)?;
                self.skip_semicolon();
                self.var_types.insert(name.clone(), typ.clone());
                globals.push(GlobalDecl::Var(typ, name, None));
                continue;
            }
            // * `static` EN EL NIVEL DE FICHERO: se acepta y se sigue de largo.
            //
            // Ahi `static` significa "este nombre no sale de esta unidad de
            // traduccion" -- enlace interno. BMO C compila **una** unidad, asi
            // que no hay nadie de quien esconderse: una global static y una
            // global normal se comportan exactamente igual.
            //
            // No es tragarselo por comodidad: es que aqui no cambia lo que el
            // programa hace, y emitir algo distinto seria inventarse una
            // diferencia. El dia que haya compilacion separada, esta linea es
            // el sitio donde ponerle el enlace interno de verdad.
            //
            // Dentro de una funcion es OTRA COSA y si cambia el programa: ver
            // `terminar_declaracion_static`.
            if *self.peek() == Token::Static {
                self.advance();
                continue;
            }
            // `inline` at file scope, same treatment and for the same reason as
            // `static` above: it changes nothing this compiler does. See
            // `strip_qualifiers`.
            if matches!(self.peek(), Token::Ident(n)
                if n == "inline" || n == "__inline" || n == "__inline__"
                    || n == "__forceinline")
            {
                self.advance();
                continue;
            }
            if *self.peek() == Token::Typedef {
                self.advance();
                let typ = self.parse_type_spec()?;
                // * `typedef void (*action_t)(void);`
                //
                // The pointer-to-function DECLARATOR was already understood
                // for variables and parameters (`parse_fnptr_tail`); only the
                // typedef could not use it, and the message said "expected
                // typedef name, got OpenParen" -- which points at the
                // parenthesis instead of at the missing feature.
                //
                // The type is `void*` like everywhere else here: calls through
                // it are indirect, so the signature buys nothing at this
                // altitude. DOOM's `d_think.h` is built out of these.
                if *self.peek() == Token::OpenParen
                    && self.tokens.get(self.pos + 1) == Some(&Token::Star)
                {
                    let (fname, ftyp) = self.parse_fnptr_tail()?;
                    self.skip_semicolon();
                    self.typedefs.insert(fname, ftyp);
                    continue;
                }
                let name = match self.advance() {
                    Token::Ident(n) => n,
                    t => return Err(CError::new(self.line(),format!("expected typedef name, got {:?}", t))),
                };
                // * `typedef byte sha1_digest_t[20];` -- a typedef OF AN ARRAY.
                //
                // The brackets belong to the declarator, exactly as they do in
                // a variable, and this was the only declarator that did not
                // read them. `sha1.h` has one, `net_defs.h` includes it, and
                // `doomstat.h` includes that -- which is how one line reached
                // most of the game.
                let typ = if *self.peek() == Token::OpenBracket {
                    self.parse_array_suffix(typ)?
                } else {
                    typ
                };
                self.skip_semicolon();
                self.typedefs.insert(name, typ);
                continue;
            }
            match self.try_parse_function()? {
                Tope::Funcion(f) => {
                    functions.push(f);
                    // Las `static` que esa funcion haya declarado ya no son
                    // suyas: son globales con nombre propio.
                    globals.append(&mut self.globales_pendientes);
                    continue;
                }
                // Un PROTOTIPO: `int f(int);`. No emite nada -- solo dice que
                // esa funcion existira. Ya se consumio, asi que se sigue.
                Tope::Prototipo => continue,
                Tope::NoEsFuncion => {}
            }
            {
                let (typ, name) = self.parse_type_and_name()?;
                // * `= { ... }` A NIVEL GLOBAL.
                //
                // Antes esto reventaba con `unexpected token: OpenBrace`:
                // `parse_assign` no empieza por `{`, y un global solo admitia
                // una expresion. Dentro de una funcion funcionaba desde
                // siempre, asi que **la diferencia era el ambito, no el
                // inicializador**.
                //
                // Importa porque es la forma de las TABLAS ESTATICAS, y un
                // programa grande de C es en buena parte tablas: el `info.c`
                // de DOOM son cuatro mil lineas de `{ ... }` a nivel global.
                //
                // Se reusa `parse_inicializador`, el mismo aplanador que usan
                // los locales, asi que los designadores (`{[2].y = 8}`) y el
                // relleno implicito valen aqui sin escribir nada nuevo.
                // The base type, kept before the declarator eats any `*`: in
                // `int *a, b;` the `b` is an `int`. Captured here because the
                // declarators after the comma need it below.
                let base = self.base_del_declarador.clone();
                if *self.peek() == Token::Assign
                    && self.tokens.get(self.pos + 1) == Some(&Token::OpenBrace)
                {
                    self.advance(); // el `=`
                    let escrituras = self.parse_inicializador(&typ)?;
                    // `int t[] = { 10, 20, 30 };` -- the list is what says the
                    // array is three long. Until now the bracket never got
                    // this far.
                    let typ = self.cerrar_array_incompleto(typ, &escrituras);
                    self.declaradores_globales_tras_coma(&base, &mut globals)?;
                    self.skip_semicolon();
                    self.var_types.insert(name.clone(), typ.clone());
                    globals.push(GlobalDecl::VarLista(typ, name, escrituras));
                    continue;
                }
                let init = if *self.peek() == Token::Assign {
                    self.advance();
                    Some(self.parse_assign()?)
                } else {
                    None
                };
                // * `int alpha, beta;` AT FILE SCOPE.
                //
                // It worked inside a function and not outside it -- the local
                // path already called `declaradores_tras_coma` and this one
                // did not, so the difference was the SCOPE and not the syntax.
                // The message, "expected type, got Comma", sends you to look
                // at the type, which is perfect. It was the first error in 20
                // of DOOM's files.
                self.declaradores_globales_tras_coma(&base, &mut globals)?;
                self.skip_semicolon();
                self.var_types.insert(name.clone(), typ.clone());
                globals.push(GlobalDecl::Var(typ, name, init));
            }
        }
        globals.append(&mut self.globales_pendientes);
        // * LO QUE ESTE FRONTEND DICE QUE MIDE CADA AGREGADO.
        //
        // No se calcula aqui: se COPIA de las tablas que el parser ya lleno al
        // colocar cada struct. Recalcularlo seria fabricar un tercer juez.
        let disposiciones = self
            .struct_fields
            .iter()
            .map(|(nombre, campos)| {
                (
                    nombre.clone(),
                    DisposicionAgregado {
                        campos: campos.clone(),
                        tamano: self.struct_sizes.get(nombre).copied().unwrap_or(0),
                        alineado: self.struct_aligns.get(nombre).copied().unwrap_or(0),
                    },
                )
            })
            .collect();
        Ok(Program { globals, functions, exported: Vec::new(), disposiciones })
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
        // We must post-process the AST to convert Expr::Call -> Expr::Syscall
        // for any function names that match a loaded syscall definition.
        let usings = std::mem::take(&mut self.usings);
        for path in &usings {
            // Load module sources (optional -- module may not exist for syscall-only paths)
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
        // Post-process: convert Expr::Call(name,args) -> Expr::Syscall(def,args)
        // for any function calls whose name matches a loaded syscall definition.
        self.resolve_syscalls_in_program(&mut program);
        // Validate syscall argument counts
        self.validate_syscall_args(&program)?;
        Ok(program)
    }

}

/// Fold a constant expression down to its value, or `None` if it is not one.
///
/// It exists for enum values, which C requires to be constant expressions. The
/// parser already turns macros and previous enum constants into `Expr::Int`
/// (see `parse_primary`), so what arrives here is arithmetic over literals --
/// `(30*TICRATE)` reaches this function as `Mul(Int(30), Int(35))`.
///
/// Returning `None` instead of guessing is the point: a value that cannot be
/// computed at compile time is an error with a name, not a silent zero. That
/// is the same rule the loader applies to a global it cannot evaluate.
fn const_eval(e: &Expr) -> Option<i64> {
    Some(match e {
        Expr::Int(n) => *n,
        Expr::CharLit(c) => *c as i64,
        Expr::Neg(a) => const_eval(a)?.wrapping_neg(),
        Expr::Not(a) => (const_eval(a)? == 0) as i64,
        Expr::BitNot(a) => !const_eval(a)?,
        Expr::Add(a, b) => const_eval(a)?.wrapping_add(const_eval(b)?),
        Expr::Sub(a, b) => const_eval(a)?.wrapping_sub(const_eval(b)?),
        Expr::Mul(a, b) => const_eval(a)?.wrapping_mul(const_eval(b)?),
        Expr::Div(a, b) => {
            let d = const_eval(b)?;
            if d == 0 { return None; }
            const_eval(a)? / d
        }
        Expr::Mod(a, b) => {
            let d = const_eval(b)?;
            if d == 0 { return None; }
            const_eval(a)? % d
        }
        Expr::Shl(a, b) => const_eval(a)?.wrapping_shl(const_eval(b)? as u32),
        Expr::Shr(a, b) => const_eval(a)?.wrapping_shr(const_eval(b)? as u32),
        Expr::BitAnd(a, b) => const_eval(a)? & const_eval(b)?,
        Expr::BitOr(a, b) => const_eval(a)? | const_eval(b)?,
        Expr::BitXor(a, b) => const_eval(a)? ^ const_eval(b)?,
        _ => return None,
    })
}

/// Reescribe `lvalue` + 1 (o - 1) como la ASIGNACION que ya sabe emitir el
/// codegen, para las cinco formas de lvalue que existen.
///
/// Es la misma tabla que usan `+=` y `-=`; sacarla aparte es lo que permite que
/// `++`, `--` y `+= 1` compartan un solo camino en vez de tres que tienen que
/// coincidir. Devuelve `None` si lo que llega no es asignable -- y entonces
/// quien llama lo DICE, que es la mitad que faltaba.
fn asignacion_con_uno(expr: Expr, op: fn(Box<Expr>, Box<Expr>) -> Expr) -> Option<Expr> {
    let uno = || Box::new(Expr::Int(1));
    Some(match expr {
        Expr::Var(n) => {
            let leer = Box::new(Expr::Var(n.clone()));
            Expr::Assign(n, Box::new(op(leer, uno())))
        }
        Expr::Field(e, f, off, ft) => {
            let leer = Box::new(Expr::Field(e.clone(), f.clone(), off, ft.clone()));
            Expr::AssignField(e, f, off, ft, Box::new(op(leer, uno())))
        }
        Expr::Arrow(e, f, off, ft) => {
            let leer = Box::new(Expr::Arrow(e.clone(), f.clone(), off, ft.clone()));
            Expr::AssignArrow(e, f, off, ft, Box::new(op(leer, uno())))
        }
        Expr::Subscript(n, idx, sc) => {
            let leer = Box::new(Expr::Subscript(n.clone(), idx.clone(), sc));
            Expr::AssignSubscript(n, idx, sc, Box::new(op(leer, uno())))
        }
        Expr::IndexPtr(b, idx, ty) => {
            let leer = Box::new(Expr::IndexPtr(b.clone(), idx.clone(), ty.clone()));
            Expr::AssignIndexPtr(b, idx, ty, Box::new(op(leer, uno())))
        }
        Expr::Deref(a) => {
            let leer = Box::new(Expr::Deref(a.clone()));
            Expr::AssignDeref(a, Box::new(op(leer, uno())))
        }
        _ => return None,
    })
}
