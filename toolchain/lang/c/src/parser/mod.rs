//! C Parser -- tokens a AST (gramatica completa) + preprocesador.

pub mod preprocessor;
/// Las listas `{ ... }`, en su propio fichero. Ver su cabecera para el porque del
/// reparto y para que hicieron GCC, Clang, chibicc, TCC y MSVC con esto mismo.
mod inicializador;

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

    /// Tipo estatico de una expresion, hasta donde el parser puede saberlo.
    /// Devuelve None si no es resoluble (y el offset caera a 0 -- visible en tests).
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
                    // * `*tabla` sobre un ARRAY es su primer elemento.
                    //
                    // Un array decae a puntero en cuanto se usa en una
                    // expresion, y aqui no decaia: solo se aceptaba `Ptr`. El
                    // precio lo pagaba el modismo mas comun de C --
                    // `sizeof(t) / sizeof(*t)` para contar los elementos-- que
                    // aparece en nueve ficheros de DOOM y en casi todo
                    // programa que recorra una tabla.
                    TypeSpec::Array(base, _) => Some(*base),
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
            // * Los que faltaban, y los cuatro son EXACTOS.
            //
            // Aparecieron pidiendolos `sizeof`, que desde hoy acepta una
            // expresion. Se anaden solo estos y no la aritmetica: el tipo de
            // `a + b` pide las conversiones usuales de C, y **equivocarse aqui
            // no da un error, da un `memset` de la medida equivocada**. Sin
            // resolver, `sizeof` lo dice; adivinando, lo escribe.
            //
            // `p[i]` ya trae el tipo del elemento dentro del nodo, que es
            // justo lo que hace que este sea exacto y no una suposicion.
            Expr::IndexPtr(_, _, elem) => Some(elem.clone()),
            Expr::Cast(t, _) => Some(t.clone()),
            Expr::Int(_) => Some(TypeSpec::Int),
            Expr::CharLit(_) => Some(TypeSpec::Char),
            // En C `sizeof("abc")` son CUATRO: el literal es un array con su
            // cero, no un puntero.
            Expr::StringLit(s) => Some(TypeSpec::Array(
                Box::new(TypeSpec::Char),
                s.len() as u32 + 1,
            )),
            _ => None,
        }
    }

    /// Struct/union del que la expresion ES valor (para `expr.field`).
    fn resolve_struct_type(&self, expr: &Expr) -> Option<String> {
        let t = self.resolve_expr_type(expr)?;
        match &t {
            TypeSpec::StructRef(s) | TypeSpec::UnionRef(s) => Some(s.clone()),
            // permisivo historico: p[i] con p: struct* ya cae en resolve_expr_type
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

    /// Tamano del elemento apuntado/contenido por `base` (para escalar subindices).
    /// * THE STRIDE OF ONE STEP, AND WHY IT IS NOT A `u8`.
    ///
    /// For `int grid[2][3]`, one step of the outer index is a whole ROW: three
    /// ints, twelve bytes. The old version answered 8 for any array-of-array
    /// (it fell through to a catch-all), so `grid[1][0]` read `grid[0][2]`.
    /// That compiles, runs, and prints a plausible number -- the failure mode
    /// this compiler's own test bench exists to catch.
    ///
    /// It returns `u32` because a row is not small: `gammatable[5][256]` steps
    /// 256 bytes, and a table of 1024 ints steps 4096. A `u8` here does not
    /// clamp, it WRAPS, which is the same bug with a bigger table.
    fn pointee_size(&self, base: &TypeSpec) -> u32 {
        match base {
            TypeSpec::Char | TypeSpec::UnsignedChar => 1,
            TypeSpec::Short | TypeSpec::UnsignedShort => 2,
            TypeSpec::Int | TypeSpec::UnsignedInt => 4,
            TypeSpec::Float => 4, TypeSpec::Double => 8,
            TypeSpec::Void => 1,
            TypeSpec::StructRef(s) | TypeSpec::UnionRef(s) => *self.struct_sizes.get(s.as_str()).unwrap_or(&8) as u32,
            TypeSpec::Array(inner, n) => self.pointee_size(inner).saturating_mul(*n),
            _ => 8,
        }
    }

    fn element_size(&self, name: &str) -> u32 {
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

    /// La regla de disposicion **ya no esta aqui**: vive una sola vez en
    /// `bmo_abi::types::disposicion`. Estaba copiada a mano en tres sitios
    /// --este, `codegen::build_struct_layout` y el parser de C++-- y una
    /// divergencia entre ellas no da un error: da un programa que escribe en
    /// el campo de al lado.
    fn compute_struct_layout(&mut self, name: &str, members: &[StructMember]) {
        let mut layout = Vec::new();
        let mut d = bmo_abi::types::Disposicion::nueva();
        for m in members {
            let sz = m.typ.stack_size();
            layout.push((m.name.clone(), d.coloca(sz), sz));
            self.field_types.insert((name.to_string(), m.name.clone()), m.typ.clone());
        }
        self.struct_fields.insert(name.to_string(), layout);
        self.struct_sizes.insert(name.to_string(), d.total());
    }

    fn compute_union_layout(&mut self, name: &str, members: &[StructMember]) {
        let mut layout = Vec::new();
        let mut d = bmo_abi::types::DisposicionUnion::nueva();
        for m in members {
            let sz = m.typ.stack_size();
            layout.push((m.name.clone(), d.coloca(sz), sz));
            self.field_types.insert((name.to_string(), m.name.clone()), m.typ.clone());
        }
        self.struct_fields.insert(name.to_string(), layout);
        self.struct_sizes.insert(name.to_string(), d.total());
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

    /// Walk all function bodies and convert Expr::Call -> Expr::Syscall for
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
        // `static int f(){...}` -- el `static` de una funcion es enlace interno,
        // y aqui solo hay una unidad de traduccion. Se acepta y se sigue.
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
        let mut variadica = false;
        while *self.peek() != Token::CloseParen && *self.peek() != Token::Eof {
            // * `...` -- el resto de los argumentos, sin nombre ni tipo.
            //
            // Va SIEMPRE al final, y por eso se corta el bucle aqui: lo que
            // viniera detras no seria un parametro de nadie.
            if *self.peek() == Token::Puntos {
                self.advance();
                variadica = true;
                break;
            }
            if *self.peek() == Token::Void && (self.pos + 1 >= self.tokens.len() || self.tokens[self.pos + 1] == Token::CloseParen) {
                self.advance(); break;
            }
            let ptype = self.parse_type_spec()?;
            // * A PARAMETER THAT IS A POINTER TO FUNCTION:
            //   `void P_PathTraverse(..., boolean (*trav)(intercept_t *))`
            //
            // The declarator was understood for globals, members, locals and
            // typedefs, and not here -- so a callback could be declared, stored
            // and called, but never PASSED. The message, "expected param name,
            // got OpenParen", blames the parenthesis.
            //
            // It was the first error in 24 of DOOM's files, and it is not a
            // corner of the language there: `p_map.c` and `p_sight.c` are built
            // on passing the traverser in.
            if *self.peek() == Token::OpenParen
                && self.tokens.get(self.pos + 1) == Some(&Token::Star)
            {
                let (pname, ptype) = self.parse_fnptr_tail()?;
                self.var_types.insert(pname.clone(), ptype.clone());
                params.push(Param { typ: ptype, name: pname });
                if *self.peek() == Token::Comma { self.advance(); }
                continue;
            }
            // * El nombre del parametro es OPCIONAL.
            //
            // `int f(int);` es C legal y es como se escriben los prototipos en
            // las cabeceras de cualquier programa de verdad -- DOOM incluido.
            // Aqui se exigia nombre siempre, asi que un prototipo moria con
            // "expected param name, got CloseParen": un mensaje que acusa al
            // programa de algo que el estandar permite.
            //
            // Sin nombre no se puede referenciar dentro del cuerpo, y por eso
            // solo aparece en declaraciones. Se le pone uno inventado para que
            // el resto del compilador no tenga que saber que puede faltar.
            let pname = match self.peek().clone() {
                Token::Ident(n) => { self.advance(); n }
                Token::Comma | Token::CloseParen => {
                    anonimos += 1;
                    format!("_anon{}", anonimos)
                }
                t => return Err(CError::new(self.line(),format!("expected param name, got {:?}", t))),
            };
            // * El tipo de un PARAMETRO tambien se registra.
            //
            // Solo se guardaba el de las variables locales, asi que dentro de
            // `int suma(struct P p)` el parser no sabia que `p` era un struct:
            // `p.x` salia como un campo de offset 0 y tipo `long`, y los tres
            // campos leian **la misma direccion y ocho bytes**. Daba
            // `0x200000001` -- las dos primeras `int` juntas -- en vez de 1.
            //
            // Mientras un parametro solo pudo ser un escalar esto no se notaba:
            // ningun escalar tiene campos que consultar.
            // * `void f(patch_t *c[])` -- un PARAMETRO declarado como array.
            //
            // En C un array como parametro ES un puntero (decae al llamar), y
            // aqui ni siquiera se leian los corchetes: el `[` sobraba y el
            // error acusaba al tipo. `wi_stuff.c` pasa asi sus tablas de
            // graficos.
            let ptype = if *self.peek() == Token::OpenBracket {
                let t = self.parse_array_suffix(ptype)?;
                match t {
                    TypeSpec::Array(base, _) => TypeSpec::Ptr(base),
                    otro => otro,
                }
            } else {
                ptype
            };
            self.var_types.insert(pname.clone(), ptype.clone());
            params.push(Param { typ: ptype, name: pname });
            if *self.peek() == Token::Comma { self.advance(); }
        }
        self.expect(&Token::CloseParen)?;
        // * PROTOTIPO: `int f(int a);` -- declarar sin definir.
        //
        // Sin esto no se puede llamar a una funcion antes de escribirla, y eso
        // no es una comodidad: **la recursion mutua es imposible sin ella**. Un
        // programa de cincuenta ficheros --DOOM son unos cincuenta-- esta lleno
        // de funciones que se llaman en circulo, y ninguna puede ir "antes" de
        // todas las demas. Era el hueco mas caro de los que quedaban, y no se
        // sabia que estaba: el lexer no tiene la culpa de nada aqui.
        //
        // No emite codigo. Lo unico que deja es el tipo de retorno anotado,
        // para que una llamada anterior a la definicion sepa que recibe.
        if *self.peek() == Token::Semicolon {
            self.advance();
            self.var_types.insert(name.clone(), ret_type);
            return Ok(Tope::Prototipo);
        }
        // After expect advances past ), pos should be at {
        if self.pos >= self.tokens.len() || *self.peek() != Token::OpenBrace { self.pos = save; return Ok(Tope::NoEsFuncion); }
        self.advance();
        // Cada funcion empieza sin `static` heredadas de la anterior: el mapa
        // ES el ambito.
        self.static_alias.clear();
        let mut var_count = 0u32;
        let mut var_names: Vec<String> = params.iter().map(|p| p.name.clone()).collect();
        let mut body = Vec::new();
        // Quien es la funcion, para las `static` locales de CUALQUIER bloque
        // suyo -- incluidos los anidados, que hasta ahora no podian tener una.
        self.funcion_actual = name.clone();
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
                    // `extern` en el cuerpo: declara un nombre que vive en otro
                    // sitio. Se registra el tipo y no se emite nada. Estaba en
                    // `parse_block` y faltaba aqui, que es el bucle del cuerpo
                    // de la funcion -- las mismas dos gramaticas otra vez.
                    if *self.peek() == Token::Extern {
                        self.advance();
                        if let Some((t, vname)) = self.try_parse_decl()? {
                            self.var_types.insert(vname, t);
                            self.skip_semicolon();
                        }
                        continue;
                    }
                    // * Una local `static` NO es una local: se va a las
                    // globales y aqui no queda nada.
                    if *self.peek() == Token::Static {
                        self.advance();
                        let Some((typ, vname)) = self.try_parse_decl()? else {
                            return Err(CError::new(self.line(),
                                "static: esperaba una declaracion de variable"));
                        };
                        let base = self.base_del_declarador.clone();
                        self.declarar_static_local(&name, typ, vname)?;
                        // `static int lastlevel = -1, lastepisode = -1;`
                        // La coma tambien vale detras de un `static`.
                        let mut mas = Vec::new();
                        self.declaradores_tras_coma(&base, &mut mas)?;
                        for (t2, n2) in mas {
                            self.declarar_static_local(&name, t2, n2)?;
                        }
                        continue;
                    }
                    // Un prototipo dentro del cuerpo: se consume y ya.
                    if self.saltar_prototipo_local() {
                        continue;
                    }
                    if let Some((typ, name)) = self.try_parse_decl()? {
                        let base = self.base_del_declarador.clone();
                        var_count += 1;
                        var_names.push(name.clone());
                        body.push(self.terminar_declaracion(typ, name)?);
                        // `int a, b;` -- los de detras de la coma comparten el
                        // tipo BASE y traen su propio `*` y su propio `[n]`.
                        let mut mas = Vec::new();
                        self.declaradores_tras_coma(&base, &mut mas)?;
                        for (t2, n2) in mas {
                            var_count += 1;
                            var_names.push(n2.clone());
                            body.push(self.terminar_declaracion(t2, n2)?);
                        }
                    } else {
                        body.push(self.parse_stmt()?);
                    }
                }
            }
        }
        Ok(Tope::Funcion(Function { ret_type, name, params, var_count, var_names, body, line: start_line, variadica }))
    }

    /// **Una `static` dentro de una funcion.**
    ///
    /// Aqui `static` si cambia lo que el programa hace, y en dos cosas a la vez:
    ///
    /// 1. **Sobrevive entre llamadas.** No puede vivir en la pila, que se
    ///    deshace al volver: vive donde viven las globales.
    /// 2. **Su inicializador corre UNA vez**, no en cada llamada. Por eso el
    ///    valor viaja con la global y **no se emite ninguna sentencia** en el
    ///    cuerpo -- si se emitiera una asignacion, un contador `static int n=0`
    ///    se pondria a cero en cada llamada y pareceria que no cuenta nada.
    ///
    /// Lo que NO cambia es su ambito: el nombre solo se ve dentro de su
    /// funcion, y dos funciones pueden tener cada una su `static int n`. De ahi
    /// el renombrado: la global se llama `funcion.variable` --con un punto, que
    /// un identificador de C no puede contener, asi que no puede chocar con
    /// nada que el programa escriba-- y el mapa de alias traduce.
    fn declarar_static_local(
        &mut self,
        funcion: &str,
        typ: TypeSpec,
        name: String,
    ) -> Result<(), CError> {
        let real = format!("{}.{}", funcion, name);
        // * `static event_t st_notify = { ... };` -- una LISTA tambien.
        //
        // Solo se admitia una expresion, asi que una `static` local con
        // inicializador de agregado moria con "unexpected token: OpenBrace".
        // Y una `static` local con lista no es rara: es como se escribe una
        // tabla que no hace falta fuera de su funcion -- `am_map.c` guarda ahi
        // el evento que manda al pulsar una tecla.
        if *self.peek() == Token::Assign
            && self.tokens.get(self.pos + 1) == Some(&Token::OpenBrace)
        {
            self.advance();
            let escrituras = self.parse_inicializador(&typ)?;
            let typ = self.cerrar_array_incompleto(typ, &escrituras);
            self.skip_semicolon();
            self.var_types.insert(real.clone(), typ.clone());
            self.static_alias.insert(name, real.clone());
            self.globales_pendientes
                .push(GlobalDecl::VarLista(typ, real, escrituras));
            return Ok(());
        }
        let init = if *self.peek() == Token::Assign {
            self.advance();
            Some(self.parse_assign()?)
        } else {
            None
        };
        self.skip_semicolon();
        self.var_types.insert(real.clone(), typ.clone());
        self.static_alias.insert(name, real.clone());
        self.globales_pendientes.push(GlobalDecl::Var(typ, real, init));
        Ok(())
    }

    /// Los declaradores que van detras de una coma: `int a, *b, c[4];`.
    ///
    /// * Cada uno tiene su propio `*` y su propio `[n]`, y **comparte solo el
    /// tipo BASE**. Es el detalle de C que mas se salta al implementarlo: en
    /// `int *a, b;` la `b` es un `int`, **no** un puntero. El asterisco es del
    /// declarador, no del tipo -- y quien lo trate al reves compila el programa
    /// y le cambia el significado.
    fn declaradores_tras_coma(
        &mut self,
        base: &TypeSpec,
        salida: &mut Vec<(TypeSpec, String)>,
    ) -> Result<(), CError> {
        while *self.peek() == Token::Comma {
            self.declaradores_tras_coma_uno(base, salida)?;
        }
        Ok(())
    }

    /// One declarator after one comma. Split out of the loop above so that the
    /// file-scope caller can stop between declarators -- at file scope each one
    /// may carry its own initializer, and reading them all first would leave
    /// the `=` behind.
    fn declaradores_tras_coma_uno(
        &mut self,
        base: &TypeSpec,
        salida: &mut Vec<(TypeSpec, String)>,
    ) -> Result<(), CError> {
        self.expect(&Token::Comma)?;
        let mut typ = base.clone();
        while *self.peek() == Token::Star {
            self.advance();
            typ = TypeSpec::Ptr(Box::new(typ));
        }
        let Token::Ident(name) = self.peek().clone() else {
            return Err(CError::new(self.line(),
                "esperaba otro nombre despues de la coma en la declaracion"));
        };
        self.advance();
        if *self.peek() == Token::OpenBracket {
            typ = self.parse_array_suffix(typ)?;
        }
        salida.push((typ, name));
        Ok(())
    }

    /// The declarators after the comma of a FILE-SCOPE declaration, pushed as
    /// globals of their own.
    ///
    /// A wrapper over `declaradores_tras_coma` so that both scopes share one
    /// reader for `int *a, b[4], c;`. Each one may also carry its own `=`,
    /// which is why the initializer is read here and not by the caller.
    fn declaradores_globales_tras_coma(
        &mut self,
        base: &TypeSpec,
        globals: &mut Vec<GlobalDecl>,
    ) -> Result<(), CError> {
        while *self.peek() == Token::Comma {
            let mut mas = Vec::new();
            // Reads exactly one declarator: the helper loops on commas, and
            // the initializer belongs to the one just read.
            let antes = self.pos;
            self.declaradores_tras_coma_uno(base, &mut mas)?;
            if self.pos == antes {
                break;
            }
            for (typ, name) in mas {
                if *self.peek() == Token::Assign
                    && self.tokens.get(self.pos + 1) == Some(&Token::OpenBrace)
                {
                    self.advance();
                    let escrituras = self.parse_inicializador(&typ)?;
                    let typ = self.cerrar_array_incompleto(typ, &escrituras);
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
                self.var_types.insert(name.clone(), typ.clone());
                globals.push(GlobalDecl::Var(typ, name, init));
            }
        }
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
        // puntero a funcion: RETTYPE (*name)(params) -- variable de tipo puntero.
        // Es lo que sostiene las vtables de C++ y las tablas de drivers.
        if *self.peek() == Token::OpenParen
            && self.tokens.get(self.pos + 1) == Some(&Token::Star)
        {
            match self.parse_fnptr_tail() {
                Ok((fname, ftyp)) => {
                    if *self.peek() != Token::Semicolon && *self.peek() != Token::Assign {
                        self.pos = save; return Ok(None);
                    }
                    return Ok(Some((ftyp, fname)));
                }
                Err(_) => { self.pos = save; return Ok(None); }
            }
        }
        let Token::Ident(name) = self.peek().clone() else { self.pos = save; return Ok(None); };
        if self.pos + 1 < self.tokens.len() && self.tokens[self.pos + 1] == Token::OpenParen {
            self.pos = save; return Ok(None);
        }
        self.advance();
        // * El MISMO lector de corchetes que el nivel de fichero.
        //
        // Este camino tenia su propia copia, que leia una sola dimension y
        // exigia una medida dentro. Asi que lo que ya funcionaba fuera de una
        // funcion volvia a fallar dentro de ella:
        //
        //   byte endtrack[] = {0xFF, 0x2F, 0x00};   "unexpected token: CloseBracket"
        //   short caja[2][4];                       el segundo [4] sobraba
        //
        // Dos copias de la misma regla es exactamente como se llega a que una
        // sepa algo que la otra no. Ahora es `parse_array_suffix` en los dos.
        if *self.peek() == Token::OpenBracket {
            typ = self.parse_array_suffix(typ)?;
        }
        // * La COMA tambien cierra un declarador: `int a, b;`.
        //
        // Antes solo valian `;` y `=`, asi que `int a, b;` no se reconocia como
        // declaracion y caia al camino de las expresiones -- donde `b` no existe
        // todavia. Lo destapo una sonda de `memcpy` que declaraba
        // `char a[4],b[4];` y acusaba a `memcpy`, que estaba perfecto.
        if *self.peek() != Token::Semicolon
            && *self.peek() != Token::Assign
            && *self.peek() != Token::Comma
        {
            self.pos = save; return Ok(None);
        }
        Ok(Some((typ, name)))
    }

    /// Consume the `{ ... }` of a struct or a union and return its members.
    /// Assumes the cursor is on the `{`.
    ///
    /// It lives on its own so that the TAGGED form (`struct P { ... };`) and
    /// the untagged one (`typedef struct { ... } P;`) read the same body with
    /// the same code. They used to be one path, which is why only the tagged
    /// one existed.
    fn parse_aggregate_body(&mut self) -> Result<Vec<StructMember>, CError> {
        self.expect(&Token::OpenBrace)?;
        let mut members = Vec::new();
        while *self.peek() != Token::CloseBrace && *self.peek() != Token::Eof {
            let mtype = self.parse_type_spec()?;
            // The base type, before the declarator's stars, for the members
            // after a comma.
            let base = self.base_del_declarador.clone();
            // A pointer-to-function member: `void (*action)(void);`
            if *self.peek() == Token::OpenParen
                && self.tokens.get(self.pos + 1) == Some(&Token::Star)
            {
                let (mname, mtyp) = self.parse_fnptr_tail()?;
                self.skip_semicolon();
                members.push(StructMember { typ: mtyp, name: mname });
                continue;
            }
            let mname = match self.advance() {
                Token::Ident(n) => n,
                t => return Err(CError::new(self.line(),format!("expected member name, got {:?}", t))),
            };
            // * `char name[8];` -- un ARRAY como miembro.
            //
            // Faltaba, y el error que salia --"expected type, got
            // OpenBracket"-- mandaba a mirar el tipo, que estaba perfecto. La
            // sonda lo encontro en la union, pero fallaba **igual en un
            // struct**: es el declarador, no el agregado.
            //
            // El tamano y el alineado salen solos: `stack_size()` de un
            // `Array(t,n)` ya es `t*n`, y el reparto de offsets se calcula
            // con eso.
            // Same reader as everywhere else, which is what buys `short
            // bbox[2][4];` -- a two-dimensional MEMBER. This branch had its
            // own bracket code that read one dimension and demanded a literal,
            // so the second `[4]` was left in front of the parser.
            //
            // `doomdata.h` is the on-disk map format: nodes carry their
            // bounding boxes exactly like that, and every file that can read a
            // level reaches it.
            let mtype = if *self.peek() == Token::OpenBracket {
                self.parse_array_suffix(mtype)?
            } else {
                mtype
            };
            // * Campo de bits: `unsigned a:3;`.
            //
            // Se ACEPTA la sintaxis y se le da al campo su tipo entero
            // entero -- **sin empaquetar**. Y se dice aqui por que, porque es
            // una decision y no un descuido: empaquetar de verdad obliga a que
            // cada lectura lleve su desplazamiento y su mascara, y cada
            // escritura sea leer-modificar-escribir. Eso es correcto solo si
            // se hace entero; a medias da campos que se pisan.
            //
            // Mientras no este, un `unsigned a:3` ocupa sus cuatro bytes y
            // **guarda lo que le metas**: el programa hace lo que dice, solo
            // que la estructura mide mas. Lo que NO vale es un layout binario
            // ajeno -- ver BRECHA.md.
            if *self.peek() == Token::Colon {
                self.advance();
                match self.advance() {
                    Token::IntLit(_) => {}
                    t => return Err(CError::new(self.line(), format!(
                        "'{mname}:': la anchura de un campo de bits es un numero, no {t:?}"))),
                }
            }
            members.push(StructMember { typ: mtype, name: mname });
            // * `int data1, data2, data3, data4;` INSIDE the aggregate.
            //
            // One member per line was the assumption, and C does not make it.
            // `d_event.h` -- the event every input in DOOM travels in -- packs
            // its four payload fields on one line, so this single line was the
            // first error in twenty files that never even reach their own code.
            //
            // Same reader as the other two scopes: the type is the BASE, and
            // each name brings its own `*` and its own `[n]`.
            let mut mas = Vec::new();
            self.declaradores_tras_coma(&base, &mut mas)?;
            for (t2, n2) in mas {
                members.push(StructMember { typ: t2, name: n2 });
            }
            self.skip_semicolon();
        }
        self.expect(&Token::CloseBrace)?;
        Ok(members)
    }

    /// Un PROTOTIPO dentro de un cuerpo: `void WI_unloadData(void);`
    ///
    /// C lo permite y `wi_stuff.c` lo usa para declarar una funcion justo antes
    /// de llamarla. Aqui no declara nada --el compilador ya admite llamar a lo
    /// que se defina despues-- pero hay que CONSUMIRLO: sin esto caia en el
    /// camino de las expresiones y el error acusaba al tipo ("unexpected token:
    /// Void"), que es lo unico de la linea que estaba bien.
    ///
    /// Devuelve si consumio algo. Si no lo era, deja el cursor donde estaba.
    fn saltar_prototipo_local(&mut self) -> bool {
        let guardado = self.pos;
        if !self.peek_is_type_start() {
            return false;
        }
        if self.parse_type_spec().is_err() {
            self.pos = guardado;
            return false;
        }
        let Token::Ident(_) = self.peek().clone() else {
            self.pos = guardado;
            return false;
        };
        self.advance();
        if *self.peek() != Token::OpenParen {
            self.pos = guardado;
            return false;
        }
        // La lista de parametros, equilibrada.
        self.advance();
        let mut hondo = 1;
        while hondo > 0 {
            match self.advance() {
                Token::OpenParen => hondo += 1,
                Token::CloseParen => hondo -= 1,
                Token::Eof => { self.pos = guardado; return false; }
                _ => {}
            }
        }
        // Solo es un prototipo si termina en `;`. Si sigue una llave, es una
        // definicion anidada, y eso no es C.
        if *self.peek() != Token::Semicolon {
            self.pos = guardado;
            return false;
        }
        self.advance();
        true
    }

    /// `lvalue++` / `lvalue--` sobre algo que no es un nombre suelto.
    ///
    /// El valor de un post-incremento es el ANTERIOR, asi que se escribe la
    /// asignacion y se deshace por fuera: `(x += 1) - 1`. Exacto para enteros.
    ///
    /// Sobre un PUNTERO no lo es --`+1` avanza un elemento y `-1` restaria un
    /// byte-- y por eso ahi se rechaza con el motivo en vez de emitir algo que
    /// casi acierta.
    fn post_sobre_lvalue(&mut self, expr: Expr, mas: bool) -> Result<Expr, CError> {
        if let Some(TypeSpec::Ptr(_)) = self.resolve_expr_type(&expr) {
            return Err(CError::new(
                self.line(),
                "'++'/'--' detras de un puntero que no es una variable suelta todavia no \
                 se compila: usa `p = p + 1` y di cual quieres",
            ));
        }
        let op: fn(Box<Expr>, Box<Expr>) -> Expr = if mas { Expr::Add } else { Expr::Sub };
        let asignacion = asignacion_con_uno(expr, op).ok_or_else(|| {
            CError::new(self.line(), "'++'/'--' necesita algo a lo que se pueda asignar")
        })?;
        // Deshacer por fuera: el valor de la expresion es el de antes.
        Ok(if mas {
            Expr::Sub(Box::new(asignacion), Box::new(Expr::Int(1)))
        } else {
            Expr::Add(Box::new(asignacion), Box::new(Expr::Int(1)))
        })
    }

    /// A tag for an aggregate that was written without one.
    ///
    /// The layout tables are keyed by name, so an untagged struct still needs
    /// one -- it just needs to be a name no source file can collide with.
    fn anon_tag(&mut self, is_union: bool) -> String {
        self.anon_aggregates += 1;
        let kind = if is_union { "union" } else { "struct" };
        format!("<anon {kind} {}>", self.anon_aggregates)
    }

    /// Consume an `enum` specifier: `enum [tag] [{ constants }]`.
    ///
    /// One function for the three shapes C allows, because they are the same
    /// grammar and splitting them is how they drifted apart before:
    ///
    /// ```text
    ///   enum tag { A, B };          a definition
    ///   enum { A, B };              the SAME, with no tag -- legal, and it
    ///                               used to fail with "expected enum name"
    ///   typedef enum { A } thing_t; a definition inside a typedef
    /// ```
    ///
    /// The tag is parsed and dropped on purpose: an enum in this compiler is
    /// `int` plus a table of constants, so the tag names nothing that outlives
    /// this call. What matters is the constants, and those are global.
    ///
    /// The value of a constant is a CONSTANT EXPRESSION, not an integer
    /// literal. DOOM needs exactly that -- `sk_noitems = -1` and
    /// `INVULNTICS = (30*TICRATE)` -- and requiring a literal rejected both.
    fn parse_enum_spec(&mut self) -> Result<(), CError> {
        self.expect(&Token::Enum)?;
        if let Token::Ident(_) = self.peek() {
            self.advance();
        }
        // `enum tag x;` names an existing enum and defines nothing.
        if *self.peek() != Token::OpenBrace {
            return Ok(());
        }
        self.advance();

        let mut val = 0i64;
        loop {
            match self.advance() {
                Token::Ident(en) => {
                    if *self.peek() == Token::Assign {
                        self.advance();
                        let e = self.parse_conditional()?;
                        val = const_eval(&e).ok_or_else(|| {
                            CError::new(
                                self.line(),
                                format!("enum '{en}': the value is not a constant expression"),
                            )
                        })?;
                    }
                    // The constant resolves to its VALUE where it is used (see
                    // parse_primary); its type stays int.
                    self.var_types.insert(en.clone(), TypeSpec::Int);
                    self.enum_constants.insert(en.clone(), val);
                }
                Token::CloseBrace => break,
                t => {
                    return Err(CError::new(
                        self.line(),
                        format!("expected enum constant, got {t:?}"),
                    ))
                }
            }
            val += 1;
            if *self.peek() == Token::Comma {
                self.advance();
            }
        }
        Ok(())
    }

    /// Consume la cola de un puntero a funcion: `(*name)(param-types)`.
    /// Asume estar en el `(` inicial. Devuelve el nombre. El tipo del
    /// puntero es opaco (se trata como Ptr): las llamadas son indirectas.
    fn parse_fnptr_tail(&mut self) -> Result<(String, TypeSpec), CError> {
        self.expect(&Token::OpenParen)?;
        self.expect(&Token::Star)?;
        let name = match self.advance() {
            Token::Ident(n) => n,
            t => return Err(CError::new(self.line(), format!("expected fnptr name, got {:?}", t))),
        };
        // `static int (*wipes[])(int, int, int)` -- una TABLA de punteros a
        // funcion. Los corchetes van dentro del parentesis, entre el nombre y
        // el cierre, y no se leian. `f_wipe.c` guarda ahi los tres efectos de
        // transicion del juego.
        let mut typ = TypeSpec::Ptr(Box::new(TypeSpec::Void));
        if *self.peek() == Token::OpenBracket {
            // Los corchetes hacen del declarador una TABLA de punteros, y ese
            // dato tiene que salir de aqui: si se pierde, el tipo queda escalar
            // y su lista de inicializacion contesta "sobran valores".
            typ = self.parse_array_suffix(typ)?;
        }
        self.expect(&Token::CloseParen)?;
        // saltar la lista de parametros ( ... ) balanceada
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
        Ok((name, typ))
    }

    fn parse_type_and_name(&mut self) -> Result<(TypeSpec, String), CError> {
        let mut typ = self.parse_type_spec()?;
        // puntero a funcion en globals/params: RETTYPE (*name)(params)
        if *self.peek() == Token::OpenParen
            && self.tokens.get(self.pos + 1) == Some(&Token::Star)
        {
            let (fname, ftyp) = self.parse_fnptr_tail()?;
            return Ok((ftyp, fname));
        }
        let name = match self.advance() {
            Token::Ident(n) => n,
            t => return Err(CError::new(self.line(),format!("expected identifier, got {:?}", t))),
        };
        // array declarator [size] -- el tamano SE GUARDA (antes se tiraba)
        if *self.peek() == Token::OpenBracket {
            typ = self.parse_array_suffix(typ)?;
        }
        Ok((typ, name))
    }

    /// The `[...]` of a declarator, with or without a size inside.
    ///
    /// * WHY AN EMPTY `[]` IS A LENGTH OF ZERO AND NOT AN ERROR
    ///
    /// `int t[] = { 10, 20, 30 };` and `extern int t[];` are both ordinary C
    /// and both used to die on the bracket -- `parse_expr` was called on a `]`
    /// and reported "unexpected token: CloseBracket", which names the symbol
    /// and not the situation.
    ///
    /// Zero here means INCOMPLETE, not empty. When an initializer follows, the
    /// length is whatever the initializer wrote (see `cerrar_array_incompleto`)
    /// -- which is the C rule: the list is what says how long the array is.
    /// Without an initializer it stays incomplete, which is exactly what
    /// `extern int t[];` claims: the array is somebody else's.
    /// * AND IT CONSUMES EVERY BRACKET, NOT JUST THE FIRST.
    ///
    /// `extern const byte gammatable[5][256];` -- DOOM's gamma tables, in
    /// `tables.h`, which almost every file reaches through `r_local.h`. Only
    /// the first `[5]` was read, so the `[256]` was left in front of the
    /// parser, which asked for a type and got a bracket. 39 files.
    ///
    /// The dimensions fold from the RIGHT, because that is what they mean:
    /// `[5][256]` is five arrays of 256, not an array of five-by-256.
    fn parse_array_suffix(&mut self, base: TypeSpec) -> Result<TypeSpec, CError> {
        let mut dims = Vec::new();
        while *self.peek() == Token::OpenBracket {
            self.advance();
            if *self.peek() == Token::CloseBracket {
                self.advance();
                dims.push(0);
                continue;
            }
            let size_expr = self.parse_expr()?;
            self.expect(&Token::CloseBracket)?;
            // A size that cannot be computed is an ERROR, not a 1.
            //
            // It used to fall back to one element, and that is the worst
            // possible answer: the program compiles, the array is a single
            // slot, and every write past the first lands on whatever follows
            // it. Same rule as a global the compiler cannot evaluate.
            match const_eval(&size_expr) {
                Some(n) if n > 0 => dims.push(n as u32),
                Some(n) => {
                    return Err(CError::new(
                        self.line(),
                        format!("un array no puede medir {n}"),
                    ))
                }
                None => {
                    return Err(CError::new(
                        self.line(),
                        "la medida de un array tiene que ser una constante que se pueda \
                         calcular al compilar".to_string(),
                    ))
                }
            }
        }
        let mut typ = base;
        for n in dims.into_iter().rev() {
            typ = TypeSpec::Array(Box::new(typ), n);
        }
        Ok(typ)
    }

    /// Give an incomplete array the length its initializer just implied.
    ///
    /// The writes carry absolute offsets, so the last one plus one element is
    /// the length. Anything that is not an incomplete array passes through.
    fn cerrar_array_incompleto(
        &self,
        typ: TypeSpec,
        escrituras: &[Escritura],
    ) -> TypeSpec {
        let TypeSpec::Array(elem, 0) = &typ else { return typ };
        let tam = self.tamano_de(elem).max(1);
        let n = escrituras
            .iter()
            .map(|e| e.offset / tam + 1)
            .max()
            .unwrap_or(0);
        TypeSpec::Array(elem.clone(), n.max(1))
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
                // * `inline` and its GCC spellings are consumed and dropped.
                //
                // Not laziness: `inline` is a REQUEST, and the standard says a
                // conforming compiler may ignore it. BMO C does not inline, so
                // honouring it and ignoring it produce the same program -- the
                // only difference was that the word made the file stop.
                //
                // `__inline__` and `__forceinline` are here because DOOM's
                // `m_misc.c` and `sha1.c` reach for them behind an `#ifdef`
                // that resolves to whatever the host compiler was.
                //
                // The day there IS an inliner, this is where it stops being a
                // no-op, and nothing else has to move.
                Token::Ident(n)
                    if n == "inline" || n == "__inline" || n == "__inline__"
                        || n == "__forceinline" =>
                {
                    self.advance();
                }
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
            // * `struct`/`union`, WITH or WITHOUT a tag, with or without a body.
            //
            // Only `struct P` was understood here, so the two shapes C code
            // actually uses for a one-off type both failed on the brace:
            //
            //   typedef struct { ... } thing_t;   "expected struct name, got OpenBrace"
            //   typedef union  { ... } action_t;  "expected union name, got OpenBrace"
            //
            // That is 34 of DOOM's 81 files, and `d_think.h` -- the union at
            // the centre of every thinker in the game -- is one of them.
            //
            // An untagged aggregate still gets a tag, because the layout table
            // is keyed by name. It is generated with characters an identifier
            // cannot contain, so it can never collide with a real one.
            tok @ (Token::Struct | Token::Union) => {
                let is_union = tok == Token::Union;
                let name = match self.peek() {
                    Token::Ident(_) => match self.advance() {
                        Token::Ident(n) => n,
                        _ => unreachable!(),
                    },
                    _ => self.anon_tag(is_union),
                };
                if *self.peek() == Token::OpenBrace {
                    let members = self.parse_aggregate_body()?;
                    if is_union {
                        self.compute_union_layout(&name, &members);
                        self.globales_pendientes
                            .push(GlobalDecl::Union(name.clone(), members));
                    } else {
                        self.compute_struct_layout(&name, &members);
                        self.globales_pendientes
                            .push(GlobalDecl::Struct(name.clone(), members));
                    }
                }
                if is_union { TypeSpec::UnionRef(name) } else { TypeSpec::StructRef(name) }
            }
            // * An `enum` IS a type, and `int` is the type it is.
            //
            // Without this arm the specifier was only understood at file
            // scope, so `typedef enum { A, B } thing_t;` failed with "expected
            // type, got Enum" -- and that is the single most common way C code
            // declares an enum. It reached 30 of DOOM's 81 files.
            //
            // The token is pushed back for `parse_enum_spec`, which owns the
            // whole shape (optional tag, optional body) so the two places
            // cannot disagree about what an enum looks like.
            Token::Enum => {
                self.pos -= 1;
                self.parse_enum_spec()?;
                TypeSpec::Int
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
        // * El tipo BASE, **antes** de los asteriscos. Lo necesitan los
        // declaradores que vengan detras de una coma: en `int *a, b;` la `b`
        // es un `int`, no un puntero -- el asterisco es del DECLARADOR.
        self.base_del_declarador = base.clone();
        // punteros multinivel: int **pp, char ***ppp, ...
        let mut typ = base;
        while *self.peek() == Token::Star {
            self.advance();
            typ = TypeSpec::Ptr(Box::new(typ));
            // * `char * const p` -- el calificador va DETRAS del asterisco, y
            // ahi califica al PUNTERO, no a lo apuntado.
            //
            // Solo se quitaban los de delante, asi que esto moria con
            // "expected identifier, got Const": el parser pedia el nombre y se
            // encontraba una palabra clave que en ese sitio es legal.
            //
            // Se consume y se tira, como el de delante: BMO C no comprueba
            // constancia, y fingir que si seria peor que no hacerlo.
            self.strip_qualifiers();
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
                    // * La etiqueta de un `case` es una EXPRESION CONSTANTE.
                    //
                    // Se aceptaba un literal, una constante de enum o un
                    // caracter, y nada mas. `case -1:` moria con "expected int
                    // in case, got Minus" -- un mensaje que acusa al signo.
                    //
                    // Se lee con el mismo `parse_conditional` + `const_eval`
                    // que los valores de un enum: el signo, `1 << 3` y
                    // `MAX - 1` son la misma cosa para quien la escribe, y
                    // ahora tambien para quien la lee.
                    let e = self.parse_conditional()?;
                    let val = const_eval(&e).ok_or_else(|| {
                        CError::new(
                            self.line(),
                            "la etiqueta de un 'case' tiene que ser una constante que se \
                             pueda calcular al compilar".to_string(),
                        )
                    })?;
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
                // * La directiva se caza AQUI, antes que nada. Es donde se
                // colaba: `try_parse_decl` miraba el `#`, decia "esto no es una
                // declaracion" y devolvia None sin consumirlo, y el bucle
                // seguia adelante -- asi que un `#define X 5` dentro de una
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
                    // ** LO QUE UN BLOQUE ANIDADO NO SABIA HACER.
                    //
                    // El cuerpo de una funcion entendia `static`, `extern` y
                    // los declaradores separados por coma. Un bloque de dentro
                    // --el de un `if`, el de un `for`-- no, porque es OTRO
                    // bucle. Asi que lo mismo compilaba o no segun estuviera
                    // una llave mas adentro:
                    //
                    //   static mobj_t dummy_mobj;      "unexpected token: Static"
                    //   extern boolean advancedemo;    "unexpected token: Extern"
                    //   char *startname, *endname;     "unexpected token: Comma"
                    //
                    // Ninguno de los tres es raro: son p_mobj.c, d_net.c y
                    // p_spec.c. Dos bucles con la misma gramatica es como se
                    // llega a que uno sepa cosas que el otro no.
                    if *self.peek() == Token::Static {
                        self.advance();
                        let Some((typ, vname)) = self.try_parse_decl()? else {
                            return Err(CError::new(self.line(),
                                "static: esperaba una declaracion de variable"));
                        };
                        let quien = self.funcion_actual.clone();
                        let base = self.base_del_declarador.clone();
                        self.declarar_static_local(&quien, typ, vname)?;
                        // `static int lastlevel = -1, lastepisode = -1;` -- la
                        // coma tambien vale detras de un `static`, y este era
                        // el ultimo sitio donde no.
                        let mut mas = Vec::new();
                        self.declaradores_tras_coma(&base, &mut mas)?;
                        for (t2, n2) in mas {
                            self.declarar_static_local(&quien, t2, n2)?;
                        }
                        continue;
                    }
                    // `extern` DENTRO de una funcion: declara un nombre que
                    // vive en otro sitio. Se registra el tipo y no se emite
                    // nada -- que es todo lo que significa.
                    if *self.peek() == Token::Extern {
                        self.advance();
                        if let Some((typ, vname)) = self.try_parse_decl()? {
                            self.var_types.insert(vname, typ);
                            self.skip_semicolon();
                        }
                        continue;
                    }
                    // Un prototipo dentro del cuerpo: se consume y ya.
                    if self.saltar_prototipo_local() {
                        continue;
                    }
                    if let Some((typ, name)) = self.try_parse_decl()? {
                        let base = self.base_del_declarador.clone();
                        stmts.push(self.terminar_declaracion(typ, name)?);
                        let mut mas = Vec::new();
                        self.declaradores_tras_coma(&base, &mut mas)?;
                        for (t2, n2) in mas {
                            stmts.push(self.terminar_declaracion(t2, n2)?);
                        }
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
            // Atajo para `printf("literal")` SIN argumentos variadicos: baja
            // directo a la puerta de consola, sin runtime ni imports.
            //
            // El `args.len() == 1` es la condicion que faltaba: antes
            // `printf("%d\n", x)` tambien entraba aqui y los argumentos se
            // DESCARTABAN en silencio -- el programa imprimia literalmente
            // "%d". Con mas de un argumento debe seguir por la ruta
            // variadica, que si los formatea.
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
        let sub_assign_op = |n: String, idx: Box<Expr>, sc: u32, val: Expr, op: fn(Box<Expr>, Box<Expr>) -> Expr| {
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
            // * `++` y `--` DELANTE, sobre cualquier lvalue y no solo un nombre.
            //
            // Solo se aceptaba `++nombre`. `--door->topcountdown` --que es como
            // `p_doors.c` cuenta los ticks de una puerta-- moria con "expected
            // CloseParen, got Arrow": el parser leia `door` como la variable
            // entera y la flecha sobraba.
            //
            // Un pre-incremento ES una asignacion: `--x` vale exactamente
            // `x = x - 1`, valor nuevo incluido. Asi que se reescribe con la
            // maquinaria de `+=`, que ya existia para las cinco formas de
            // lvalue. No hay nada nuevo en el codegen.
            Token::PlusPlus => {
                self.advance();
                if let Token::Ident(n) = self.peek().clone() {
                    if !matches!(self.tokens.get(self.pos + 1),
                        Some(Token::Arrow) | Some(Token::Dot) | Some(Token::OpenBracket))
                    {
                        self.advance();
                        return Ok(Expr::PreInc(n));
                    }
                }
                let e = self.parse_unary()?;
                asignacion_con_uno(e, Expr::Add).ok_or_else(|| {
                    CError::new(self.line(), "'++' necesita algo a lo que se pueda asignar")
                })
            }
            Token::MinusMinus => {
                self.advance();
                if let Token::Ident(n) = self.peek().clone() {
                    if !matches!(self.tokens.get(self.pos + 1),
                        Some(Token::Arrow) | Some(Token::Dot) | Some(Token::OpenBracket))
                    {
                        self.advance();
                        return Ok(Expr::PreDec(n));
                    }
                }
                let e = self.parse_unary()?;
                asignacion_con_uno(e, Expr::Sub).ok_or_else(|| {
                    CError::new(self.line(), "'--' necesita algo a lo que se pueda asignar")
                })
            }
            Token::And => { self.advance(); let expr = self.parse_unary()?; Ok(Expr::AddrOf(Box::new(expr))) }
            Token::Star => { self.advance(); let e = self.parse_unary()?; Ok(Expr::Deref(Box::new(e))) }
            // * `sizeof` de un TIPO y de una EXPRESION.
            //
            // Solo entendia el tipo, asi que `sizeof(p->campo)` moria con
            // "expected type, got Ident(p)" -- un mensaje que manda a buscar un
            // typedef que no falta. Y la forma con expresion no es un adorno:
            // es como se escribe `memset(&x, 0, sizeof(x))` sin repetir el
            // tipo, o sea la forma que NO se rompe cuando el tipo cambia.
            //
            // Se intenta primero el tipo y se vuelve atras si no cuela: los dos
            // empiezan igual y solo el intento distingue `sizeof(int)` de
            // `sizeof(x)`. La expresion no se EVALUA -- solo se le pregunta el
            // tipo, que es lo que dice el estandar.
            Token::Sizeof => {
                self.advance();
                self.expect(&Token::OpenParen)?;
                let guardado = self.pos;
                if let Ok(t) = self.parse_type_spec() {
                    if *self.peek() == Token::CloseParen {
                        self.advance();
                        return Ok(Expr::Int(self.tamano_de(&t) as i64));
                    }
                }
                self.pos = guardado;
                let e = self.parse_expr()?;
                self.expect(&Token::CloseParen)?;
                let t = self.resolve_expr_type(&e).ok_or_else(|| {
                    CError::new(
                        self.line(),
                        "sizeof: no se de que tipo es esa expresion".to_string(),
                    )
                })?;
                Ok(Expr::Int(self.tamano_de(&t) as i64))
            }
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
                            // cast REAL: codegen trunca/extiende al tamano del tipo
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
                // ** `p->x++` SE IGNORABA EN SILENCIO.
                //
                // El brazo era `_ => {}`: si el operando no era un nombre
                // suelto, el `++` se consumia y **no se emitia nada**. O sea
                // que `s->count++` compilaba, corria, y no incrementaba --
                // ningun error, ningun aviso, y un contador que no se mueve.
                // Es la peor forma de fallar que hay en este proyecto.
                //
                // Ahora se reescribe, y si no se puede, se DICE.
                //
                // * Un post-incremento vale el valor VIEJO, asi que no basta
                // con `x += 1`: se compensa con la resta de fuera. Es exacto
                // para enteros -- y por eso se rechaza sobre un puntero, donde
                // `+1` avanza un elemento y `-1` restaria un byte.
                Token::PlusPlus => {
                    self.advance();
                    match expr {
                        Expr::Var(ref n) => expr = Expr::PostInc(n.clone()),
                        otro => expr = self.post_sobre_lvalue(otro, true)?,
                    }
                }
                Token::MinusMinus => {
                    self.advance();
                    match expr {
                        Expr::Var(ref n) => expr = Expr::PostDec(n.clone()),
                        otro => expr = self.post_sobre_lvalue(otro, false)?,
                    }
                }
                Token::OpenParen => {
                    // (*fp)(args) -- llamada a traves de un puntero CALCULADO.
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
        let tok_line = self.line(); // linea del token que vamos a consumir
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
                        // consuming argument separators -- C grammar requires
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
                        // FUSION sem-asm<->C: __hlt(), __outb(p,v), __rdtsc()... =
                        // instruccion de la tabla como funcion. El namespace __
                        // es reservado a la implementacion -- aqui ES la
                        // implementacion. La aridad la valida el codegen contra
                        // la tabla (donde vive la verdad de cada intrinseco).
                        Ok(Expr::Intrinsic(stripped.to_string(), args))
                    } else {
                        Ok(Expr::Call(name, args))
                    }
                } else if let Some(&value) = self.enum_constants.get(&name) {
                    // Una constante de enum ES su valor, no una variable: no
                    // tiene direccion ni hueco en la pila.
                    Ok(Expr::Int(value))
                } else if let Some(real) = self.static_alias.get(&name) {
                    // * El UNICO sitio donde un identificador se vuelve
                    // variable, y por eso el unico que hace falta tocar para
                    // que las `static` locales funcionen. Si hubiera dos
                    // caminos, uno se quedaria sin traducir y el bug seria
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
