//! **Parser de BMO C++** -- tokens a AST, por descenso recursivo.
//!
//! === La decision mas cara de deshacer, tomada aqui ===
//!
//! > **El parser y la tabla de simbolos se hablan.**
//!
//! No es una preferencia de estilo: **C++ no se puede parsear sin resolver
//! nombres a la vez**, y conviene tenerlo escrito antes de la primera linea en
//! vez de descubrirlo en el paso 6. Cuatro sitios donde la gramatica se muerde
//! la cola, con lo que hace este fichero en cada uno:
//!
//! 1. **`a<b>(c)`** -- instanciar la plantilla `a` con `b` y llamarla, o
//!    `(a<b)>(c)`, dos comparaciones? **Depende de si `a` es un nombre de
//!    plantilla**, que solo lo sabe la tabla. Hoy `plantillas` esta vacio y
//!    todo `<` es comparacion; el paso 6 puebla el conjunto y esta rama pasa a
//!    decidir. El punto de decision ya existe: [`Parser::es_plantilla`].
//! 2. **El *most vexing parse*** -- `T x(y);` declara `x` o declara una
//!    funcion? El estandar dice **si puede ser declaracion, es declaracion**, y
//!    aqui se implementa a proposito en [`Parser::parece_declaracion`].
//! 3. **Sentencia-declaracion vs sentencia-expresion** -- `T(x);` otra vez las
//!    dos cosas. Mismo desempate.
//! 4. **`>>` en plantillas** -- `Vector<Vector<int>>` es dos cierres, no un
//!    desplazamiento. Se arregla **en el parser** partiendo el token, nunca en
//!    el lexer: en el lexer no hay contexto para saber cual de los dos es.
//!    Llega con el paso 6; el sitio esta marcado.
//!
//! === La regla ===
//!
//! > Lo que no se sabe leer se **RECHAZA diciendo que se esperaba**. Nunca en
//! > silencio.
//!
//! El parser anterior hacia `pos += 1` con lo que no reconocia, asi que un
//! cuerpo entero podia desaparecer y el programa "compilaba". Aqui no hay
//! ninguna rama que descarte tokens.

use crate::ast::*;
use crate::lexer::{tokenizar, Token};
use crate::CppError;
use std::collections::{HashMap, HashSet};

/// El nombre del puntero a la vtabla dentro del objeto.
///
/// Lleva un punto --ilegal en C++-- para que no pueda chocar con un campo que
/// alguien escriba. Mismo truco que el mangling y que el `funcion.variable` de
/// BMO C.
pub const VPTR: &str = "vptr.";

pub fn parse(fuente: &str) -> Result<Program, CppError> {
    let lex = tokenizar(fuente);
    if let Some(e) = lex.errores.into_iter().next() {
        return Err(e);
    }
    Parser::nuevo(lex.toks, lex.lineas).programa()
}

// -- La tabla de simbolos --------------------------------------------

/// Ambitos anidados: el de dentro tapa al de fuera, y al salir se descarta.
///
/// Guarda **el tipo** de cada nombre, no solo que exista, porque el parser lo
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

// -- El parser -------------------------------------------------------

/// Lo que el parser sabe de una clase: es lo que hace falta para resolver
/// `p.x` sin volver a mirar la declaracion.
#[derive(Clone)]
struct Clase {
    /// *(nombre, offset, tipo)* en orden de declaracion.
    campos: Vec<(String, u32, TypeSpec)>,
    /// Los metodos por nombre simple; varias firmas es una sobrecarga.
    metodos: HashMap<String, Vec<Firma>>,
    /// Los constructores, que son una sobrecarga mas -- solo que el nombre lo
    /// pone el lenguaje y la llamada es implicita.
    constructores: Vec<Firma>,
    /// La clase base, si la hay. **Simple**: la multiple y la virtual estan
    /// descartadas con motivo en `BRECHA.md`.
    base: Option<String>,
    /// * **La vtabla: una ranura por metodo virtual, y el ORDEN es la tabla.**
    ///
    /// Un derivado empieza copiando la del padre; un `override` **sustituye**
    /// su ranura y un virtual nuevo se **anade** al final. Por eso un puntero
    /// a la base sirve tal cual sobre un derivado: las primeras ranuras
    /// significan lo mismo en los dos.
    vtabla: Vec<String>,
    /// Nombre de metodo -> ranura. Es lo que convierte una llamada en un
    /// despacho: si el nombre esta aqui, la llamada es virtual.
    ranura_de: HashMap<String, usize>,
    /// Tamano total, que el derivado necesita para colocar sus campos detras.
    tam: u32,
    // El TAMANO no esta aqui a proposito: el parser no lo necesita para nada
    // --resolver `p.x` solo pide offset y tipo-- y viaja en `Class::size`, que
    // es donde lo leera `new P()` en el paso 3. Guardar una copia que nadie
    // lee es exactamente la clase de dato que se queda obsoleto en silencio.
}

impl Clase {
    fn campo(&self, n: &str) -> Option<&(String, u32, TypeSpec)> {
        self.campos.iter().find(|(c, _, _)| c == n)
    }
}

/// Una declaracion de funcion o metodo, con su simbolo ya manglado.
///
/// Es lo que la resolucion de sobrecarga compara. El **retorno** viaja aqui y
/// no en el simbolo (C++ no sobrecarga por retorno) porque hace falta para
/// saber de que tipo es una llamada cuando es argumento de otra.
#[derive(Clone)]
struct Firma {
    params: Vec<TypeSpec>,
    ret: TypeSpec,
    simbolo: String,
}

/// Lo bien que encaja un argumento en un parametro. **Menos es mejor.**
///
/// === Como lo hace GCC, y que se le quita ===
///
/// `gcc/cp/call.cc` --uno de los ficheros mas grandes del frontend de C++, y
/// sorprende que lo sea-- construye para cada argumento una *secuencia de
/// conversion implicita* con hasta tres eslabones (lvalue, promocion,
/// cualificacion) y luego ordena secuencias parcialmente. Eso es lo que hace
/// falta para resolver contra plantillas, conversiones definidas por el
/// usuario y ADL.
///
/// BMO no tiene ninguna de las tres, asi que el orden colapsa a **tres
/// escalones** que se comparan sumando. Es lo que `MAESTROS.md` fijo como
/// alcance: *ranking minimo -- exacto > promocion > conversion*.
#[derive(PartialEq, PartialOrd, Clone, Copy)]
enum Encaje {
    Exacto,
    /// `char`/`short` -> `int`, `float` -> `double`. No pierde informacion.
    Promocion,
    /// Cualquier aritmetico a cualquier aritmetico. **Puede perder**, y por eso
    /// es el ultimo escalon: si hay una alternativa mejor, gana la otra.
    Conversion,
}

