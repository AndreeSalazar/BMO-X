//! **Parser de BMO C++** — tokens a AST, por descenso recursivo.
//!
//! ═══ La decisión más cara de deshacer, tomada aquí ═══
//!
//! > **El parser y la tabla de símbolos se hablan.**
//!
//! No es una preferencia de estilo: **C++ no se puede parsear sin resolver
//! nombres a la vez**, y conviene tenerlo escrito antes de la primera línea en
//! vez de descubrirlo en el paso 6. Cuatro sitios donde la gramática se muerde
//! la cola, con lo que hace este fichero en cada uno:
//!
//! 1. **`a<b>(c)`** — ¿instanciar la plantilla `a` con `b` y llamarla, o
//!    `(a<b)>(c)`, dos comparaciones? **Depende de si `a` es un nombre de
//!    plantilla**, que sólo lo sabe la tabla. Hoy `plantillas` está vacío y
//!    todo `<` es comparación; el paso 6 puebla el conjunto y esta rama pasa a
//!    decidir. El punto de decisión ya existe: [`Parser::es_plantilla`].
//! 2. **El *most vexing parse*** — `T x(y);` ¿declara `x` o declara una
//!    función? El estándar dice **si puede ser declaración, es declaración**, y
//!    aquí se implementa a propósito en [`Parser::parece_declaracion`].
//! 3. **Sentencia-declaración vs sentencia-expresión** — `T(x);` otra vez las
//!    dos cosas. Mismo desempate.
//! 4. **`>>` en plantillas** — `Vector<Vector<int>>` es dos cierres, no un
//!    desplazamiento. Se arregla **en el parser** partiendo el token, nunca en
//!    el lexer: en el lexer no hay contexto para saber cuál de los dos es.
//!    Llega con el paso 6; el sitio está marcado.
//!
//! ═══ La regla ═══
//!
//! > Lo que no se sabe leer se **RECHAZA diciendo qué se esperaba**. Nunca en
//! > silencio.
//!
//! El parser anterior hacía `pos += 1` con lo que no reconocía, así que un
//! cuerpo entero podía desaparecer y el programa "compilaba". Aquí no hay
//! ninguna rama que descarte tokens.

use crate::ast::*;
use crate::lexer::{tokenizar, Token};
use crate::CppError;
use std::collections::{HashMap, HashSet};

pub fn parse(fuente: &str) -> Result<Program, CppError> {
    let lex = tokenizar(fuente);
    if let Some(e) = lex.errores.into_iter().next() {
        return Err(e);
    }
    Parser::nuevo(lex.toks, lex.lineas).programa()
}

// ── La tabla de símbolos ────────────────────────────────────────────

/// Ámbitos anidados: el de dentro tapa al de fuera, y al salir se descarta.
///
/// Guarda **el tipo** de cada nombre, no sólo que exista, porque el parser lo
/// necesita para tres cosas concretas: resolver `auto`, calcular la escala de
/// un `v[i]`, y saber si un identificador es un tipo o un valor.
#[derive(Default)]
struct Ambitos {
    pila: Vec<HashMap<String, TypeSpec>>,
}

impl Ambitos {
    fn entrar(&mut self) { self.pila.push(HashMap::new()); }
    fn salir(&mut self) { self.pila.pop(); }
    fn declarar(&mut self, n: &str, t: TypeSpec) {
        if let Some(a) = self.pila.last_mut() { a.insert(n.to_string(), t); }
    }
    fn tipo(&self, n: &str) -> Option<&TypeSpec> {
        self.pila.iter().rev().find_map(|a| a.get(n))
    }
}

// ── El parser ───────────────────────────────────────────────────────

/// Lo que el parser sabe de una clase: es lo que hace falta para resolver
/// `p.x` sin volver a mirar la declaración.
#[derive(Clone)]
struct Clase {
    /// *(nombre, offset, tipo)* en orden de declaración.
    campos: Vec<(String, u32, TypeSpec)>,
    metodos: HashSet<String>,
    // El TAMAÑO no está aquí a propósito: el parser no lo necesita para nada
    // —resolver `p.x` sólo pide offset y tipo— y viaja en `Class::size`, que
    // es donde lo leerá `new P()` en el paso 3. Guardar una copia que nadie
    // lee es exactamente la clase de dato que se queda obsoleto en silencio.
}

impl Clase {
    fn campo(&self, n: &str) -> Option<&(String, u32, TypeSpec)> {
        self.campos.iter().find(|(c, _, _)| c == n)
    }
}

struct Parser {
    toks: Vec<Token>,
    lineas: Vec<usize>,
    pos: usize,
    ambitos: Ambitos,
    /// Nombres de función conocidos. Sólo informativo hoy; el paso 4 lo usa
    /// para la resolución de sobrecarga.
    funciones: HashSet<String>,
    /// ★ Nombres de plantilla. **Vacío hasta el paso 6** — y mientras esté
    /// vacío, todo `<` es una comparación. Ver [`Parser::es_plantilla`].
    plantillas: HashSet<String>,
    /// Las clases vistas. Es lo que hace que `P` sea un tipo y no un nombre.
    clases: HashMap<String, Clase>,
    /// La clase cuyo método se está parseando, si alguno. Es lo que le da
    /// sentido a `this` y a un campo nombrado a secas dentro de un método.
    clase_actual: Option<String>,
}

impl Parser {
    fn nuevo(toks: Vec<Token>, lineas: Vec<usize>) -> Self {
        let mut p = Self {
            toks, lineas, pos: 0,
            ambitos: Ambitos::default(),
            funciones: HashSet::new(),
            plantillas: HashSet::new(),
            clases: HashMap::new(),
            clase_actual: None,
        };
        p.ambitos.entrar(); // ámbito de fichero
        p
    }

    fn peek(&self) -> &Token { &self.toks[self.pos.min(self.toks.len() - 1)] }
    fn peek_en(&self, n: usize) -> &Token { &self.toks[(self.pos + n).min(self.toks.len() - 1)] }
    fn linea(&self) -> usize { self.lineas[self.pos.min(self.lineas.len() - 1)] }
    fn avanzar(&mut self) -> Token {
        let t = self.toks[self.pos.min(self.toks.len() - 1)].clone();
        if self.pos < self.toks.len() - 1 { self.pos += 1; }
        t
    }
    fn come(&mut self, t: &Token) -> bool {
        if self.peek() == t { self.avanzar(); true } else { false }
    }
    fn exige(&mut self, t: &Token) -> Result<(), CppError> {
        if self.come(t) { return Ok(()); }
        Err(self.err(format!("se esperaba {t:?} y vino {:?}", self.peek())))
    }
    fn err(&self, m: impl Into<String>) -> CppError { CppError::new(self.linea(), m) }
    fn pendiente(&self, que: &str, paso: u8) -> CppError {
        self.err(format!(
            "{que}: llega en el PASO {paso}. El orden completo esta en \
             toolchain/lang/cpp/BRECHA.md"
        ))
    }

