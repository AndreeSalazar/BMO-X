//! **El descenso**: AST de C++ → `bmo_c_front::ast::Program`.
//!
//! ═══ Qué es este fichero y qué NO es ═══
//!
//! Es **la frontera entera** entre BMO C++ y BMO C, y es a propósito lo más
//! tonto que se puede escribir: una traducción nodo a nodo, sin decisiones.
//! Lo que sale de aquí es un `Program` de C —un **formato**— y quien lo recibe
//! (`bmo_c_front::codegen`) no sabe ni sabrá que existe una clase.
//!
//! Lo que **no** es: no es un puente para que C aprenda C++. La flecha apunta
//! en un solo sentido y `lang/c` no se entera de que este crate existe. Las
//! cuatro reglas están en `HERENCIA.md`.
//!
//! ═══ Cómo lo hicieron los maestros ═══
//!
//! **Cfront** (Bjarne, 1983–1993) hacía exactamente esto, y de ahí sale la
//! forma: clase → `struct`, método → función con `this` de primer parámetro,
//! virtual → array de punteros a función. Murió por las excepciones, por el
//! prelinker de plantillas y porque el depurador veía el C generado.
//!
//! ★ Aquí **ninguna de las tres aplica**: no hay excepciones (descartadas con
//! motivo), se compila **una sola unidad de traducción** con monomorfización en
//! el frontend, y **no se emite texto C** — se emite el AST, así que nunca hay
//! un `.c` intermedio que confunda a nadie. El estudio completo está en
//! `MAESTROS.md`.
//!
//! ═══ La regla que gobierna cada rama ═══
//!
//! > Lo que se puede bajar, se baja. **Lo que no, se RECHAZA diciendo en qué
//! > paso llega.** Nunca en silencio.
//!
//! Esto es lo que arregla el pecado del frontend anterior, cuyo `parse_body`
//! hacía `pos += 1` con todo lo que no reconocía: un cuerpo entero podía
//! desaparecer y el programa "compilaba". La regla de BMO es *nada que compile
//! y no haga lo que dice*, y un descenso parcial en silencio la rompe más
//! fuerte que un error.
//!
//! Por eso hay casos que se rechazan **aunque el tipo encajase**. Una
//! referencia `T&` es un puntero, y mapearla a `Ptr` compilaría; pero sin la
//! indirección automática en cada uso —que es trabajo del paso 2— el programa
//! leería la dirección donde debía leer el valor. Compilaría, y haría otra
//! cosa. Se rechaza.

use bmo_c_front::ast as c;
use crate::ast as cpp;
use crate::CppError;
use std::collections::HashMap;

// ── RAII: la pila de limpieza ───────────────────────────────────────
//
// ═══ Cómo lo hace Clang, y qué se le quita ═══
//
// `CGClass.cpp` lleva una pila de *cleanups* por ámbito (`EHScopeStack`) y la
// ejecuta en cada salida. **Con excepciones** eso se bifurca en dos caminos
// —el normal y el de desenrollado— y ahí es donde se vuelve caro: cada ámbito
// necesita una tabla que diga qué hay vivo para poder destruirlo desde un
// `throw` que venga de cualquier profundidad.
//
// ★ **Sin excepciones colapsa a una lista por ámbito que se recorre al revés
// en cada salida**, y las salidas son cuatro y están todas a la vista:
// el final de las llaves, `return`, `break` y `continue`. Eso es lo que se
// implementa aquí, y cabe en una pantalla.
//
// El orden inverso no es una preferencia: es el lenguaje. Si `a` se construyó
// antes que `b`, `b` puede depender de `a`, así que `b` se destruye primero.

/// A qué salida corta un `break` o un `continue`.
#[derive(Clone, Copy, PartialEq)]
enum Corte {
    /// Un bloque normal: ni `break` ni `continue` paran aquí.
    Ninguno,
    /// Un bucle: paran los dos.
    Bucle,
    /// Un `switch`: para `break`, **no** `continue`. Ésa es justo la
    /// diferencia entre los dos, y meterlos en el mismo saco haría que un
    /// `continue` dentro de un `switch` dentro de un bucle destruyera de menos.
    Switch,
}

