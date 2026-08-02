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
    for cl in &p.classes {
        let mut miembros = Vec::new();
        for m in &cl.members {
            miembros.push(c::StructMember { typ: tipo(&m.typ)?, name: m.name.clone() });
        }
        out.globals.push(c::GlobalDecl::Struct(cl.name.clone(), miembros));

        for m in &cl.methods {
            out.functions.push(metodo(cl, m)?);
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
        out.functions.push(funcion(f)?);
    }

    // ★ Un programa sin `main` no es un programa.
    //
    // Se comprueba AQUÍ y no en C a propósito. BMO C compila un fichero vacío
    // a un BEF de 8 240 bytes sin punto de entrada — es deuda **de C**, y la
    // regla 3 de `HERENCIA.md` dice que lo que le falta a C entra en C con su
    // test y su fila en la matriz DE C, no de rebote desde aquí. Que C++ se
    // defienda de su lado no toca a nadie; arreglarlo dentro de C sería
    // combinarlos.
    if !out.functions.iter().any(|f| f.name == "main") {
        return Err(CppError::new(0,
            "no hay `main`: un programa sin punto de entrada no es un programa"));
    }

    Ok(out)
}

/// **El desazucarado que define C++**: un método es una función libre cuyo
/// primer parámetro es `this`.
///
/// El nombre lleva un punto —`P.doble`— que es **ilegal en C** y por tanto no
/// puede chocar con ninguna función que alguien escriba. Es el mismo truco que
/// BMO C ya usa para promover una `static` de función a global
/// (`funcion.variable`). El mangling de verdad llega en el paso 4, cuando haya
/// sobrecarga y dos métodos distintos necesiten símbolos distintos.
pub fn nombre_de_metodo(clase: &str, metodo: &str) -> String {
    format!("{clase}.{metodo}")
}

fn metodo(cl: &cpp::Class, m: &cpp::Method) -> Result<c::Function, CppError> {
    let this = c::Param {
        typ: c::TypeSpec::Ptr(Box::new(c::TypeSpec::StructRef(cl.name.clone()))),
        name: "this".into(),
    };
    let mut f = funcion(&cpp::Function {
        ret_type: m.ret_type.clone(),
        name: nombre_de_metodo(&cl.name, &m.name),
        params: m.params.clone(),
        body: m.body.clone(),
    })?;
    f.params.insert(0, this.clone());
    f.var_names.insert(0, this.name);
    Ok(f)
}

fn funcion(f: &cpp::Function) -> Result<c::Function, CppError> {
    let mut params = Vec::new();
    for pa in &f.params {
        if pa.default.is_some() {
            return Err(pendiente("los argumentos por defecto", 4, "la resolución de sobrecarga"));
        }
        params.push(c::Param { typ: tipo(&pa.typ)?, name: pa.name.clone() });
    }

    let mut cuerpo = Vec::new();
    for s in &f.body {
        cuerpo.push(stmt(s)?);
    }

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

// ── Sentencias ──────────────────────────────────────────────────────

fn stmt(s: &cpp::Stmt) -> Result<c::Stmt, CppError> {
    use cpp::Stmt as S;
    Ok(match s {
        S::Expr(e) => c::Stmt::Expr(expr(e)?),
        S::Return(None) => c::Stmt::Return(None),
        S::Return(Some(e)) => c::Stmt::Return(Some(expr(e)?)),
        S::DeclVar(t, n, init) => {
            let v = match init { Some(e) => Some(expr(e)?), None => None };
            c::Stmt::DeclAssign(tipo(t)?, n.clone(), v)
        }
        S::Assign(n, e) => c::Stmt::Expr(c::Expr::Assign(n.clone(), Box::new(expr(e)?))),
        S::If(c_, t, e) => c::Stmt::If(
            expr(c_)?,
            Box::new(stmt(t)?),
            match e { Some(x) => Some(Box::new(stmt(x)?)), None => None },
        ),
        S::While(c_, b) => c::Stmt::While(expr(c_)?, Box::new(stmt(b)?)),
        S::DoWhile(b, c_) => c::Stmt::DoWhile(Box::new(stmt(b)?), expr(c_)?),
        S::Switch(sujeto, casos) => {
            let mut out = Vec::new();
            for k in casos {
                let mut cuerpo = Vec::new();
                for s in &k.stmts { cuerpo.push(stmt(s)?); }
                out.push(c::Case { value: k.value, stmts: cuerpo });
            }
            c::Stmt::Switch(expr(sujeto)?, out)
        }
        S::For(a, b, c_, cuerpo) => c::Stmt::For(
            opt_expr(a)?, opt_expr(b)?, opt_expr(c_)?, Box::new(stmt(cuerpo)?),
        ),
        S::Block(v) => {
            let mut out = Vec::new();
            for x in v { out.push(stmt(x)?); }
            c::Stmt::Block(out)
        }
        S::Break => c::Stmt::Break,
        S::Continue => c::Stmt::Continue,

        S::Delete(_) => return Err(pendiente("`delete`", 3,
            "el destructor, y encima la capability de memoria")),
    })
}

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
            c::Expr::Call(nombre_de_metodo(cls, m), a)
        }

        // ── Rechazos con el paso donde llegan ──
        E::VirtualCall(_, m, _, _) => return Err(pendiente(&format!("la llamada virtual `{m}`"), 5,
            "la vtable")),
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