    /// ★ El punto de decisión de `a<b>(c)`, aislado a propósito.
    ///
    /// Hoy siempre devuelve `false` —no hay plantillas— y por tanto todo `<`
    /// es una comparación, que es lo correcto para el lenguaje que hay. El
    /// paso 6 puebla `plantillas` y esta función pasa a partir la gramática en
    /// dos sin tocar nada más.
    #[allow(dead_code)]
    fn es_plantilla(&self, nombre: &str) -> bool {
        self.plantillas.contains(nombre)
    }

    // ── Nivel de fichero ────────────────────────────────────────────

    fn programa(&mut self) -> Result<Program, CppError> {
        let mut p = Program::new();
        loop {
            match self.peek() {
                Token::Eof => break,
                Token::Hash => return Err(self.pendiente(
                    "las directivas del preprocesador (`#include`, `#define`)", 1)),
                Token::Class | Token::Struct => {
                    let c = self.clase()?;
                    p.classes.push(c);
                }
                Token::Namespace => return Err(self.pendiente("los namespaces", 4)),
                Token::Template => return Err(self.pendiente("las plantillas", 6)),
                Token::Using => return Err(self.pendiente("`using`", 4)),
                Token::Enum => return Err(self.pendiente("`enum`", 4)),
                Token::Semicolon => { self.avanzar(); }
                _ => self.declaracion_de_fichero(&mut p)?,
            }
        }
        Ok(p)
    }

    /// Una función o una variable global. Se distinguen por lo que hay
    /// **detrás del nombre**: un paréntesis es una función.
    fn declaracion_de_fichero(&mut self, p: &mut Program) -> Result<(), CppError> {
        self.come(&Token::Static);
        self.come(&Token::Const);
        let base = self.tipo_base()?;
        let (tipo, nombre) = self.declarador(base)?;

        if *self.peek() == Token::OpenParen {
            self.avanzar();
            let params = self.parametros()?;
            self.exige(&Token::CloseParen)?;
            self.come(&Token::Const);

            if self.come(&Token::Semicolon) {
                // Prototipo. Se registra el nombre y no se emite nada: sirve
                // para que una llamada anterior a la definición no parezca una
                // variable, que es lo que desbloquea la recursión mutua.
                self.funciones.insert(nombre);
                return Ok(());
            }

            self.funciones.insert(nombre.clone());
            self.ambitos.entrar();
            for pa in &params { self.ambitos.declarar(&pa.name, pa.typ.clone()); }
            let cuerpo = self.bloque()?;
            self.ambitos.salir();
            p.functions.push(Function { ret_type: tipo, name: nombre, params, body: cuerpo });
            return Ok(());
        }

        // Variable global.
        let init = if self.come(&Token::Assign) { Some(self.asignacion()?) } else { None };
        self.exige(&Token::Semicolon)?;
        self.ambitos.declarar(&nombre, tipo.clone());
        p.globals.push(GlobalDecl::Var(tipo, nombre, init));
        Ok(())
    }

    // ── Clases ──────────────────────────────────────────────────────