struct Ambito {
    /// *(variable, clase)* en orden de construcción.
    objetos: Vec<(String, String)>,
    corte: Corte,
}

/// Lo que el descenso necesita saber de una clase para insertar llamadas.
#[derive(Default, Clone, Copy)]
struct Info {
    ctor: bool,
    dtor: bool,
    /// ¿Los objetos de esta clase llevan `vptr`? Si sí, hay que apuntarlo a su
    /// tabla al construir — y **antes** de llamar al constructor, porque el
    /// constructor puede llamar a un método virtual de sí mismo.
    vtabla: bool,
}

// Los nombres emitidos salen de `crate::mangling`, que es el ÚNICO sitio del
// crate donde se decide cómo se llama un símbolo. Antes estaban aquí a mano
// (`P.P`, `P.~P`) y coincidían con los del mangling por casualidad: ahora
// coinciden porque son la misma función.
use crate::mangling;

/// El nombre de la global que guarda la vtabla de una clase.
///
/// Lleva un punto —ilegal en C++— para que no pueda chocar con una variable
/// del programa, igual que todo lo demás que genera este crate.
fn nombre_vtabla(clase: &str) -> String { format!("vtabla.{clase}") }

/// Baja el cuerpo de una función insertando construcciones y destrucciones.
struct Cuerpo<'a> {
    clases: &'a HashMap<String, Info>,
    pila: Vec<Ambito>,
    /// Contador de temporales. El nombre lleva un punto —ilegal en C++— para
    /// que no pueda chocar con una variable del programa.
    temp: u32,
    /// El tipo que devuelve la función, para poder declarar el temporal del
    /// `return` cuando haya destructores que ejecutar antes de salir.
    ret: cpp::TypeSpec,
}

impl<'a> Cuerpo<'a> {
    fn nuevo(clases: &'a HashMap<String, Info>, ret: cpp::TypeSpec) -> Self {
        Self { clases, pila: Vec::new(), temp: 0, ret }
    }

    /// Las destrucciones de un ámbito, **en orden inverso al de construcción**.
    fn destruir(a: &Ambito) -> Vec<c::Stmt> {
        a.objetos.iter().rev().map(|(v, cl)| {
            c::Stmt::Expr(c::Expr::Call(
                mangling::destructor(&[], cl),
                vec![c::Expr::AddrOf(Box::new(c::Expr::Var(v.clone())))],
            ))
        }).collect()
    }

    /// Las destrucciones desde el ámbito actual hasta `hasta` (incluido),
    /// contando desde dentro hacia fuera.
    fn destruir_hasta(&self, hasta: usize) -> Vec<c::Stmt> {
        let mut out = Vec::new();
        for a in self.pila[hasta..].iter().rev() {
            out.extend(Self::destruir(a));
        }
        out
    }

    /// El ámbito donde para un `break` (bucle o `switch`) o un `continue`
    /// (sólo bucle). `None` si no hay ninguno — lo que significa un `break`
    /// suelto, que el codegen de C ya rechaza por su cuenta.
    fn objetivo(&self, solo_bucle: bool) -> Option<usize> {
        self.pila.iter().rposition(|a| match a.corte {
            Corte::Bucle => true,
            Corte::Switch => !solo_bucle,
            Corte::Ninguno => false,
        })
    }

    /// Un bloque completo: entra en un ámbito, baja las sentencias, y si no
    /// se salió por la puerta de atrás, destruye lo que quede vivo.
    fn bloque(&mut self, ss: &[cpp::Stmt], corte: Corte) -> Result<Vec<c::Stmt>, CppError> {
        self.pila.push(Ambito { objetos: Vec::new(), corte });
        let mut out = Vec::new();
        let mut cortado = false;
        for s in ss {
            if cortado {
                // Código detrás de un `return`/`break`/`continue`. No se emite
                // —nunca se ejecutaría— pero tampoco se calla: emitirlo
                // pondría destrucciones detrás de la salida.
                return Err(CppError::new(0,
                    "hay sentencias detras de un `return`, `break` o `continue`: \
                     nunca se ejecutarian"));
            }
            cortado = matches!(s, cpp::Stmt::Return(_) | cpp::Stmt::Break | cpp::Stmt::Continue);
            out.extend(self.stmt(s)?);
        }
        if !cortado {
            let a = self.pila.last().unwrap();
            out.extend(Self::destruir(a));
        }
        self.pila.pop();
        Ok(out)
    }