struct Parser {
    toks: Vec<Token>,
    lineas: Vec<usize>,
    pos: usize,
    ambitos: Ambitos,
    /// Las funciones declaradas, por nombre **simple**. Un nombre con varias
    /// firmas es una sobrecarga, y ahi entra [`Parser::resolver`].
    ///
    /// Se consulta con lo declarado **hasta ese punto**, que es la regla de C:
    /// para llamar a algo definido mas abajo hace falta un prototipo. Es lo
    /// que ya desbloqueaba la recursion mutua en el paso 1.
    funciones: HashMap<String, Vec<Firma>>,
    /// El simbolo de cada funcion, para saber su retorno al tipar una llamada.
    retornos: HashMap<String, TypeSpec>,
    /// Los espacios de nombres abiertos ahora mismo.
    espacios: Vec<String>,
    /// * Nombres de plantilla. **Vacio hasta el paso 6** -- y mientras este
    /// vacio, todo `<` es una comparacion. Ver [`Parser::es_plantilla`].
    plantillas: HashSet<String>,
    /// Las clases vistas. Es lo que hace que `P` sea un tipo y no un nombre.
    clases: HashMap<String, Clase>,
    /// La clase cuyo metodo se esta parseando, si alguno. Es lo que le da
    /// sentido a `this` y a un campo nombrado a secas dentro de un metodo.
    clase_actual: Option<String>,
}

impl Parser {
    fn nuevo(toks: Vec<Token>, lineas: Vec<usize>) -> Self {
        let mut p = Self {
            toks, lineas, pos: 0,
            ambitos: Ambitos::default(),
            funciones: HashMap::new(),
            retornos: HashMap::new(),
            espacios: Vec::new(),
            plantillas: HashSet::new(),
            clases: HashMap::new(),
            clase_actual: None,
        };
        p.ambitos.entrar(); // ambito de fichero
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

    /// * El punto de decision de `a<b>(c)`, aislado a proposito.
    ///
    /// Hoy siempre devuelve `false` --no hay plantillas-- y por tanto todo `<`
    /// es una comparacion, que es lo correcto para el lenguaje que hay. El
    /// paso 6 puebla `plantillas` y esta funcion pasa a partir la gramatica en
    /// dos sin tocar nada mas.
    #[allow(dead_code)]
    fn es_plantilla(&self, name: &str) -> bool {
        self.plantillas.contains(name)
    }

    // -- Resolucion de sobrecarga ------------------------------------

    /// Es un tipo con el que se puede hacer aritmetica?
    fn es_numero(t: &TypeSpec) -> bool {
        use TypeSpec as T;
        matches!(t, T::Bool | T::Char | T::UnsignedChar | T::Short | T::UnsignedShort
            | T::Int | T::UnsignedInt | T::Long | T::UnsignedLong
            | T::LongLong | T::UnsignedLongLong | T::Float | T::Double)
    }

    /// Lo bien que un argumento de tipo `dado` encaja en un parametro `quiere`.
    fn encaje(dado: &TypeSpec, quiere: &TypeSpec) -> Option<Encaje> {
        use TypeSpec as T;
        if dado == quiere { return Some(Encaje::Exacto); }
        // Una referencia se ata al valor: `f(int&)` acepta un `int`. Encaja
        // exacto porque no hay conversion ninguna -- solo se pasa la direccion.
        if let T::Ref(d) = quiere {
            if &**d == dado { return Some(Encaje::Exacto); }
        }
        // Un array decae a puntero a su elemento, que es lo que C hace en toda
        // llamada. Sin esto, `f(char*)` no aceptaria un `char[8]`.
        if let (T::Array(e, _), T::Ptr(p)) = (dado, quiere) {
            if e == p { return Some(Encaje::Exacto); }
        }
        if !Self::es_numero(dado) || !Self::es_numero(quiere) { return None; }
        // La promocion entera y la de coma flotante: NO pierden informacion.
        let promociona = matches!(
            (dado, quiere),
            (T::Char | T::UnsignedChar | T::Short | T::UnsignedShort | T::Bool, T::Int)
            | (T::Float, T::Double)
        );
        Some(if promociona { Encaje::Promocion } else { Encaje::Conversion })
    }

    /// Elige la firma que mejor encaja, o dice por que no puede.
    ///
    /// El criterio es la **suma** de los escalones de cada argumento, y el
    /// empate es un error con los dos candidatos escritos. Una ambiguedad que
    /// se resolviera sola --eligiendo "el primero", por ejemplo-- haria que
    /// anadir una sobrecarga cambiara a que funcion va una llamada existente,
    /// en silencio.
    fn resolver<'f>(&self, que: &str, firmas: &'f [Firma], args: &[TypeSpec])
        -> Result<&'f Firma, CppError>
    {
        let mut mejor: Option<(u32, &Firma)> = None;
        let mut empate = false;
        let mut hubo_aridad = false;

        for f in firmas {
            if f.params.len() != args.len() { continue; }
            hubo_aridad = true;
            let mut coste = 0u32;
            let mut vale = true;
            for (a, p) in args.iter().zip(f.params.iter()) {
                match Self::encaje(a, p) {
                    Some(e) => coste += e as u32,
                    None => { vale = false; break; }
                }
            }
            if !vale { continue; }
            match mejor {
                None => mejor = Some((coste, f)),
                Some((c, _)) if coste < c => { mejor = Some((coste, f)); empate = false; }
                Some((c, _)) if coste == c => empate = true,
                _ => {}
            }
        }

        if empate {
            let opciones: Vec<String> = firmas.iter()
                .filter(|f| f.params.len() == args.len())
                .map(|f| f.simbolo.clone()).collect();
            return Err(self.err(format!(
                "la llamada a `{que}` es ambigua entre {}: ninguna encaja mejor que la otra",
                opciones.join(" y "))));
        }
        match mejor {
            Some((_, f)) => Ok(f),
            None if hubo_aridad => Err(self.err(format!(
                "ninguna version de `{que}` acepta esos tipos de argumento"))),
            None => Err(self.err(format!(
                "`{que}` no tiene ninguna version con {} argumento(s)", args.len()))),
        }
    }