    /// `class P { public: int x; int doble() { … } };`
    ///
    /// ★ Se parsea en **dos vueltas**, y no por gusto: un método puede usar un
    /// campo declarado **más abajo** en la clase —eso es legal en C++ y no lo
    /// es en C— así que la disposición tiene que estar completa antes de mirar
    /// un solo cuerpo. Primero se recogen las firmas y los campos; después se
    /// parsean los cuerpos con la clase ya registrada.
    fn clase(&mut self) -> Result<Class, CppError> {
        let es_struct = *self.peek() == Token::Struct;
        self.avanzar();
        let nombre = match self.avanzar() {
            Token::Ident(n) => n,
            otro => return Err(self.err(format!("se esperaba el nombre de la clase y vino {otro:?}"))),
        };
        if self.come(&Token::Colon) {
            return Err(self.pendiente("la herencia", 5));
        }
        self.exige(&Token::OpenBrace)?;

        // ── Vuelta 1: campos y firmas ──
        let mut campos: Vec<MemberVar> = Vec::new();
        let mut cuerpos: Vec<(usize, Method)> = Vec::new(); // (posición del `{`, firma)
        let mut ctor: Option<(usize, Method)> = None;
        let mut dtor: Option<(usize, Method)> = None;
        let mut acceso = if es_struct { Access::Public } else { Access::Private };

        while *self.peek() != Token::CloseBrace {
            match self.peek().clone() {
                Token::Eof => return Err(self.err("se acabo el fichero dentro de una clase")),
                Token::Public => { self.avanzar(); self.exige(&Token::Colon)?; acceso = Access::Public; continue; }
                Token::Private => { self.avanzar(); self.exige(&Token::Colon)?; acceso = Access::Private; continue; }
                Token::Protected => { self.avanzar(); self.exige(&Token::Colon)?; acceso = Access::Protected; continue; }
                Token::Semicolon => { self.avanzar(); continue; }
                Token::Virtual => return Err(self.pendiente("las funciones virtuales", 5)),
                Token::Friend => return Err(self.pendiente("`friend`", 4)),
                Token::Static => return Err(self.pendiente("los miembros `static`", 4)),
                Token::Operator => return Err(self.pendiente("la sobrecarga de operadores", 4)),

                // ── Destructor: `~P() { … }` ──
                Token::Tilde => {
                    self.avanzar();
                    match self.avanzar() {
                        Token::Ident(n) if n == nombre => {}
                        otro => return Err(self.err(format!(
                            "el destructor de `{nombre}` se llama `~{nombre}`, no {otro:?}"))),
                    }
                    self.exige(&Token::OpenParen)?;
                    if *self.peek() != Token::CloseParen {
                        return Err(self.err("un destructor no lleva parametros"));
                    }
                    self.avanzar();
                    if dtor.is_some() {
                        return Err(self.err(format!("`{nombre}` ya tiene destructor")));
                    }
                    let inicio = self.pos;
                    self.saltar_bloque()?;
                    dtor = Some((inicio, Method {
                        name: format!("~{nombre}"), ret_type: TypeSpec::Void, params: vec![],
                        body: Vec::new(), is_virtual: false, is_override: false,
                        is_const: false, access: Access::Public, class_name: nombre.clone(),
                    }));
                    continue;
                }

                // ── Constructor: `P(…) { … }` — el nombre de la clase
                //    seguido de paréntesis, y SIN tipo de retorno delante.
                Token::Ident(n) if n == nombre && *self.peek_en(1) == Token::OpenParen => {
                    self.avanzar();
                    self.avanzar();
                    let params = self.parametros()?;
                    self.exige(&Token::CloseParen)?;
                    // `P() : x(0) {}` — la lista de inicialización de miembros.
                    // No entra todavía: pide resolver un inicializador POR
                    // MIEMBRO en el orden de declaración (que no es el orden en
                    // que se escriben), y ése es trabajo del paso 4. Mientras
                    // tanto el cuerpo `{ x = 0; }` hace lo mismo.
                    if *self.peek() == Token::Colon {
                        return Err(self.pendiente(
                            "la lista de inicializacion de miembros (`P() : x(0)`)", 4));
                    }
                    if ctor.is_some() {
                        return Err(self.pendiente(
                            "mas de un constructor (sobrecarga)", 4));
                    }
                    let inicio = self.pos;
                    self.saltar_bloque()?;
                    ctor = Some((inicio, Method {
                        name: nombre.clone(), ret_type: TypeSpec::Void, params,
                        body: Vec::new(), is_virtual: false, is_override: false,
                        is_const: false, access: acceso, class_name: nombre.clone(),
                    }));
                    continue;
                }
                _ => {}
            }

            let base = self.tipo_base()?;
            // `int operator+(int)` — el `operator` viene DETRÁS del tipo, así
            // que no lo caza la criba de arriba.
            if *self.peek() == Token::Operator {
                return Err(self.pendiente("la sobrecarga de operadores", 4));
            }
            let (tipo, miembro) = self.declarador(base)?;

            if *self.peek() == Token::OpenParen {
                self.avanzar();
                let params = self.parametros()?;
                self.exige(&Token::CloseParen)?;
                let es_const = self.come(&Token::Const);
                if self.come(&Token::Semicolon) {
                    return Err(self.pendiente("declarar un metodo y definirlo fuera de la clase", 4));
                }
                // Se anota dónde empieza el cuerpo y se salta: se parseará en
                // la vuelta 2, cuando la disposición esté completa.
                let inicio = self.pos;
                self.saltar_bloque()?;
                cuerpos.push((inicio, Method {
                    name: miembro, ret_type: tipo, params, body: Vec::new(),
                    is_virtual: false, is_override: false, is_const: es_const,
                    access: acceso, class_name: nombre.clone(),
                }));
            } else {
                self.exige(&Token::Semicolon)?;
                campos.push(MemberVar { typ: tipo, name: miembro, offset: 0, access: acceso });
            }
        }
        self.avanzar(); // `}`
        self.exige(&Token::Semicolon)?;

        // ── La disposición ──
        //
        // La regla NO está escrita aquí: vive una sola vez en
        // `bmo_abi::types::disposicion`, con sus tests. El parser de C++ tiene
        // que calcularla igualmente —los nodos `Field` de C llevan el offset
        // dentro— pero calcula con la MISMA regla que el codegen de C, que es
        // lo que importa.
        //
        // Y el descenso emite el `struct` con miembros y no con offsets, así
        // que el codegen recalcula por su cuenta: si algún día las dos
        // llamadas dieran cosas distintas, la fila `clase-disposicion` de la
        // matriz se pone roja en vez de pasar muda.
        let mut layout = Vec::new();
        let mut d = bmo_abi::types::Disposicion::nueva();
        for m in &campos {
            layout.push((m.name.clone(), d.coloca(m.typ.size()), m.typ.clone()));
        }
        let tam = d.total();

        let info = Clase {
            campos: layout.clone(),
            metodos: cuerpos.iter().map(|(_, m)| m.name.clone()).collect(),
        };
        self.clases.insert(nombre.clone(), info);

        // ── Vuelta 2: los cuerpos, con la clase ya registrada ──
        let vuelta = self.pos;
        let mut cuerpo_de = |p: &mut Self, inicio: usize, m: &mut Method| -> Result<(), CppError> {
            p.pos = inicio;
            p.clase_actual = Some(nombre.clone());
            p.ambitos.entrar();
            p.ambitos.declarar("this", TypeSpec::Ptr(Box::new(TypeSpec::ClassRef(nombre.clone()))));
            for pa in &m.params { p.ambitos.declarar(&pa.name, pa.typ.clone()); }
            m.body = p.bloque()?;
            p.ambitos.salir();
            p.clase_actual = None;
            Ok(())
        };
        let mut metodos = Vec::new();
        for (inicio, mut m) in cuerpos {
            cuerpo_de(self, inicio, &mut m)?;
            metodos.push(m);
        }
        let constructor = match ctor {
            Some((inicio, mut m)) => { cuerpo_de(self, inicio, &mut m)?; Some(m) }
            None => None,
        };
        let destructor = match dtor {
            Some((inicio, mut m)) => { cuerpo_de(self, inicio, &mut m)?; Some(m) }
            None => None,
        };
        self.pos = vuelta;

        let mut miembros = Vec::new();
        for (m, (_, o, _)) in campos.into_iter().zip(layout.iter()) {
            miembros.push(MemberVar { offset: *o, ..m });
        }

        Ok(Class {
            name: nombre, bases: Vec::new(), members: miembros, methods: metodos,
            constructor, destructor, vtable: false, size: tam,
        })
    }

    /// Salta un bloque `{ … }` contando llaves, sin interpretarlo.
    fn saltar_bloque(&mut self) -> Result<(), CppError> {
        self.exige(&Token::OpenBrace)?;
        let mut hondo = 1;
        while hondo > 0 {
            match self.avanzar() {
                Token::OpenBrace => hondo += 1,
                Token::CloseBrace => hondo -= 1,
                Token::Eof => return Err(self.err("se acabo el fichero dentro de un metodo")),
                _ => {}
            }
        }
        Ok(())
    }