    /// Un ámbito de una sola sentencia (el cuerpo de un `if` sin llaves, por
    /// ejemplo). Se envuelve igual para que las reglas sean las mismas.
    fn anidado(&mut self, s: &cpp::Stmt, corte: Corte) -> Result<c::Stmt, CppError> {
        match s {
            cpp::Stmt::Block(v) => Ok(c::Stmt::Block(self.bloque(v, corte)?)),
            otro => Ok(c::Stmt::Block(self.bloque(std::slice::from_ref(otro), corte)?)),
        }
    }

    fn stmt(&mut self, s: &cpp::Stmt) -> Result<Vec<c::Stmt>, CppError> {
        use cpp::Stmt as S;
        Ok(match s {
            // ── La construcción ──
            //
            // El parser ya eligió QUÉ constructor: tenía delante los tipos de
            // los argumentos, que es lo que hace falta para resolver la
            // sobrecarga. Aquí sólo se emite — se reserva el hueco y se llama
            // con `&objeto` de primer parámetro.
            S::DeclObj { clase, nombre, ctor, args } => {
                let info = self.clases.get(clase).copied().unwrap_or_default();
                let mut out = vec![c::Stmt::DeclAssign(
                    c::TypeSpec::StructRef(clase.clone()), nombre.clone(), None)];
                // ★ El `vptr` se apunta ANTES de llamar al constructor: un
                // constructor puede llamar a un metodo virtual de si mismo, y
                // si la tabla no estuviera puesta llamaria a la nada.
                if info.vtabla {
                    out.push(c::Stmt::Expr(c::Expr::AssignField(
                        Box::new(c::Expr::Var(nombre.clone())),
                        crate::parser::VPTR.into(), 0,
                        c::TypeSpec::Ptr(Box::new(c::TypeSpec::Void)),
                        Box::new(c::Expr::Var(nombre_vtabla(clase))),
                    )));
                }
                if let Some(simbolo) = ctor {
                    let mut a = vec![c::Expr::AddrOf(Box::new(c::Expr::Var(nombre.clone())))];
                    for x in args { a.push(expr(x)?); }
                    out.push(c::Stmt::Expr(c::Expr::Call(simbolo.clone(), a)));
                }
                if info.dtor {
                    self.pila.last_mut().unwrap().objetos.push((nombre.clone(), clase.clone()));
                }
                out
            }

            // ── Las salidas ──
            S::Return(v) => {
                let limpieza = self.destruir_hasta(0);
                match (v, limpieza.is_empty()) {
                    (_, true) => vec![c::Stmt::Return(match v {
                        Some(e) => Some(expr(e)?), None => None,
                    })],
                    (None, false) => {
                        let mut out = limpieza;
                        out.push(c::Stmt::Return(None));
                        out
                    }
                    // ★ El valor se calcula ANTES de destruir. `return
                    // p.valor();` con `p` a punto de morir tiene que leer el
                    // objeto vivo — si el destructor corriera primero, se
                    // devolvería lo que quedara en la pila.
                    (Some(e), false) => {
                        self.temp += 1;
                        let t = format!("ret.{}", self.temp);
                        let mut out = vec![c::Stmt::DeclAssign(
                            tipo(&self.ret)?, t.clone(), Some(expr(e)?))];
                        out.extend(limpieza);
                        out.push(c::Stmt::Return(Some(c::Expr::Var(t))));
                        out
                    }
                }
            }
            S::Break | S::Continue => {
                let solo_bucle = matches!(s, S::Continue);
                let mut out = match self.objetivo(solo_bucle) {
                    Some(i) => self.destruir_hasta(i),
                    None => Vec::new(),
                };
                out.push(if solo_bucle { c::Stmt::Continue } else { c::Stmt::Break });
                out
            }

            // ── Lo que abre ámbito ──
            S::Block(v) => vec![c::Stmt::Block(self.bloque(v, Corte::Ninguno)?)],
            S::If(c_, t, e) => vec![c::Stmt::If(
                expr(c_)?,
                Box::new(self.anidado(t, Corte::Ninguno)?),
                match e { Some(x) => Some(Box::new(self.anidado(x, Corte::Ninguno)?)), None => None },
            )],
            S::While(c_, b) => vec![c::Stmt::While(
                expr(c_)?, Box::new(self.anidado(b, Corte::Bucle)?))],
            S::DoWhile(b, c_) => vec![c::Stmt::DoWhile(
                Box::new(self.anidado(b, Corte::Bucle)?), expr(c_)?)],
            S::For(a, b, c_, cuerpo) => vec![c::Stmt::For(
                opt_expr(a)?, opt_expr(b)?, opt_expr(c_)?,
                Box::new(self.anidado(cuerpo, Corte::Bucle)?),
            )],
            S::Switch(sujeto, casos) => {
                let sujeto = expr(sujeto)?;
                self.pila.push(Ambito { objetos: Vec::new(), corte: Corte::Switch });
                let mut out = Vec::new();
                for k in casos {
                    let mut cuerpo = Vec::new();
                    for s in &k.stmts { cuerpo.extend(self.stmt(s)?); }
                    out.push(c::Case { value: k.value, stmts: cuerpo });
                }
                self.pila.pop();
                vec![c::Stmt::Switch(sujeto, out)]
            }

            // ── Lo demás, tal cual ──
            S::Expr(e) => vec![c::Stmt::Expr(expr(e)?)],
            S::DeclVar(t, n, init) => {
                let v = match init { Some(e) => Some(expr(e)?), None => None };
                vec![c::Stmt::DeclAssign(tipo(t)?, n.clone(), v)]
            }
            S::Assign(n, e) => vec![c::Stmt::Expr(
                c::Expr::Assign(n.clone(), Box::new(expr(e)?)))],
            S::Delete(_) => return Err(pendiente("`delete`", 3,
                "un asignador de memoria sobre `KIND_MEMORIA`, que todavia no existe")),
        })
    }
}