    /// El tipo de una expresion. Se usa para resolver `.` y para tipar los
    /// argumentos de una llamada.
    ///
    /// Cubre poco a proposito: en cuanto cubriera de mas seria un comprobador
    /// de tipos, y eso no es lo que este paso promete. Lo que no sabe tipar
    /// lo dice, en vez de suponer `int`.
    fn tipo_de(&self, e: &Expr) -> Option<TypeSpec> {
        use TypeSpec as T;
        Some(match e {
            Expr::Int(_) => T::Int,
            Expr::FloatLit(_) => T::Double,
            Expr::CharLit(_) => T::Char,
            Expr::BoolLit(_) => T::Bool,
            Expr::StringLit(_) => T::Ptr(Box::new(T::Char)),
            Expr::NullPtr => T::Ptr(Box::new(T::Void)),
            Expr::Var(n) => self.ambitos.tipo(n)?.clone(),
            Expr::This => T::Ptr(Box::new(T::ClassRef(self.clase_actual.clone()?))),
            Expr::MemberAccess(_, _, _, t) | Expr::Arrow(_, _, _, t) => t.clone(),
            Expr::AssignMember(_, _, _, t, _) | Expr::AssignArrow(_, _, _, t, _) => t.clone(),
            Expr::Cast(t, _) => t.clone(),
            Expr::Call(simbolo, _) => self.retornos.get(simbolo)?.clone(),
            Expr::MethodCall(_, _, simbolo, _) => self.retornos.get(simbolo)?.clone(),
            Expr::Deref(b) => match self.tipo_de(b)? {
                T::Ptr(t) | T::Array(t, _) => *t,
                _ => return None,
            },
            Expr::AddrOf(b) => T::Ptr(Box::new(self.tipo_de(b)?)),
            Expr::Subscript(n, _, _) => match self.ambitos.tipo(n)? {
                T::Ptr(t) | T::Array(t, _) => (**t).clone(),
                _ => return None,
            },
            Expr::Assign(n, _) => self.ambitos.tipo(n)?.clone(),
            Expr::Neg(b) | Expr::BitNot(b) => self.tipo_de(b)?,
            Expr::PreInc(n) | Expr::PreDec(n) | Expr::PostInc(n) | Expr::PostDec(n) =>
                self.ambitos.tipo(n)?.clone(),
            // Comparaciones y logicos dan un entero, como en C.
            Expr::Eq(..) | Expr::Neq(..) | Expr::Lt(..) | Expr::Gt(..) | Expr::Le(..)
            | Expr::Ge(..) | Expr::And(..) | Expr::Or(..) | Expr::Not(_) => T::Int,
            // Las conversiones aritmeticas al uso, recortadas: si alguno es
            // `double`, el resultado es `double`; si no, `int`. Sin esto, un
            // `f(1 + 2.0)` elegiria la sobrecarga entera.
            Expr::Add(a, b) | Expr::Sub(a, b) | Expr::Mul(a, b) | Expr::Div(a, b) => {
                let (ta, tb) = (self.tipo_de(a)?, self.tipo_de(b)?);
                // La aritmetica de punteros conserva el puntero.
                if matches!(ta, T::Ptr(_) | T::Array(..)) { ta }
                else if matches!(tb, T::Ptr(_) | T::Array(..)) { tb }
                else if ta == T::Double || tb == T::Double { T::Double }
                else { T::Int }
            }
            Expr::Mod(..) | Expr::BitAnd(..) | Expr::BitOr(..) | Expr::BitXor(..)
            | Expr::Shl(..) | Expr::Shr(..) => T::Int,
            Expr::Conditional(_, a, _) => self.tipo_de(a)?,
            _ => return None,
        })
    }

    /// Los tipos de una lista de argumentos, o un error que dice cual no se
    /// supo tipar.
    fn tipos_de(&self, args: &[Expr], que: &str) -> Result<Vec<TypeSpec>, CppError> {
        let mut out = Vec::new();
        for (i, a) in args.iter().enumerate() {
            out.push(self.tipo_de(a).ok_or_else(|| self.err(format!(
                "no se sabe de que tipo es el argumento {} de `{que}`", i + 1)))?);
        }
        Ok(out)
    }

    // -- Nivel de fichero --------------------------------------------

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

    /// Una funcion o una variable global. Se distinguen por lo que hay
    /// **detras del nombre**: un parentesis es una funcion.
    fn declaracion_de_fichero(&mut self, p: &mut Program) -> Result<(), CppError> {
        self.come(&Token::Static);
        self.come(&Token::Const);
        let base = self.tipo_base()?;
        let (tipo, name) = self.declarador(base)?;

        if *self.peek() == Token::OpenParen {
            self.avanzar();
            let params = self.parametros()?;
            self.exige(&Token::CloseParen)?;
            self.come(&Token::Const);

            let simbolo = self.declarar_funcion(&name, &params, &tipo)?;

            if self.come(&Token::Semicolon) {
                // Prototipo. Se registra la firma y no se emite nada: sirve
                // para que una llamada anterior a la definicion se pueda
                // resolver, que es lo que desbloquea la recursion mutua.
                return Ok(());
            }

            self.ambitos.entrar();
            for pa in &params { self.ambitos.declarar(&pa.name, pa.typ.clone()); }
            let cuerpo = self.bloque()?;
            self.ambitos.salir();
            // * `main` NO se mangla. Es el punto de entrada que el codegen de C
            // busca por nombre, y manglarlo dejaria un `.bef` sin `main`.
            let emitido = if name == "main" && self.espacios.is_empty() {
                "main".to_string()
            } else { simbolo };
            p.functions.push(Function { ret_type: tipo, name: emitido, params, body: cuerpo });
            return Ok(());
        }

        // Variable global.
        let init = if self.come(&Token::Assign) { Some(self.asignacion()?) } else { None };
        self.exige(&Token::Semicolon)?;
        self.ambitos.declarar(&name, tipo.clone());
        p.globals.push(GlobalDecl::Var(tipo, name, init));
        Ok(())
    }

    // -- Clases ------------------------------------------------------