    /// El tipo de una expresión, para resolver `.` y `->`.
    ///
    /// Sólo cubre lo que puede estar a la izquierda de un punto — que es poco
    /// a propósito: en cuanto cubriera de más, sería un comprobador de tipos, y
    /// eso no es lo que el paso 2 promete.
    fn tipo_de(&self, e: &Expr) -> Option<TypeSpec> {
        match e {
            Expr::Var(n) => self.ambitos.tipo(n).cloned(),
            Expr::This => self.clase_actual.clone()
                .map(|c| TypeSpec::Ptr(Box::new(TypeSpec::ClassRef(c)))),
            Expr::MemberAccess(_, _, _, t) | Expr::Arrow(_, _, _, t) => Some(t.clone()),
            Expr::Deref(b) => match self.tipo_de(b)? {
                TypeSpec::Ptr(t) => Some(*t),
                _ => None,
            },
            _ => None,
        }
    }

    /// La clase a la que se le puede pedir un campo, dado el tipo de la base
    /// y si el acceso fue con `.` o con `->`.
    fn clase_de(&self, t: &TypeSpec, flecha: bool) -> Option<String> {
        match (t, flecha) {
            (TypeSpec::ClassRef(n), false) => Some(n.clone()),
            (TypeSpec::Ptr(b), true) => match &**b {
                TypeSpec::ClassRef(n) => Some(n.clone()),
                _ => None,
            },
            _ => None,
        }
    }

    fn parametros(&mut self) -> Result<Vec<Param>, CppError> {
        let mut out = Vec::new();
        if *self.peek() == Token::CloseParen { return Ok(out); }
        // `f(void)` es `f()`.
        if *self.peek() == Token::Void && *self.peek_en(1) == Token::CloseParen {
            self.avanzar();
            return Ok(out);
        }
        loop {
            if *self.peek() == Token::Puntos {
                return Err(self.pendiente("los argumentos variadicos (`...`)", 4));
            }
            self.come(&Token::Const);
            let base = self.tipo_base()?;
            // El nombre del parámetro es opcional: `int f(int);` es legal.
            let (tipo, nombre) = if matches!(self.peek(), Token::Ident(_))
                || matches!(self.peek(), Token::Star | Token::And)
            {
                self.declarador(base)?
            } else {
                (base, String::new())
            };
            let defecto = if self.come(&Token::Assign) {
                return Err(self.pendiente("los argumentos por defecto", 4));
            } else { None };
            out.push(Param { typ: tipo, name: nombre, default: defecto });
            if !self.come(&Token::Comma) { break; }
        }
        Ok(out)
    }

    // ── Tipos ───────────────────────────────────────────────────────

    fn tipo_base(&mut self) -> Result<TypeSpec, CppError> {
        self.come(&Token::Const);
        let t = match self.peek().clone() {
            Token::Void => { self.avanzar(); TypeSpec::Void }
            Token::Bool => { self.avanzar(); TypeSpec::Bool }
            Token::Char => { self.avanzar(); TypeSpec::Char }
            Token::Short => { self.avanzar(); self.come(&Token::Int); TypeSpec::Short }
            Token::Int => { self.avanzar(); TypeSpec::Int }
            Token::Float => { self.avanzar(); TypeSpec::Float }
            Token::Double => { self.avanzar(); TypeSpec::Double }
            Token::Long => {
                self.avanzar();
                let t = if self.come(&Token::Long) { TypeSpec::LongLong } else { TypeSpec::Long };
                self.come(&Token::Int);
                t
            }
            Token::Signed => { self.avanzar(); self.tipo_sin_signo(false)? }
            Token::Unsigned => { self.avanzar(); self.tipo_sin_signo(true)? }
            Token::Auto => return Err(self.pendiente("`auto`", 2)),
            Token::Ident(n) => {
                if self.clases.contains_key(&n) { self.avanzar(); TypeSpec::ClassRef(n) }
                else { return Err(self.err(format!("`{n}` no es un tipo conocido"))); }
            }
            otro => return Err(self.err(format!("se esperaba un tipo y vino {otro:?}"))),
        };
        self.come(&Token::Const);
        Ok(t)
    }

    fn tipo_sin_signo(&mut self, sin_signo: bool) -> Result<TypeSpec, CppError> {
        let t = match self.peek().clone() {
            Token::Char => { self.avanzar(); if sin_signo { TypeSpec::UnsignedChar } else { TypeSpec::Char } }
            Token::Short => { self.avanzar(); self.come(&Token::Int);
                if sin_signo { TypeSpec::UnsignedShort } else { TypeSpec::Short } }
            Token::Long => { self.avanzar();
                let doble = self.come(&Token::Long);
                self.come(&Token::Int);
                match (sin_signo, doble) {
                    (true, true) => TypeSpec::UnsignedLongLong,
                    (true, false) => TypeSpec::UnsignedLong,
                    (false, true) => TypeSpec::LongLong,
                    (false, false) => TypeSpec::Long,
                } }
            _ => { self.come(&Token::Int); if sin_signo { TypeSpec::UnsignedInt } else { TypeSpec::Int } }
        };
        Ok(t)
    }

    /// ★ **El asterisco es del DECLARADOR, no del tipo base.**
    ///
    /// En `int *a, b;` la `b` es un `int`. Es un bug que BMO C ya pagó una vez
    /// —guardaba como base el tipo *ya con punteros*— y por eso aquí el tipo
    /// base se pasa **por valor** a cada declarador: cada uno se lleva su
    /// copia y le añade lo suyo.
    fn declarador(&mut self, base: TypeSpec) -> Result<(TypeSpec, String), CppError> {
        let mut t = base;
        loop {
            if self.come(&Token::Star) { t = TypeSpec::Ptr(Box::new(t)); self.come(&Token::Const); }
            else if self.come(&Token::And) { t = TypeSpec::Ref(Box::new(t)); }
            else { break; }
        }
        let nombre = match self.avanzar() {
            Token::Ident(n) => n,
            otro => return Err(self.err(format!("se esperaba un nombre y vino {otro:?}"))),
        };
        // `v[n]` — el corchete es del declarador, igual que el asterisco.
        while self.come(&Token::OpenBracket) {
            let n = match self.avanzar() {
                Token::IntLit(v) if v > 0 => v as u32,
                otro => return Err(self.err(format!(
                    "el tamano de un array tiene que ser un entero positivo, vino {otro:?}"))),
            };
            self.exige(&Token::CloseBracket)?;
            t = TypeSpec::Array(Box::new(t), n);
        }
        Ok((t, nombre))
    }