/// Traduce un programa de C++ al `Program` de BMO C que el codegen entiende.
pub fn descender(p: &cpp::Program) -> Result<c::Program, CppError> {
    if !p.includes.is_empty() {
        return Err(pendiente("#include", 1, "el preprocesador de C++"));
    }
    if !p.namespaces.is_empty() {
        return Err(pendiente("los namespaces", 4, "el mangling"));
    }

    let mut out = c::Program::new();

    // ── Las clases: un `struct` y una función suelta por método ──
    //
    // ★ Es Cfront, literalmente. Y el `struct` se emite con sus miembros
    // **sin offsets**, para que el codegen de C recalcule la disposición con
    // SU regla. El parser de C++ ya la calculó para poder poner el offset
    // dentro de cada `Field`; emitir aquí los offsets en vez de los miembros
    // haría que la única copia que manda fuera la de C++, y el día que las
    // dos reglas divergieran nadie se enteraría. Así, si divergen, el valor
    // sale mal y la matriz se pone roja.
    // Quién tiene constructor y quién destructor. Se recoge ANTES de bajar
    // ningún cuerpo, porque una clase puede usarse en un método de otra que se
    // declaró antes.
    let mut info: HashMap<String, Info> = HashMap::new();
    for cl in &p.classes {
        info.insert(cl.name.clone(), Info {
            ctor: !cl.constructors.is_empty(),
            dtor: cl.destructor.is_some(),
            vtabla: !cl.vtabla.is_empty(),
        });
    }

    // Las tablas virtuales: una global de `n` ranuras por clase con virtuales,
    // y las instrucciones que la rellenan. No se pueden emitir como un
    // inicializador estático porque **las globales de BMO C sólo admiten un
    // entero**, y la dirección de una función no se conoce hasta emitir el
    // código. Se rellenan al principio de `main`, que es el único sitio por el
    // que pasa todo programa antes de construir nada.
    let mut relleno: Vec<c::Stmt> = Vec::new();
    for cl in &p.classes {
        if cl.vtabla.is_empty() { continue; }
        let n = cl.vtabla.len() as u32;
        out.globals.push(c::GlobalDecl::Var(
            c::TypeSpec::Array(Box::new(c::TypeSpec::Ptr(Box::new(c::TypeSpec::Void))), n),
            nombre_vtabla(&cl.name),
            None,
        ));
        for (i, simbolo) in cl.vtabla.iter().enumerate() {
            relleno.push(c::Stmt::Expr(c::Expr::AssignSubscript(
                nombre_vtabla(&cl.name),
                Box::new(c::Expr::Int(i as i64)),
                8,
                Box::new(c::Expr::Var(simbolo.clone())),
            )));
        }
    }

    for cl in &p.classes {
        let mut miembros = Vec::new();
        // El `vptr` es un campo más, y va el PRIMERO. El parser ya lo colocó
        // en el offset 0; aquí sólo hay que declararlo para que el codegen de
        // C le reserve su sitio.
        if !cl.vtabla.is_empty() {
            miembros.push(c::StructMember {
                typ: c::TypeSpec::Ptr(Box::new(c::TypeSpec::Void)),
                name: crate::parser::VPTR.into(),
            });
        }
        for m in &cl.members {
            miembros.push(c::StructMember { typ: tipo(&m.typ)?, name: m.name.clone() });
        }
        out.globals.push(c::GlobalDecl::Struct(cl.name.clone(), miembros));

        for m in &cl.methods {
            out.functions.push(metodo(cl, m, &info)?);
        }
        // El constructor y el destructor son **funciones normales** con `this`.
        // Ahí acaba toda la magia: lo único especial de ellos es QUIÉN las
        // llama y CUÁNDO, y eso lo decide `Cuerpo`.
        for ctor in &cl.constructors {
            let mut f = metodo(cl, ctor, &info)?;
            f.name = mangling::constructor(&[], &cl.name,
                &ctor.params.iter().map(|p| p.typ.clone()).collect::<Vec<_>>());
            out.functions.push(f);
        }
        if let Some(dtor) = &cl.destructor {
            let mut f = metodo(cl, dtor, &info)?;
            f.name = mangling::destructor(&[], &cl.name);
            out.functions.push(f);
        }
    }

    for g in &p.globals {
        let cpp::GlobalDecl::Var(ts, nombre, init) = g;
        let tipo = tipo(ts)?;
        let valor = match init {
            Some(e) => Some(expr(e)?),
            None => None,
        };
        out.globals.push(c::GlobalDecl::Var(tipo, nombre.clone(), valor));
    }

    for f in &p.functions {
        out.functions.push(funcion(f, &info)?);
    }

    // ★ Un programa sin `main` no es un programa.
    //
    // Se comprueba AQUÍ y no en C a propósito. BMO C compila un fichero vacío
    // a un BEF de 8 240 bytes sin punto de entrada — es deuda **de C**, y la
    // regla 3 de `HERENCIA.md` dice que lo que le falta a C entra en C con su
    // test y su fila en la matriz DE C, no de rebote desde aquí. Que C++ se
    // defienda de su lado no toca a nadie; arreglarlo dentro de C sería
    // combinarlos.
    let Some(main) = out.functions.iter_mut().find(|f| f.name == "main") else {
        return Err(CppError::new(0,
            "no hay `main`: un programa sin punto de entrada no es un programa"));
    };
    // Las tablas se rellenan lo PRIMERO de todo, antes de cualquier sentencia
    // del programa: construir un objeto ya necesita que su tabla exista.
    if !relleno.is_empty() {
        relleno.extend(std::mem::take(&mut main.body));
        main.body = relleno;
    }

    Ok(out)
}