    /// `class P { public: int x; int doble() { ... } };`
    ///
    /// * Se parsea en **dos vueltas**, y no por gusto: un metodo puede usar un
    /// campo declarado **mas abajo** en la clase --eso es legal en C++ y no lo
    /// es en C-- asi que la disposicion tiene que estar completa antes de mirar
    /// un solo cuerpo. Primero se recogen las firmas y los campos; despues se
    /// parsean los cuerpos con la clase ya registrada.
    fn clase(&mut self) -> Result<Class, CppError> {
        let es_struct = *self.peek() == Token::Struct;
        self.avanzar();
        let name = match self.avanzar() {
            Token::Ident(n) => n,
            otro => return Err(self.err(format!("se esperaba el nombre de la clase y vino {otro:?}"))),
        };
        // -- La herencia, simple --
        //
        // `class B : public A` -- el derivado **empieza por** la base entera.
        // Eso es todo el mecanismo: un `B*` vale como `A*` sin ajustar nada,
        // porque los campos de `A` estan en los mismos offsets. La herencia
        // MULTIPLE necesitaria ajustar el `this` al llamar (thunks) y la
        // VIRTUAL localizar la base compartida en ejecucion; las dos estan
        // descartadas con motivo en `BRECHA.md`.
        let mut base = None;
        if self.come(&Token::Colon) {
            self.come(&Token::Public);
            if self.come(&Token::Private) || self.come(&Token::Protected) {
                return Err(self.pendiente("la herencia privada o protegida", 6));
            }
            self.come(&Token::Virtual);
            let n = match self.avanzar() {
                Token::Ident(n) => n,
                otro => return Err(self.err(format!(
                    "se esperaba el nombre de la clase base y vino {otro:?}"))),
            };
            if !self.clases.contains_key(&n) {
                return Err(self.err(format!("la clase base `{n}` no esta definida")));
            }
            if *self.peek() == Token::Comma {
                return Err(self.pendiente("la herencia multiple", 6));
            }
            base = Some(n);
        }
        self.exige(&Token::OpenBrace)?;

        // -- Vuelta 1: campos y firmas --
        let mut campos: Vec<MemberVar> = Vec::new();
        let mut cuerpos: Vec<(usize, Method)> = Vec::new(); // (posicion del `{`, firma)
        let mut ctores: Vec<(usize, Method)> = Vec::new();
        let mut dtor: Option<(usize, Method)> = None;
        let mut acceso = if es_struct { Access::Public } else { Access::Private };
        let mut virtual_ahora = false;

        while *self.peek() != Token::CloseBrace {
            match self.peek().clone() {
                Token::Eof => return Err(self.err("se acabo el fichero dentro de una clase")),
                Token::Public => { self.avanzar(); self.exige(&Token::Colon)?; acceso = Access::Public; continue; }
                Token::Private => { self.avanzar(); self.exige(&Token::Colon)?; acceso = Access::Private; continue; }
                Token::Protected => { self.avanzar(); self.exige(&Token::Colon)?; acceso = Access::Protected; continue; }
                Token::Semicolon => { self.avanzar(); continue; }
                Token::Virtual => { self.avanzar(); virtual_ahora = true; continue; }
                Token::Friend => return Err(self.pendiente("`friend`", 4)),
                Token::Static => return Err(self.pendiente("los miembros `static`", 4)),
                Token::Operator => return Err(self.pendiente("la sobrecarga de operadores", 4)),

                // -- Destructor: `~P() { ... }` --
                Token::Tilde => {
                    self.avanzar();
                    match self.avanzar() {
                        Token::Ident(n) if n == name => {}
                        otro => return Err(self.err(format!(
                            "el destructor de `{name}` se llama `~{name}`, no {otro:?}"))),
                    }
                    self.exige(&Token::OpenParen)?;
                    if *self.peek() != Token::CloseParen {
                        return Err(self.err("un destructor no lleva parametros"));
                    }
                    self.avanzar();
                    if dtor.is_some() {
                        return Err(self.err(format!("`{name}` ya tiene destructor")));
                    }
                    let inicio = self.pos;
                    self.saltar_bloque()?;
                    dtor = Some((inicio, Method {
                        name: format!("~{name}"), ret_type: TypeSpec::Void, params: vec![],
                        body: Vec::new(), is_virtual: false, is_override: false,
                        is_const: false, access: Access::Public, class_name: name.clone(),
                    }));
                    continue;
                }

                // -- Constructor: `P(...) { ... }` -- el nombre de la clase
                //    seguido de parentesis, y SIN tipo de retorno delante.
                Token::Ident(n) if n == name && *self.peek_en(1) == Token::OpenParen => {
                    self.avanzar();
                    self.avanzar();
                    let params = self.parametros()?;
                    self.exige(&Token::CloseParen)?;
                    // `P() : x(0) {}` -- la lista de inicializacion de miembros.
                    // No entra todavia: pide resolver un inicializador POR
                    // MIEMBRO en el orden de declaracion (que no es el orden en
                    // que se escriben), y ese es trabajo del paso 4. Mientras
                    // tanto el cuerpo `{ x = 0; }` hace lo mismo.
                    if *self.peek() == Token::Colon {
                        return Err(self.pendiente(
                            "la lista de inicializacion de miembros (`P() : x(0)`)", 4));
                    }
                    let inicio = self.pos;
                    self.saltar_bloque()?;
                    ctores.push((inicio, Method {
                        name: name.clone(), ret_type: TypeSpec::Void, params,
                        body: Vec::new(), is_virtual: false, is_override: false,
                        is_const: false, access: acceso, class_name: name.clone(),
                    }));
                    continue;
                }
                _ => {}
            }

            let base = self.tipo_base()?;
            // `int operator+(int)` -- el `operator` viene DETRAS del tipo, asi
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
                let es_override = self.come(&Token::Override);
                // `virtual int f() = 0;` -- una pura. Pide una clase abstracta
                // y una ranura que nadie rellena; llega despues.
                if *self.peek() == Token::Assign {
                    return Err(self.pendiente("las funciones virtuales PURAS (`= 0`)", 6));
                }
                if self.come(&Token::Semicolon) {
                    return Err(self.pendiente("declarar un metodo y definirlo fuera de la clase", 4));
                }
                // Se anota donde empieza el cuerpo y se salta: se parseara en
                // la vuelta 2, cuando la disposicion este completa.
                let inicio = self.pos;
                self.saltar_bloque()?;
                cuerpos.push((inicio, Method {
                    name: miembro, ret_type: tipo, params, body: Vec::new(),
                    is_virtual: virtual_ahora, is_override: es_override, is_const: es_const,
                    access: acceso, class_name: name.clone(),
                }));
                virtual_ahora = false;
            } else {
                self.exige(&Token::Semicolon)?;
                if virtual_ahora {
                    return Err(self.err("`virtual` sobre un campo no significa nada"));
                }
                campos.push(MemberVar { typ: tipo, name: miembro, offset: 0, access: acceso });
            }
        }
        self.avanzar(); // `}`
        self.exige(&Token::Semicolon)?;