    /// ¿Lo que viene es una declaración?
    ///
    /// ★ Aquí vive el ***most vexing parse***: `T x(y);` puede leerse como una
    /// variable `x` inicializada con `y`, o como la declaración de una función
    /// `x` que toma un `y`. El estándar zanja que **si puede ser declaración,
    /// es declaración** — y como aquí lo único que puede empezar una
    /// declaración es una palabra clave de tipo, la regla sale sola: si el
    /// token es un tipo, es declaración; si no, es expresión.
    ///
    /// El día que un identificador pueda ser un tipo (paso 2, clases), esta
    /// función es el único sitio que cambia — y necesitará la tabla de
    /// símbolos, que ya está aquí.
    fn parece_declaracion(&self) -> bool {
        if matches!(self.peek(),
            Token::Void | Token::Bool | Token::Char | Token::Short | Token::Int
            | Token::Long | Token::Float | Token::Double | Token::Unsigned
            | Token::Signed | Token::Const | Token::Static | Token::Auto)
        {
            return true;
        }
        // ★★ **`P *q` es una declaración o una multiplicación, y sólo la
        // tabla de símbolos lo sabe.**
        //
        // Éste es el caso que justifica, él solo, que el parser y la tabla se
        // hablen. `P * q` con `P` desconocida es el producto de dos variables;
        // con `P` declarada como clase es *"puntero a P llamado q"*. La misma
        // secuencia de tokens, dos árboles distintos, y **las dos compilan**:
        // si se elige mal, el programa hace otra cosa sin quejarse.
        //
        // Es el hermano pequeño de `a<b>(c)` (ver `MAESTROS.md`), y llegó en
        // cuanto existieron las clases — antes de lo previsto, porque no hace
        // falta una plantilla para que C++ muerda.
        let Token::Ident(n) = self.peek() else { return false };
        if !self.clases.contains_key(n) {
            // Dos identificadores seguidos sólo pueden ser `Tipo nombre`. Se
            // reconoce aunque el tipo no exista, para que el error sea
            // *"`P` no es un tipo conocido"* y no *"se esperaba `;`"*, que
            // manda a mirar la puntuación.
            return matches!(self.peek_en(1), Token::Ident(_));
        }
        matches!(self.peek_en(1), Token::Ident(_) | Token::Star | Token::And)
    }

    // ── Sentencias ──────────────────────────────────────────────────

    fn bloque(&mut self) -> Result<Vec<Stmt>, CppError> {
        self.exige(&Token::OpenBrace)?;
        self.ambitos.entrar();
        let mut out = Vec::new();
        while *self.peek() != Token::CloseBrace {
            if *self.peek() == Token::Eof {
                self.ambitos.salir();
                return Err(self.err("se acabo el fichero dentro de un bloque: falta `}`"));
            }
            out.push(self.sentencia()?);
        }
        self.avanzar();
        self.ambitos.salir();
        Ok(out)
    }

    fn sentencia(&mut self) -> Result<Stmt, CppError> {
        match self.peek().clone() {
            Token::OpenBrace => Ok(Stmt::Block(self.bloque()?)),
            Token::Semicolon => { self.avanzar(); Ok(Stmt::Block(vec![])) }
            Token::Return => {
                self.avanzar();
                if self.come(&Token::Semicolon) { return Ok(Stmt::Return(None)); }
                let e = self.expresion()?;
                self.exige(&Token::Semicolon)?;
                Ok(Stmt::Return(Some(e)))
            }
            Token::Break => { self.avanzar(); self.exige(&Token::Semicolon)?; Ok(Stmt::Break) }
            Token::Continue => { self.avanzar(); self.exige(&Token::Semicolon)?; Ok(Stmt::Continue) }
            Token::If => self.si(),
            Token::While => self.mientras(),
            Token::Do => self.hacer(),
            Token::For => self.para(),
            Token::Switch => self.segun(),
            Token::Delete => {
                self.avanzar();
                self.come(&Token::OpenBracket);
                self.come(&Token::CloseBracket);
                let n = match self.avanzar() {
                    Token::Ident(n) => n,
                    otro => return Err(self.err(format!("`delete` de {otro:?}"))),
                };
                self.exige(&Token::Semicolon)?;
                Ok(Stmt::Delete(n))
            }
            Token::Hash => Err(self.pendiente("las directivas del preprocesador", 1)),
            Token::Class | Token::Struct => Err(self.pendiente("las clases", 2)),
            _ if self.parece_declaracion() => self.declaracion_local(),
            _ => {
                let e = self.expresion()?;
                self.exige(&Token::Semicolon)?;
                Ok(Stmt::Expr(e))
            }
        }
    }

    /// `T a = 1, b;` — el tipo base se comparte, cada declarador trae lo suyo.
    fn declaracion_local(&mut self) -> Result<Stmt, CppError> {
        self.come(&Token::Static);
        let base = self.tipo_base()?;
        let mut decls = Vec::new();
        loop {
            let (tipo, nombre) = self.declarador(base.clone())?;
            // ★ El inicializador es una `assignment-expression`, NO una
            // `expression`. Con la coma completa, `int a = 20, b = 22;` se
            // leería `a = (20, b = 22)` usando el operador coma. El escalón de
            // la gramática existe exactamente para esto — es un bug que BMO C
            // ya pagó.
            let init = if self.come(&Token::Assign) { Some(self.asignacion()?) } else { None };
            self.ambitos.declarar(&nombre, tipo.clone());
            decls.push(Stmt::DeclVar(tipo, nombre, init));
            if !self.come(&Token::Comma) { break; }
        }
        self.exige(&Token::Semicolon)?;
        if decls.len() == 1 { Ok(decls.pop().unwrap()) } else { Ok(Stmt::Block(decls)) }
    }

    fn si(&mut self) -> Result<Stmt, CppError> {
        self.avanzar();
        self.exige(&Token::OpenParen)?;
        let cond = self.expresion()?;
        self.exige(&Token::CloseParen)?;
        let entonces = Box::new(self.sentencia()?);
        let si_no = if self.come(&Token::Else) { Some(Box::new(self.sentencia()?)) } else { None };
        Ok(Stmt::If(cond, entonces, si_no))
    }

    fn mientras(&mut self) -> Result<Stmt, CppError> {
        self.avanzar();
        self.exige(&Token::OpenParen)?;
        let cond = self.expresion()?;
        self.exige(&Token::CloseParen)?;
        Ok(Stmt::While(cond, Box::new(self.sentencia()?)))
    }