fn metodo(cl: &cpp::Class, m: &cpp::Method, info: &HashMap<String, Info>)
    -> Result<c::Function, CppError>
{
    let this = c::Param {
        typ: c::TypeSpec::Ptr(Box::new(c::TypeSpec::StructRef(cl.name.clone()))),
        name: "this".into(),
    };
    let mut f = funcion(&cpp::Function {
        ret_type: m.ret_type.clone(),
        name: mangling::metodo(&[], &cl.name, &m.name,
            &m.params.iter().map(|p| p.typ.clone()).collect::<Vec<_>>()),
        params: m.params.clone(),
        body: m.body.clone(),
    }, info)?;
    f.params.insert(0, this.clone());
    f.var_names.insert(0, this.name);
    Ok(f)
}

fn funcion(f: &cpp::Function, info: &HashMap<String, Info>) -> Result<c::Function, CppError> {
    let mut params = Vec::new();
    for pa in &f.params {
        if pa.default.is_some() {
            return Err(pendiente("los argumentos por defecto", 4, "la resolución de sobrecarga"));
        }
        params.push(c::Param { typ: tipo(&pa.typ)?, name: pa.name.clone() });
    }

    // El cuerpo entero es un ámbito: lo que se declare aquí se destruye al
    // salir, y `Cuerpo` se encarga de que también se destruya en cada `return`.
    let mut cu = Cuerpo::nuevo(info, f.ret_type.clone());
    let cuerpo = cu.bloque(&f.body, Corte::Ninguno)?;

    // `var_names` es el camino LEGADO de C: `build_var_map` saca las locales
    // recorriendo el cuerpo (`collect_decls_stmt`), que es donde está el tipo
    // real. Aquí se rellena igual que lo hace el parser de C —parámetros
    // primero, luego las declaradas— para no depender de cuál de los dos
    // caminos gane el día que alguien toque el otro.
    let mut var_names: Vec<String> = params.iter().map(|p| p.name.clone()).collect();
    let mut var_count = 0u32;
    for s in &cuerpo {
        if let c::Stmt::DeclAssign(_, n, _) = s {
            var_names.push(n.clone());
            var_count += 1;
        }
    }

    Ok(c::Function {
        ret_type: tipo(&f.ret_type)?,
        name: f.name.clone(),
        params,
        var_count,
        var_names,
        body: cuerpo,
        line: 0,
        variadica: false,
    })
}