        // -- La disposicion --
        //
        // La regla NO esta escrita aqui: vive una sola vez en
        // `bmo_abi::types::disposicion`, con sus tests. El parser de C++ tiene
        // que calcularla igualmente --los nodos `Field` de C llevan el offset
        // dentro-- pero calcula con la MISMA regla que el codegen de C, que es
        // lo que importa.
        //
        // Y el descenso emite el `struct` con miembros y no con offsets, asi
        // que el codegen recalcula por su cuenta: si algun dia las dos
        // llamadas dieran cosas distintas, la fila `clase-disposicion` de la
        // matriz se pone roja en vez de pasar muda.
        let padre = base.as_ref().and_then(|b| self.clases.get(b)).cloned();
        let hay_virtuales = cuerpos.iter().any(|(_, m)| m.is_virtual)
            || padre.as_ref().map_or(false, |p| !p.vtabla.is_empty());

        let mut layout = Vec::new();
        let mut d = bmo_abi::types::Disposicion::nueva();

        if let Some(p) = &padre {
            // * El derivado **empieza por la base entera**, campos incluidos y
            // en los MISMOS offsets. Ese es todo el mecanismo de la herencia
            // simple: un `B*` vale como `A*` sin ajustar nada.
            for (n, off, t) in &p.campos {
                layout.push((n.clone(), *off, t.clone()));
            }
            for _ in 0..p.tam { d.coloca(1); }
        } else if hay_virtuales {
            // * **El `vptr` va en el offset 0**, no en medio de la tabla como
            // en Itanium: el *offset-to-top* y la ranura de RTTI solo hacen
            // falta con herencia multiple y RTTI, y las dos estan descartadas.
            // Al principio es lo que se escribiria a mano en C -- y es lo que
            // hace que el despacho sea una indireccion y no una resta.
            let vptr = TypeSpec::Ptr(Box::new(TypeSpec::Void));
            layout.push((VPTR.to_string(), d.coloca(vptr.size()), vptr));
        }

        for m in &campos {
            layout.push((m.name.clone(), d.coloca(m.typ.size()), m.typ.clone()));
        }
        let tam = d.total();

        // Las firmas de los metodos, con su simbolo. Se registran ANTES de
        // bajar ningun cuerpo, que es lo que permite que un metodo llame a otro
        // declarado mas abajo -- y a si mismo.
        let mut metodos: HashMap<String, Vec<Firma>> = HashMap::new();
        for (_, m) in &cuerpos {
            let tipos: Vec<TypeSpec> = m.params.iter().map(|p| p.typ.clone()).collect();
            let simbolo = crate::mangling::metodo(&self.espacios, &name, &m.name, &tipos);
            let lista = metodos.entry(m.name.clone()).or_default();
            if lista.iter().any(|f| f.simbolo == simbolo) {
                return Err(self.err(format!(
                    "`{name}::{}` esta declarado dos veces con los mismos parametros", m.name)));
            }
            lista.push(Firma { params: tipos, ret: m.ret_type.clone(), simbolo: simbolo.clone() });
            self.retornos.insert(simbolo, m.ret_type.clone());
        }
        let mut constructores: Vec<Firma> = Vec::new();
        for (_, m) in &ctores {
            let tipos: Vec<TypeSpec> = m.params.iter().map(|p| p.typ.clone()).collect();
            let simbolo = crate::mangling::constructor(&self.espacios, &name, &tipos);
            if constructores.iter().any(|f| f.simbolo == simbolo) {
                return Err(self.err(format!(
                    "`{name}` tiene dos constructores con los mismos parametros")));
            }
            constructores.push(Firma {
                params: tipos, ret: TypeSpec::Void, simbolo: simbolo.clone() });
            self.retornos.insert(simbolo, TypeSpec::Void);
        }
        // -- La vtabla --
        //
        // Se parte de la del padre. Un `override` **sustituye** su ranura; un
        // virtual nuevo se **anade** al final. Ese es el motivo por el que un
        // puntero a la base sirve sobre un derivado sin tocar nada: las
        // primeras ranuras significan lo mismo en los dos.
        let mut vtabla = padre.as_ref().map(|p| p.vtabla.clone()).unwrap_or_default();
        let mut ranura_de = padre.as_ref().map(|p| p.ranura_de.clone()).unwrap_or_default();
        for (_, m) in &cuerpos {
            let heredada = ranura_de.get(&m.name).copied();
            if !m.is_virtual && heredada.is_none() { continue; }
            if m.is_override && heredada.is_none() {
                return Err(self.err(format!(
                    "`{}::{}` dice `override` pero no hay ningun metodo virtual con ese \
                     nombre en la base", name, m.name)));
            }
            let tipos: Vec<TypeSpec> = m.params.iter().map(|p| p.typ.clone()).collect();
            let simbolo = crate::mangling::metodo(&self.espacios, &name, &m.name, &tipos);
            match heredada {
                Some(r) => vtabla[r] = simbolo,
                None => { ranura_de.insert(m.name.clone(), vtabla.len()); vtabla.push(simbolo); }
            }
        }
        // Los metodos del padre que el hijo NO redefine se heredan tal cual:
        // sus firmas siguen valiendo, y su ranura ya esta puesta arriba.
        if let Some(p) = &padre {
            for (n, fs) in &p.metodos {
                metodos.entry(n.clone()).or_insert_with(|| fs.clone());
            }
        }

        let info = Clase {
            campos: layout.clone(), metodos, constructores,
            base: base.clone(), vtabla: vtabla.clone(), ranura_de, tam,
        };
        self.clases.insert(name.clone(), info);