    fn hacer(&mut self) -> Result<Stmt, CppError> {
        self.avanzar();
        let cuerpo = Box::new(self.sentencia()?);
        self.exige(&Token::While)?;
        self.exige(&Token::OpenParen)?;
        let cond = self.expresion()?;
        self.exige(&Token::CloseParen)?;
        self.exige(&Token::Semicolon)?;
        Ok(Stmt::DoWhile(cuerpo, cond))
    }

    /// `for(T i = 0; …)` se desazucara a `{ T i = 0; for(; …) cuerpo }`.
    ///
    /// Es lo mismo que hace el parser de C, y por el mismo motivo: el nodo
    /// `For` lleva una **expresión** en el init, y una declaración no es una
    /// expresión. Envolver en un bloque además le da a `i` el ámbito correcto.
    fn para(&mut self) -> Result<Stmt, CppError> {
        self.avanzar();
        self.exige(&Token::OpenParen)?;
        self.ambitos.entrar();

        let decl = if self.parece_declaracion() {
            let base = self.tipo_base()?;
            let (tipo, nombre) = self.declarador(base)?;
            let init = if self.come(&Token::Assign) { Some(self.asignacion()?) } else { None };
            self.exige(&Token::Semicolon)?;
            self.ambitos.declarar(&nombre, tipo.clone());
            Some(Stmt::DeclVar(tipo, nombre, init))
        } else {
            let e = if *self.peek() == Token::Semicolon { None } else { Some(self.expresion()?) };
            self.exige(&Token::Semicolon)?;
            if let Some(e) = e {
                let cond = if *self.peek() == Token::Semicolon { None } else { Some(self.expresion()?) };
                self.exige(&Token::Semicolon)?;
                let inc = if *self.peek() == Token::CloseParen { None } else { Some(self.expresion()?) };
                self.exige(&Token::CloseParen)?;
                let cuerpo = self.sentencia()?;
                self.ambitos.salir();
                return Ok(Stmt::For(Some(e), cond, inc, Box::new(cuerpo)));
            }
            None
        };

        let cond = if *self.peek() == Token::Semicolon { None } else { Some(self.expresion()?) };
        self.exige(&Token::Semicolon)?;
        let inc = if *self.peek() == Token::CloseParen { None } else { Some(self.expresion()?) };
        self.exige(&Token::CloseParen)?;
        let cuerpo = self.sentencia()?;
        self.ambitos.salir();

        let bucle = Stmt::For(None, cond, inc, Box::new(cuerpo));
        Ok(match decl {
            Some(d) => Stmt::Block(vec![d, bucle]),
            None => bucle,
        })
    }

    fn segun(&mut self) -> Result<Stmt, CppError> {
        self.avanzar();
        self.exige(&Token::OpenParen)?;
        let sujeto = self.expresion()?;
        self.exige(&Token::CloseParen)?;
        self.exige(&Token::OpenBrace)?;
        let mut casos: Vec<Case> = Vec::new();
        while *self.peek() != Token::CloseBrace {
            if *self.peek() == Token::Eof {
                return Err(self.err("se acabo el fichero dentro de un `switch`"));
            }
            let valor = if self.come(&Token::Case) {
                let v = match self.avanzar() {
                    Token::IntLit(v) => v,
                    Token::CharLit(c) => c as i64,
                    otro => return Err(self.err(format!(
                        "un `case` pide un entero constante, vino {otro:?}"))),
                };
                self.exige(&Token::Colon)?;
                Some(v)
            } else if self.come(&Token::Default) {
                self.exige(&Token::Colon)?;
                None
            } else if let Some(ultimo) = casos.last_mut() {
                // Cuerpo del case anterior.
                let s = self.sentencia()?;
                ultimo.stmts.push(s);
                continue;
            } else {
                return Err(self.err("dentro de un `switch` lo primero tiene que ser `case` o `default`"));
            };
            casos.push(Case { value: valor, stmts: Vec::new() });
        }
        self.avanzar();
        Ok(Stmt::Switch(sujeto, casos))
    }

    // ── Expresiones, por precedencia ────────────────────────────────
    //
    // De menor a mayor: coma → asignación → ternario → || → && → | → ^ → &
    // → ==/!= → </>/<=/>= → <</>> → +/- → */÷/% → unario → sufijo → primario.
    //
    // Cada nivel es una función y llama al de más arriba. Es la escalera de la
    // gramática de C tal cual: no está aquí por copiarla, está porque **el
    // escalón de la asignación es lo único que impide que `int a = 20, b = 22`
    // se lea con el operador coma**.

    fn expresion(&mut self) -> Result<Expr, CppError> {
        let e = self.asignacion()?;
        // El operador coma no está en el AST de C++ todavía. En vez de
        // tragárselo (que daría el valor equivocado en silencio), se dice.
        if *self.peek() == Token::Comma {
            return Err(self.pendiente("el operador coma en una expresion", 4));
        }
        Ok(e)
    }

    fn asignacion(&mut self) -> Result<Expr, CppError> {
        let izq = self.ternario()?;
        let compuesta = |op: fn(Box<Expr>, Box<Expr>) -> Expr| op;
        let op: Option<Option<fn(Box<Expr>, Box<Expr>) -> Expr>> = match self.peek() {
            Token::Assign => Some(None),
            Token::AddAssign => Some(Some(compuesta(Expr::Add))),
            Token::SubAssign => Some(Some(compuesta(Expr::Sub))),
            Token::MulAssign => Some(Some(compuesta(Expr::Mul))),
            Token::DivAssign => Some(Some(compuesta(Expr::Div))),
            Token::ModAssign => Some(Some(compuesta(Expr::Mod))),
            Token::AndAssign => Some(Some(compuesta(Expr::BitAnd))),
            Token::OrAssign => Some(Some(compuesta(Expr::BitOr))),
            Token::XorAssign => Some(Some(compuesta(Expr::BitXor))),
            Token::ShlAssign => Some(Some(compuesta(Expr::Shl))),
            Token::ShrAssign => Some(Some(compuesta(Expr::Shr))),
            _ => None,
        };
        let Some(op) = op else { return Ok(izq) };
        self.avanzar();
        // La asignación asocia a la DERECHA: `a = b = c` es `a = (b = c)`.
        let der = self.asignacion()?;
        let valor = |lhs: Expr| match op {
            None => der.clone(),
            Some(f) => f(Box::new(lhs), Box::new(der.clone())),
        };
        match izq {
            Expr::Var(n) => {
                let lhs = Expr::Var(n.clone());
                Ok(Expr::Assign(n, Box::new(valor(lhs))))
            }
            Expr::Subscript(n, idx, sc) => {
                let lhs = Expr::Subscript(n.clone(), idx.clone(), sc);
                Ok(Expr::AssignSubscript(n, idx, sc, Box::new(valor(lhs))))
            }
            Expr::Deref(p) => {
                let lhs = Expr::Deref(p.clone());
                Ok(Expr::AssignDeref(p, Box::new(valor(lhs))))
            }
            Expr::MemberAccess(b, n, off, t) => {
                let lhs = Expr::MemberAccess(b.clone(), n.clone(), off, t.clone());
                Ok(Expr::AssignMember(b, n, off, t, Box::new(valor(lhs))))
            }
            Expr::Arrow(b, n, off, t) => {
                let lhs = Expr::Arrow(b.clone(), n.clone(), off, t.clone());
                Ok(Expr::AssignArrow(b, n, off, t, Box::new(valor(lhs))))
            }
            otro => Err(self.err(format!("esto no se puede asignar: {otro:?}"))),
        }
    }