// ── Tipos ───────────────────────────────────────────────────────────

fn tipo(t: &cpp::TypeSpec) -> Result<c::TypeSpec, CppError> {
    use cpp::TypeSpec as T;
    Ok(match t {
        T::Void => c::TypeSpec::Void,
        T::Char => c::TypeSpec::Char,
        T::Short => c::TypeSpec::Short,
        T::Int => c::TypeSpec::Int,
        T::Long => c::TypeSpec::Long,
        T::LongLong => c::TypeSpec::LongLong,
        T::UnsignedChar => c::TypeSpec::UnsignedChar,
        T::UnsignedShort => c::TypeSpec::UnsignedShort,
        T::UnsignedInt => c::TypeSpec::UnsignedInt,
        T::UnsignedLong => c::TypeSpec::UnsignedLong,
        T::UnsignedLongLong => c::TypeSpec::UnsignedLongLong,
        T::Float => c::TypeSpec::Float,
        T::Double => c::TypeSpec::Double,
        // `bool` no existe en el AST de C. Un byte con 0 o 1 es lo que emite
        // cualquiera, y `BoolLit` baja a `Int(0)`/`Int(1)` más abajo.
        T::Bool => c::TypeSpec::Char,
        T::Ptr(t) => c::TypeSpec::Ptr(Box::new(tipo(t)?)),
        T::Array(t, n) => c::TypeSpec::Array(Box::new(tipo(t)?), *n),

        // ── Rechazos con el paso donde llegan ──
        //
        // `Ref` NO se mapea a `Ptr` aunque quepa: sin la indirección
        // automática en cada uso, el programa leería la dirección en lugar
        // del valor. Compilaría y haría otra cosa, que es peor que no
        // compilar.
        T::Ref(_) => return Err(pendiente("las referencias `T&`", 2,
            "la indirección automática en cada uso")),
        T::ClassRef(n) => c::TypeSpec::StructRef(n.clone()),
        T::Template(n, _) => return Err(pendiente(&format!("la plantilla `{n}`"), 6,
            "la monomorfización")),
        T::Auto => return Err(pendiente("`auto`", 1,
            "la tabla de símbolos que sabe el tipo")),
    })
}

