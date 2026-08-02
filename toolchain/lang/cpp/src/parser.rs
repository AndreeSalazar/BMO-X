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
}

impl Parser {
    fn nuevo(toks: Vec<Token>, lineas: Vec<usize>) -> Self {
        let mut p = Self {
            toks, lineas, pos: 0,
            ambitos: Ambitos::default(),
            funciones: HashSet::new(),
            plantillas: HashSet::new(),
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
                Token::Class | Token::Struct => return Err(self.pendiente("las clases", 2)),
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
                // Un identificador en posición de tipo sólo puede ser una
                // clase; y las clases llegan en el paso 2. Decirlo así es más
                // útil que "se esperaba un tipo".
                return Err(self.pendiente(&format!("el tipo `{n}` (definido por el usuario)"), 2));
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
        // ★ Dos identificadores seguidos sólo pueden ser `Tipo nombre`.
        //
        // Hoy eso no compila —el tipo tendría que ser una clase, y las clases
        // llegan en el paso 2— pero **hay que reconocerlo igual**: sin esta
        // rama, `P p;` se lee como la expresión `P` y el error sale
        // *"se esperaba `;` y vino `p`"*, que manda a mirar la puntuación en
        // vez de decir lo que pasa. Reconocerlo aquí hace que `tipo_base`
        // conteste lo cierto: **el tipo `P` llega en el paso 2**.
        matches!((self.peek(), self.peek_en(1)), (Token::Ident(_), Token::Ident(_)))
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
            Expr::MemberAccess(..) | Expr::Arrow(..) =>
                Err(self.pendiente("asignar a un miembro", 2)),
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
                    e = Expr::Call(n, args);
                }
                Token::Dot => {
                    self.avanzar();
                    let _ = self.avanzar();
                    return Err(self.pendiente("el acceso a miembro con `.`", 2));
                }
                Token::Arrow => {
                    self.avanzar();
                    let _ = self.avanzar();
                    return Err(self.pendiente("el acceso a miembro con `->`", 2));
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
            Token::This => Err(CppError::new(l,
                "`this`: llega en el PASO 2. El orden completo esta en \
                 toolchain/lang/cpp/BRECHA.md")),
            Token::Ident(n) => Ok(Expr::Var(n)),
            Token::OpenParen => {
                let e = self.expresion()?;
                self.exige(&Token::CloseParen)?;
                Ok(e)
            }
            otro => Err(culpa(format!("se esperaba una expresion y vino {otro:?}"))),
        }
    }
}