        // -- Vuelta 2: los cuerpos, con la clase ya registrada --
        let vuelta = self.pos;
        let mut cuerpo_de = |p: &mut Self, inicio: usize, m: &mut Method| -> Result<(), CppError> {
            p.pos = inicio;
            p.clase_actual = Some(name.clone());
            p.ambitos.entrar();
            p.ambitos.declarar("this", TypeSpec::Ptr(Box::new(TypeSpec::ClassRef(name.clone()))));
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
        let mut constructors = Vec::new();
        for (inicio, mut m) in ctores {
            cuerpo_de(self, inicio, &mut m)?;
            constructors.push(m);
        }
        let destructor = match dtor {
            Some((inicio, mut m)) => { cuerpo_de(self, inicio, &mut m)?; Some(m) }
            None => None,
        };
        self.pos = vuelta;

        // * Los miembros del AST salen de `layout`, **no de `campos`**, porque
        // un derivado tiene que llevar tambien los campos de la base: el
        // `struct` que vera el codegen de C es el objeto ENTERO. Antes se
        // emparejaban `campos` (solo los propios) con `layout` (base incluida)
        // y el resultado era un struct al que le faltaban los heredados -- y
        // ademas con los offsets corridos, porque el emparejado se desalineaba.
        //
        // El `vptr` se salta: lo anade el descenso, que es quien decide como
        // se llama el campo del lado de C.
        let acceso_de: HashMap<&str, Access> =
            campos.iter().map(|m| (m.name.as_str(), m.access)).collect();
        let mut miembros = Vec::new();
        for (n, off, t) in &layout {
            if n == VPTR { continue; }
            miembros.push(MemberVar {
                typ: t.clone(),
                name: n.clone(),
                offset: *off,
                access: acceso_de.get(n.as_str()).copied().unwrap_or(Access::Public),
            });
        }

        Ok(Class {
            name: name,
            bases: base.into_iter().collect(),
            members: miembros,
            methods: metodos,
            constructors, destructor,
            vtable: !vtabla.is_empty(),
            vtabla,
            size: tam,
        })
    }

    /// Salta un bloque `{ ... }` contando llaves, sin interpretarlo.
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

    /// Elige el constructor de `cls` para estos argumentos.
    ///
    /// `None` significa *"esta clase no tiene constructor"*, que es legal y
    /// deja el objeto sin inicializar -- igual que un `struct` de C. Pedir
    /// argumentos a una clase sin constructor si es error.
    fn resolver_ctor(&self, cls: &str, args: &[Expr]) -> Result<Option<String>, CppError> {
        let Some(info) = self.clases.get(cls) else {
            return Err(self.err(format!("la clase `{cls}` no esta definida")));
        };
        if info.constructores.is_empty() {
            if !args.is_empty() {
                return Err(self.err(format!(
                    "`{cls}` no tiene constructor, asi que no acepta argumentos")));
            }
            return Ok(None);
        }
        let firmas = info.constructores.clone();
        let tipos = self.tipos_de(args, cls)?;
        Ok(Some(self.resolver(cls, &firmas, &tipos)?.simbolo.clone()))
    }