    fn ternario(&mut self) -> Result<Expr, CppError> {
        let cond = self.binario(0)?;
        if !self.come(&Token::Question) { return Ok(cond); }
        let si = self.asignacion()?;
        self.exige(&Token::Colon)?;
        let no = self.asignacion()?;
        Ok(Expr::Conditional(Box::new(cond), Box::new(si), Box::new(no)))
    }

    /// Los binarios por **escalada de precedencia**: un solo bucle con una
    /// tabla, en vez de nueve funciones que sólo se diferencian en la fila.
    /// Añadir un operador es añadir una fila de [`Self::precedencia`].
    fn binario(&mut self, minima: u8) -> Result<Expr, CppError> {
        let mut izq = self.unario()?;
        loop {
            let Some((prec, constructor)) = Self::precedencia(self.peek()) else { break };
            if prec < minima { break; }
            self.avanzar();
            // Todos asocian a la izquierda: el lado derecho exige precedencia
            // estrictamente mayor.
            let der = self.binario(prec + 1)?;
            izq = constructor(Box::new(izq), Box::new(der));
        }
        Ok(izq)
    }

    fn precedencia(t: &Token) -> Option<(u8, fn(Box<Expr>, Box<Expr>) -> Expr)> {
        Some(match t {
            Token::LOr => (1, Expr::Or),
            Token::LAnd => (2, Expr::And),
            Token::Or => (3, Expr::BitOr),
            Token::Xor => (4, Expr::BitXor),
            Token::And => (5, Expr::BitAnd),
            Token::EqEq => (6, Expr::Eq),
            Token::Neq => (6, Expr::Neq),
            // ★ Aquí es donde el paso 6 tendrá que preguntar a la tabla de
            // símbolos: un `<` detrás de un nombre de plantilla abre una lista
            // de argumentos, no una comparación. Mientras `plantillas` esté
            // vacío, todo `<` es comparación — que es la verdad de hoy.
            Token::Lt => (7, Expr::Lt),
            Token::Gt => (7, Expr::Gt),
            Token::Le => (7, Expr::Le),
            Token::Ge => (7, Expr::Ge),
            Token::Shl => (8, Expr::Shl),
            Token::Shr => (8, Expr::Shr),
            Token::Plus => (9, Expr::Add),
            Token::Minus => (9, Expr::Sub),
            Token::Star => (10, Expr::Mul),
            Token::Slash => (10, Expr::Div),
            Token::Percent => (10, Expr::Mod),
            _ => return None,
        })
    }

    fn unario(&mut self) -> Result<Expr, CppError> {
        match self.peek().clone() {
            Token::Minus => { self.avanzar(); Ok(Expr::Neg(Box::new(self.unario()?))) }
            Token::Plus => { self.avanzar(); self.unario() }
            Token::Not => { self.avanzar(); Ok(Expr::Not(Box::new(self.unario()?))) }
            Token::Tilde => { self.avanzar(); Ok(Expr::BitNot(Box::new(self.unario()?))) }
            Token::Star => { self.avanzar(); Ok(Expr::Deref(Box::new(self.unario()?))) }
            Token::And => { self.avanzar(); Ok(Expr::AddrOf(Box::new(self.unario()?))) }
            Token::PlusPlus => { self.avanzar(); match self.unario()? {
                Expr::Var(n) => Ok(Expr::PreInc(n)),
                _ => Err(self.err("`++` pide una variable")),
            } }
            Token::MinusMinus => { self.avanzar(); match self.unario()? {
                Expr::Var(n) => Ok(Expr::PreDec(n)),
                _ => Err(self.err("`--` pide una variable")),
            } }
            Token::New => Err(self.pendiente("`new`", 3)),
            Token::Sizeof => Err(self.pendiente("`sizeof`", 2)),
            // `(T)e` — una conversión. Se distingue de `(expr)` porque dentro
            // del paréntesis hay una palabra clave de tipo.
            Token::OpenParen if Self::empieza_tipo(self.peek_en(1)) => {
                self.avanzar();
                let base = self.tipo_base()?;
                let mut t = base;
                while self.come(&Token::Star) { t = TypeSpec::Ptr(Box::new(t)); }
                self.exige(&Token::CloseParen)?;
                Ok(Expr::Cast(t, Box::new(self.unario()?)))
            }
            _ => self.sufijo(),
        }
    }

    fn empieza_tipo(t: &Token) -> bool {
        matches!(t, Token::Void | Token::Bool | Token::Char | Token::Short | Token::Int
            | Token::Long | Token::Float | Token::Double | Token::Unsigned | Token::Signed)
    }