// ── Sentencias sueltas ──────────────────────────────────────────────
//
// El descenso de sentencias vive en `Cuerpo`, arriba: desde el paso 3 una
// sentencia puede convertirse en VARIAS (declarar + construir, destruir +
// salir), y una funcion que devuelve un solo `Stmt` ya no puede expresarlo.

fn opt_expr(e: &Option<cpp::Expr>) -> Result<Option<c::Expr>, CppError> {
    match e { Some(x) => Ok(Some(expr(x)?)), None => Ok(None) }
}

// ── Expresiones ─────────────────────────────────────────────────────

fn expr(e: &cpp::Expr) -> Result<c::Expr, CppError> {
    use cpp::Expr as E;
    // Atajo para los binarios: los dos lados bajan igual en todos.
    macro_rules! bin {
        ($v:ident, $a:expr, $b:expr) => {
            c::Expr::$v(Box::new(expr($a)?), Box::new(expr($b)?))
        };
    }
    Ok(match e {
        E::Int(v) => c::Expr::Int(*v),
        E::FloatLit(v) => c::Expr::FloatLit(*v),
        E::StringLit(s) => c::Expr::StringLit(s.clone()),
        E::CharLit(b) => c::Expr::CharLit(*b),
        E::BoolLit(b) => c::Expr::Int(if *b { 1 } else { 0 }),
        // `nullptr` es un puntero nulo, y en la máquina eso es un cero. Lo
        // que `nullptr` aporta sobre `NULL` —que no se convierte solo a
        // entero— es comprobación del frontend, y cuesta cero al emitir.
        E::NullPtr => c::Expr::Int(0),
        E::Var(n) => c::Expr::Var(n.clone()),
        E::Call(n, args) => {
            let mut a = Vec::new();
            for x in args { a.push(expr(x)?); }
            c::Expr::Call(n.clone(), a)
        }
        E::Assign(n, v) => c::Expr::Assign(n.clone(), Box::new(expr(v)?)),

        E::Add(a, b) => bin!(Add, a, b),
        E::Sub(a, b) => bin!(Sub, a, b),
        E::Mul(a, b) => bin!(Mul, a, b),
        E::Div(a, b) => bin!(Div, a, b),
        E::Mod(a, b) => bin!(Mod, a, b),
        E::Eq(a, b) => bin!(Eq, a, b),
        E::Neq(a, b) => bin!(Neq, a, b),
        E::Lt(a, b) => bin!(Lt, a, b),
        E::Gt(a, b) => bin!(Gt, a, b),
        E::Le(a, b) => bin!(Le, a, b),
        E::Ge(a, b) => bin!(Ge, a, b),
        E::And(a, b) => bin!(LAnd, a, b),
        E::Or(a, b) => bin!(LOr, a, b),
        E::BitAnd(a, b) => bin!(BitAnd, a, b),
        E::BitOr(a, b) => bin!(BitOr, a, b),
        E::BitXor(a, b) => bin!(BitXor, a, b),
        E::Shl(a, b) => bin!(Shl, a, b),
        E::Shr(a, b) => bin!(Shr, a, b),

        E::Subscript(n, idx, esc) =>
            c::Expr::Subscript(n.clone(), Box::new(expr(idx)?), *esc),
        E::AssignSubscript(n, idx, esc, v) =>
            c::Expr::AssignSubscript(n.clone(), Box::new(expr(idx)?), *esc, Box::new(expr(v)?)),
        E::AssignDeref(p, v) =>
            c::Expr::AssignDeref(Box::new(expr(p)?), Box::new(expr(v)?)),
        E::Cast(t, e) => c::Expr::Cast(tipo(t)?, Box::new(expr(e)?)),

        E::Not(a) => c::Expr::Not(Box::new(expr(a)?)),
        E::Neg(a) => c::Expr::Neg(Box::new(expr(a)?)),
        E::BitNot(a) => c::Expr::BitNot(Box::new(expr(a)?)),
        E::Deref(a) => c::Expr::Deref(Box::new(expr(a)?)),
        E::AddrOf(a) => c::Expr::AddrOf(Box::new(expr(a)?)),

        E::PreInc(n) => c::Expr::PreInc(n.clone()),
        E::PreDec(n) => c::Expr::PreDec(n.clone()),
        E::PostInc(n) => c::Expr::PostInc(n.clone()),
        E::PostDec(n) => c::Expr::PostDec(n.clone()),

        E::Conditional(c_, a, b) => c::Expr::Conditional(
            Box::new(expr(c_)?), Box::new(expr(a)?), Box::new(expr(b)?),
        ),

        // ── Clases (paso 2) ──
        //
        // `this` es un parámetro más, así que baja a una variable con ese
        // nombre. Ahí acaba toda la magia del puntero implícito de C++.
        E::This => c::Expr::Var("this".into()),
        E::MemberAccess(b, n, off, t) =>
            c::Expr::Field(Box::new(expr(b)?), n.clone(), *off, tipo(t)?),
        E::Arrow(b, n, off, t) =>
            c::Expr::Arrow(Box::new(expr(b)?), n.clone(), *off, tipo(t)?),
        E::AssignMember(b, n, off, t, v) =>
            c::Expr::AssignField(Box::new(expr(b)?), n.clone(), *off, tipo(t)?, Box::new(expr(v)?)),
        E::AssignArrow(b, n, off, t, v) =>
            c::Expr::AssignArrow(Box::new(expr(b)?), n.clone(), *off, tipo(t)?, Box::new(expr(v)?)),
        // `objeto.metodo(a, b)` → `Clase.metodo(&objeto, a, b)`. El parser ya
        // puso el `&` (o lo omitió si la base venía de `->`), así que aquí no
        // se decide nada: se ordenan los argumentos.
        E::MethodCall(objeto, cls, m, args) => {
            let mut a = vec![expr(objeto)?];
            for x in args { a.push(expr(x)?); }
            let _ = cls;
            c::Expr::Call(m.clone(), a)
        }

        // ── Rechazos con el paso donde llegan ──
        // ★ **El despacho virtual, entero.**
        //
        //   objeto->vptr        el objeto lleva dentro la tabla de SU tipo
        //   tabla[ranura]       la ranura la fijó el parser
        //   (…)(objeto, args)   y se llama por el puntero que salga
        //
        // Es exactamente lo que se escribiría a mano en C, y por eso Bjarne
        // pudo implementarlo como una traducción. Dos objetos del mismo tipo
        // estático con tablas distintas ejecutan funciones distintas en la
        // misma línea de código: eso es una función virtual y no hace falta
        // nada más.
        E::VirtualCall(objeto, _, ranura, args) => {
            let obj = expr(objeto)?;
            let tabla = c::Expr::Arrow(
                Box::new(obj.clone()), crate::parser::VPTR.into(), 0,
                c::TypeSpec::Ptr(Box::new(c::TypeSpec::Void)));
            let destino = c::Expr::IndexPtr(
                Box::new(tabla), Box::new(c::Expr::Int(*ranura as i64)), c::TypeSpec::Long);
            let mut a = vec![obj];
            for x in args { a.push(expr(x)?); }
            c::Expr::CallPtr(Box::new(destino), a)
        }
        E::New(cl, _) => return Err(pendiente(&format!("`new {cl}`"), 3,
            "el constructor, y encima la capability de memoria")),
        E::TemplateCall(n, _, _) => return Err(pendiente(&format!("la plantilla `{n}`"), 6,
            "la monomorfización")),
        E::Syscall(d, _) => return Err(pendiente(&format!("la puerta `{}`", d.name), 1,
            "los intrínsecos, que en C son filas de `intrinsics.toml`")),
    })
}

// ── El error ────────────────────────────────────────────────────────

/// Un rechazo que **dice en qué paso llega** lo que falta.
///
/// La línea va a 0 porque el AST de hoy no lleva posiciones: el parser del
/// paso 1 las añade, y entonces esto pasa a decir dónde. Mientras tanto es
/// preferible un error sin línea a un silencio con línea.
fn pendiente(que: &str, paso: u8, necesita: &str) -> CppError {
    CppError::new(0, format!(
        "{que}: llega en el PASO {paso} — necesita {necesita}. \
         El orden completo está en toolchain/lang/cpp/BRECHA.md"
    ))
}