    /// Registra una funcion y devuelve su simbolo.
    ///
    /// Rechaza redeclarar la MISMA firma con otro retorno, que es lo que C++
    /// prohibe: no se puede sobrecargar por retorno, asi que dos `f(int)` con
    /// retornos distintos son la misma funcion declarada dos veces mal.
    fn declarar_funcion(&mut self, name: &str, params: &[Param], ret: &TypeSpec)
        -> Result<String, CppError>
    {
        let tipos: Vec<TypeSpec> = params.iter().map(|p| p.typ.clone()).collect();
        let simbolo = if name == "main" && self.espacios.is_empty() {
            "main".to_string()
        } else {
            crate::mangling::funcion(&self.espacios, name, &tipos)
        };
        let lista = self.funciones.entry(name.to_string()).or_default();
        if let Some(ya) = lista.iter().find(|f| f.simbolo == simbolo) {
            if &ya.ret != ret {
                return Err(self.err(format!(
                    "`{name}` ya esta declarada con los mismos parametros y otro retorno: \
                     C++ no permite sobrecargar por el tipo de retorno")));
            }
        } else {
            lista.push(Firma { params: tipos, ret: ret.clone(), simbolo: simbolo.clone() });
        }
        self.retornos.insert(simbolo.clone(), ret.clone());
        Ok(simbolo)
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
            // El nombre del parametro es opcional: `int f(int);` es legal.
            let (tipo, name) = if matches!(self.peek(), Token::Ident(_))
                || matches!(self.peek(), Token::Star | Token::And)
            {
                self.declarador(base)?
            } else {
                (base, String::new())
            };
            let defecto = if self.come(&Token::Assign) {
                return Err(self.pendiente("los argumentos por defecto", 4));
            } else { None };
            // [!] Un parametro de coma flotante NO se puede pasar todavia: BMO C
            // evalua floats por la ruta SSE pero **no los pasa como
            // argumento** (falta la ABI de xmm), y lo peor es que los acepta
            // en silencio -- `int g(double a)` compila y no hace lo que dice.
            // Es deuda de C; mientras exista, C++ no la emite.
            if matches!(tipo, TypeSpec::Float | TypeSpec::Double) {
                return Err(self.err(format!(
                    "`{}` es un parametro de coma flotante, y BMO C todavia no los PASA \
                     (evalua floats en xmm, pero la ABI de argumentos xmm esta pendiente). \
                     Se rechaza aqui a proposito: C lo acepta en silencio y no funciona",
                    if name.is_empty() { "<sin nombre>" } else { &name })));
            }
            out.push(Param { typ: tipo, name: name, default: defecto });
            if !self.come(&Token::Comma) { break; }
        }
        Ok(out)
    }

    // -- Tipos -------------------------------------------------------

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

    /// * **El asterisco es del DECLARADOR, no del tipo base.**
    ///
    /// En `int *a, b;` la `b` es un `int`. Es un bug que BMO C ya pago una vez
    /// --guardaba como base el tipo *ya con punteros*-- y por eso aqui el tipo
    /// base se pasa **por valor** a cada declarador: cada uno se lleva su
    /// copia y le anade lo suyo.
    fn declarador(&mut self, base: TypeSpec) -> Result<(TypeSpec, String), CppError> {
        let mut t = base;
        loop {
            if self.come(&Token::Star) { t = TypeSpec::Ptr(Box::new(t)); self.come(&Token::Const); }
            else if self.come(&Token::And) { t = TypeSpec::Ref(Box::new(t)); }
            else { break; }
        }
        let name = match self.avanzar() {
            Token::Ident(n) => n,
            otro => return Err(self.err(format!("se esperaba un nombre y vino {otro:?}"))),
        };
        // `v[n]` -- el corchete es del declarador, igual que el asterisco.
        while self.come(&Token::OpenBracket) {
            let n = match self.avanzar() {
                Token::IntLit(v) if v > 0 => v as u32,
                otro => return Err(self.err(format!(
                    "el tamano de un array tiene que ser un entero positivo, vino {otro:?}"))),
            };
            self.exige(&Token::CloseBracket)?;
            t = TypeSpec::Array(Box::new(t), n);
        }
        Ok((t, name))
    }

    /// Lo que viene es una declaracion?
    ///
    /// * Aqui vive el ***most vexing parse***: `T x(y);` puede leerse como una
    /// variable `x` inicializada con `y`, o como la declaracion de una funcion
    /// `x` que toma un `y`. El estandar zanja que **si puede ser declaracion,
    /// es declaracion** -- y como aqui lo unico que puede empezar una
    /// declaracion es una palabra clave de tipo, la regla sale sola: si el
    /// token es un tipo, es declaracion; si no, es expresion.
    ///
    /// El dia que un identificador pueda ser un tipo (paso 2, clases), esta
    /// funcion es el unico sitio que cambia -- y necesitara la tabla de
    /// simbolos, que ya esta aqui.
    fn parece_declaracion(&self) -> bool {
        if matches!(self.peek(),
            Token::Void | Token::Bool | Token::Char | Token::Short | Token::Int
            | Token::Long | Token::Float | Token::Double | Token::Unsigned
            | Token::Signed | Token::Const | Token::Static | Token::Auto)
        {
            return true;
        }
        // ** **`P *q` es una declaracion o una multiplicacion, y solo la
        // tabla de simbolos lo sabe.**
        //
        // Este es el caso que justifica, el solo, que el parser y la tabla se
        // hablen. `P * q` con `P` desconocida es el producto de dos variables;
        // con `P` declarada como clase es *"puntero a P llamado q"*. La misma
        // secuencia de tokens, dos arboles distintos, y **las dos compilan**:
        // si se elige mal, el programa hace otra cosa sin quejarse.
        //
        // Es el hermano pequeno de `a<b>(c)` (ver `MAESTROS.md`), y llego en
        // cuanto existieron las clases -- antes de lo previsto, porque no hace
        // falta una plantilla para que C++ muerda.
        let Token::Ident(n) = self.peek() else { return false };
        if !self.clases.contains_key(n) {
            // Dos identificadores seguidos solo pueden ser `Tipo name`. Se
            // reconoce aunque el tipo no exista, para que el error sea
            // *"`P` no es un tipo conocido"* y no *"se esperaba `;`"*, que
            // manda a mirar la puntuacion.
            return matches!(self.peek_en(1), Token::Ident(_));
        }
        matches!(self.peek_en(1), Token::Ident(_) | Token::Star | Token::And)
    }

    // -- Sentencias --------------------------------------------------

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

    /// `T a = 1, b;` -- el tipo base se comparte, cada declarador trae lo suyo.
    fn declaracion_local(&mut self) -> Result<Stmt, CppError> {
        self.come(&Token::Static);
        let base = self.tipo_base()?;
        let mut decls = Vec::new();
        loop {
            let (tipo, name) = self.declarador(base.clone())?;
            // -- Declarar un objeto de clase --
            if let TypeSpec::ClassRef(cls) = &tipo {
                let cls = cls.clone();
                let args = if *self.peek() == Token::OpenParen {
                    self.avanzar();
                    // ** **El *most vexing parse*, aqui mismo.**
                    //
                    // `P p();` NO declara un objeto: declara una FUNCION `p`
                    // que no toma nada y devuelve `P`. El estandar zanja que
                    // si algo puede leerse como declaracion, es declaracion --
                    // y esto puede. Es el error que todo el mundo comete una
                    // vez, y el compilador que lo acepta en silencio deja un
                    // objeto sin construir.
                    if *self.peek() == Token::CloseParen {
                        return Err(self.err(format!(
                            "`{cls} {name}();` declara una FUNCION que devuelve `{cls}`, \
                             no un objeto (el *most vexing parse*). Para construir con el \
                             constructor por defecto se escribe `{cls} {name};`")));
                    }
                    let mut a = Vec::new();
                    loop {
                        a.push(self.asignacion()?);
                        if !self.come(&Token::Comma) { break; }
                    }
                    self.exige(&Token::CloseParen)?;
                    a
                } else if *self.peek() == Token::Assign {
                    return Err(self.pendiente("el constructor de copia (`P b = a;`)", 5));
                } else {
                    Vec::new()
                };
                let ctor = self.resolver_ctor(&cls, &args)?;
                self.ambitos.declarar(&name, tipo.clone());
                decls.push(Stmt::DeclObj { clase: cls, name, ctor, args });
                if self.come(&Token::Comma) { continue; }
                break;
            }
            // * El inicializador es una `assignment-expression`, NO una
            // `expression`. Con la coma completa, `int a = 20, b = 22;` se
            // leeria `a = (20, b = 22)` usando el operador coma. El escalon de
            // la gramatica existe exactamente para esto -- es un bug que BMO C
            // ya pago.
            let init = if self.come(&Token::Assign) { Some(self.asignacion()?) } else { None };
            self.ambitos.declarar(&name, tipo.clone());
            decls.push(Stmt::DeclVar(tipo, name, init));
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

    /// `for(T i = 0; ...)` se desazucara a `{ T i = 0; for(; ...) cuerpo }`.
    ///
    /// Es lo mismo que hace el parser de C, y por el mismo motivo: el nodo
    /// `For` lleva una **expresion** en el init, y una declaracion no es una
    /// expresion. Envolver en un bloque ademas le da a `i` el ambito correcto.
    fn para(&mut self) -> Result<Stmt, CppError> {
        self.avanzar();
        self.exige(&Token::OpenParen)?;
        self.ambitos.entrar();

        let decl = if self.parece_declaracion() {
            let base = self.tipo_base()?;
            let (tipo, name) = self.declarador(base)?;
            let init = if self.come(&Token::Assign) { Some(self.asignacion()?) } else { None };
            self.exige(&Token::Semicolon)?;
            self.ambitos.declarar(&name, tipo.clone());
            Some(Stmt::DeclVar(tipo, name, init))
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

    // -- Expresiones, por precedencia --------------------------------
    //
    // De menor a mayor: coma -> asignacion -> ternario -> || -> && -> | -> ^ -> &
    // -> ==/!= -> </>/<=/>= -> <</>> -> +/- -> *///% -> unario -> sufijo -> primario.
    //
    // Cada nivel es una funcion y llama al de mas arriba. Es la escalera de la
    // gramatica de C tal cual: no esta aqui por copiarla, esta porque **el
    // escalon de la asignacion es lo unico que impide que `int a = 20, b = 22`
    // se lea con el operador coma**.

    fn expresion(&mut self) -> Result<Expr, CppError> {
        let e = self.asignacion()?;
        // El operador coma no esta en el AST de C++ todavia. En vez de
        // tragarselo (que daria el valor equivocado en silencio), se dice.
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
        // La asignacion asocia a la DERECHA: `a = b = c` es `a = (b = c)`.
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
    /// tabla, en vez de nueve funciones que solo se diferencian en la fila.
    /// Anadir un operador es anadir una fila de [`Self::precedencia`].
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
            // * Aqui es donde el paso 6 tendra que preguntar a la tabla de
            // simbolos: un `<` detras de un nombre de plantilla abre una lista
            // de argumentos, no una comparacion. Mientras `plantillas` este
            // vacio, todo `<` es comparacion -- que es la verdad de hoy.
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
            // `(T)e` -- una conversion. Se distingue de `(expr)` porque dentro
            // del parentesis hay una palabra clave de tipo.
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
                    // La escala sale de la tabla de simbolos: es el tamano del
                    // ELEMENTO, no el del array.
                    let escala = match self.ambitos.tipo(&n) {
                        Some(TypeSpec::Array(t, _)) => t.size() as u32,
                        Some(TypeSpec::Ptr(t)) => t.size() as u32,
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
                    // * Un metodo propio llamado a secas ES `this->metodo(...)`.
                    //
                    // El mismo caso que un campo a secas, y con el mismo
                    // desempate: una funcion libre del mismo nombre tapa al
                    // metodo solo si el metodo no existe. Al reves, una clase
                    // con un metodo `abs` haria que `abs(x)` llamara al metodo
                    // desde fuera de la clase.
                    let propio = self.clase_actual.clone().and_then(|c| {
                        self.clases.get(&c).and_then(|i| {
                            i.metodos.get(&n).map(|f| {
                                (c.clone(), f.clone(), i.ranura_de.get(&n).copied())
                            })
                        })
                    });
                    e = match propio {
                        Some((cls, firmas, slot)) => {
                            let tipos = self.tipos_de(&args, &n)?;
                            let s = self.resolver(&format!("{cls}::{n}"), &firmas, &tipos)?
                                .simbolo.clone();
                            // * Un metodo propio llamado a secas **tambien
                            // despacha virtualmente**. Es el caso que mas se
                            // olvida: `int doble() { return f() * 2; }` con `f`
                            // virtual tiene que llamar a la `f` del objeto
                            // REAL, no a la de la clase donde esta escrito
                            // `doble`. Un compilador que lo resuelve estatico
                            // devuelve el resultado de la base y nadie sabe por
                            // que.
                            match slot {
                                Some(r) => Expr::VirtualCall(
                                    Box::new(Expr::This), n, r as u32, args),
                                None => Expr::MethodCall(Box::new(Expr::This), cls, s, args),
                            }
                        }
                        None => {
                            // * Un nombre que no esta en la tabla de C++ pasa
                            // TAL CUAL, sin manglar. Es el puente con lo de C
                            // --`printf`, `getchar`, los intrinsecos-- y es lo
                            // que `extern "C"` nombra en el estandar: una
                            // funcion de C tiene el simbolo que tiene, porque
                            // el que la escribio no sabia que C++ existia.
                            match self.funciones.get(&n) {
                                Some(firmas) => {
                                    let firmas = firmas.clone();
                                    let tipos = self.tipos_de(&args, &n)?;
                                    let s = self.resolver(&n, &firmas, &tipos)?.simbolo.clone();
                                    Expr::Call(s, args)
                                }
                                None => Expr::Call(n, args),
                            }
                        }
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

                    // Metodo o campo? Se decide con el parentesis, y el
                    // parser ya sabe cual de los dos nombres existe.
                    if *self.peek() == Token::OpenParen {
                        let Some(firmas) = info.metodos.get(&miembro) else {
                            return Err(self.err(format!("`{cls}` no tiene el metodo `{miembro}`")));
                        };
                        self.avanzar();
                        let mut args = Vec::new();
                        if *self.peek() != Token::CloseParen {
                            loop {
                                args.push(self.asignacion()?);
                                if !self.come(&Token::Comma) { break; }
                            }
                        }
                        self.exige(&Token::CloseParen)?;
                        let tipos = self.tipos_de(&args, &miembro)?;
                        let simbolo = self.resolver(
                            &format!("{cls}::{miembro}"), firmas, &tipos)?.simbolo.clone();
                        // El objeto viaja como el `this` que el descenso
                        // pondra de primer parametro. Con `->` la base YA es
                        // un puntero; con `.` hay que tomarle la direccion.
                        let objeto = if flecha { e } else { Expr::AddrOf(Box::new(e)) };
                        // * Si el metodo tiene ranura, la llamada es VIRTUAL:
                        // no va al simbolo que dice el tipo estatico, va al que
                        // haya en esa ranura de la tabla que el objeto lleva
                        // dentro. Es la unica diferencia entre las dos, y esta
                        // decidida aqui, en el parser, que es quien sabe si el
                        // metodo es virtual.
                        e = match info.ranura_de.get(&miembro) {
                            Some(&r) => Expr::VirtualCall(
                                Box::new(objeto), miembro, r as u32, args),
                            None => Expr::MethodCall(Box::new(objeto), cls, simbolo, args),
                        };
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
        // * La linea se captura ANTES de consumir el token. Si se leyera
        // despues, `self.pos` ya apunta al siguiente y el error saldria con la
        // linea de lo que viene detras -- que en `int y = ;` es el `return` de
        // la linea siguiente, y manda a mirar donde no es.
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
            // * Un campo nombrado a secas dentro de un metodo ES `this->campo`.
            //
            // Y el orden importa: primero se mira el ambito local, porque un
            // parametro o una local **tapan** al campo. Al reves, `int doble(int x)`
            // leeria el campo `x` en vez del argumento -- y las dos versiones
            // compilan, asi que el bug seria mudo.
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