    fn sufijo(&mut self) -> Result<Expr, CppError> {
        let mut e = self.primario()?;
        loop {
            match self.peek().clone() {
                Token::OpenBracket => {
                    self.avanzar();
                    let idx = self.expresion()?;
                    self.exige(&Token::CloseBracket)?;
                    let Expr::Var(n) = e else {
                        return Err(self.pendiente("indexar algo que no es una variable", 2));
                    };
                    // La escala sale de la tabla de símbolos: es el tamaño del
                    // ELEMENTO, no el del array.
                    let escala = match self.ambitos.tipo(&n) {
                        Some(TypeSpec::Array(t, _)) => t.size() as u8,
                        Some(TypeSpec::Ptr(t)) => t.size() as u8,
                        Some(otro) => return Err(self.err(format!(
                            "`{n}` no es un array ni un puntero: es {otro:?}"))),
                        None => return Err(self.err(format!("`{n}` no esta declarada"))),
                    };
                    e = Expr::Subscript(n, Box::new(idx), escala);
                }
                Token::OpenParen => {
                    self.avanzar();
                    let mut args = Vec::new();
                    if *self.peek() != Token::CloseParen {
                        loop {
                            args.push(self.asignacion()?);
                            if !self.come(&Token::Comma) { break; }
                        }
                    }
                    self.exige(&Token::CloseParen)?;
                    let Expr::Var(n) = e else {
                        return Err(self.pendiente("llamar a algo que no es un nombre", 2));
                    };
                    // ★ Un método propio llamado a secas ES `this->metodo(…)`.
                    //
                    // El mismo caso que un campo a secas, y con el mismo
                    // desempate: una función libre del mismo nombre tapa al
                    // método sólo si el método no existe. Al revés, una clase
                    // con un método `abs` haría que `abs(x)` llamara al método
                    // desde fuera de la clase.
                    let propio = self.clase_actual.as_ref()
                        .and_then(|c| self.clases.get(c).map(|i| (c.clone(), i.metodos.contains(&n))));
                    e = match propio {
                        Some((cls, true)) => Expr::MethodCall(Box::new(Expr::This), cls, n, args),
                        _ => Expr::Call(n, args),
                    };
                }
                Token::Dot | Token::Arrow => {
                    let flecha = *self.peek() == Token::Arrow;
                    self.avanzar();
                    let miembro = match self.avanzar() {
                        Token::Ident(n) => n,
                        otro => return Err(self.err(format!(
                            "se esperaba el nombre de un miembro y vino {otro:?}"))),
                    };
                    let t = self.tipo_de(&e).ok_or_else(|| self.err(
                        "no se sabe de que tipo es lo que hay antes del punto"))?;
                    let signo = if flecha { "->" } else { "." };
                    let cls = self.clase_de(&t, flecha).ok_or_else(|| self.err(format!(
                        "`{signo}` sobre algo que no es una clase: {t:?}")))?;
                    let info = self.clases.get(&cls).cloned()
                        .ok_or_else(|| self.err(format!("la clase `{cls}` no esta definida")))?;

                    // ¿Método o campo? Se decide con el paréntesis, y el
                    // parser ya sabe cuál de los dos nombres existe.
                    if *self.peek() == Token::OpenParen {
                        if !info.metodos.contains(&miembro) {
                            return Err(self.err(format!("`{cls}` no tiene el metodo `{miembro}`")));
                        }
                        self.avanzar();
                        let mut args = Vec::new();
                        if *self.peek() != Token::CloseParen {
                            loop {
                                args.push(self.asignacion()?);
                                if !self.come(&Token::Comma) { break; }
                            }
                        }
                        self.exige(&Token::CloseParen)?;
                        // El objeto viaja como el `this` que el descenso
                        // pondrá de primer parámetro. Con `->` la base YA es
                        // un puntero; con `.` hay que tomarle la dirección.
                        let objeto = if flecha { e } else { Expr::AddrOf(Box::new(e)) };
                        e = Expr::MethodCall(Box::new(objeto), cls, miembro, args);
                    } else {
                        let (_, off, ft) = info.campo(&miembro).cloned().ok_or_else(|| {
                            self.err(format!("`{cls}` no tiene el campo `{miembro}`"))
                        })?;
                        e = if flecha { Expr::Arrow(Box::new(e), miembro, off, ft) }
                            else { Expr::MemberAccess(Box::new(e), miembro, off, ft) };
                    }
                }
                Token::ColonColon => return Err(self.pendiente("los nombres cualificados con `::`", 4)),
                Token::PlusPlus => { self.avanzar(); match e {
                    Expr::Var(n) => e = Expr::PostInc(n),
                    _ => return Err(self.err("`++` pide una variable")),
                } }
                Token::MinusMinus => { self.avanzar(); match e {
                    Expr::Var(n) => e = Expr::PostDec(n),
                    _ => return Err(self.err("`--` pide una variable")),
                } }
                _ => break,
            }
        }
        Ok(e)
    }

    fn primario(&mut self) -> Result<Expr, CppError> {
        // ★ La línea se captura ANTES de consumir el token. Si se leyera
        // después, `self.pos` ya apunta al siguiente y el error saldría con la
        // línea de lo que viene detrás — que en `int y = ;` es el `return` de
        // la línea siguiente, y manda a mirar donde no es.
        let l = self.linea();
        let culpa = |m: String| CppError::new(l, m);
        match self.avanzar() {
            Token::IntLit(v) => Ok(Expr::Int(v)),
            Token::FloatLit(v) => Ok(Expr::FloatLit(v)),
            Token::StringLit(s) => Ok(Expr::StringLit(s)),
            Token::CharLit(c) => Ok(Expr::CharLit(c)),
            Token::True => Ok(Expr::BoolLit(true)),
            Token::False => Ok(Expr::BoolLit(false)),
            Token::Nullptr => Ok(Expr::NullPtr),
            Token::This => match &self.clase_actual {
                Some(_) => Ok(Expr::This),
                None => Err(CppError::new(l, "`this` fuera de un metodo")),
            },
            // ★ Un campo nombrado a secas dentro de un método ES `this->campo`.
            //
            // Y el orden importa: primero se mira el ámbito local, porque un
            // parámetro o una local **tapan** al campo. Al revés, `int doble(int x)`
            // leería el campo `x` en vez del argumento — y las dos versiones
            // compilan, así que el bug sería mudo.
            Token::Ident(n) => {
                if self.ambitos.tipo(&n).is_none() {
                    if let Some(cls) = self.clase_actual.clone() {
                        if let Some((_, off, t)) = self.clases.get(&cls)
                            .and_then(|c| c.campo(&n)).cloned()
                        {
                            return Ok(Expr::Arrow(Box::new(Expr::This), n, off, t));
                        }
                    }
                }
                Ok(Expr::Var(n))
            }
            Token::OpenParen => {
                let e = self.expresion()?;
                self.exige(&Token::CloseParen)?;
                Ok(e)
            }
            otro => Err(culpa(format!("se esperaba una expresion y vino {otro:?}"))),
        }
    }
}
