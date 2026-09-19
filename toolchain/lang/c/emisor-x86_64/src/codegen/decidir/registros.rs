//! **EL TROQUEL** -- que valores caben en la matriz de registros.
//!
//! [fase]     EMISION
//!
//! [aparece]  BANCO -- decide, no emite. Un error suyo pone en un registro algo
//!            que no debia, y eso lo caza el banco al primer programa que lea
//!            esa variable por dos caminos
//!
//! [carril]   VERDE -- y NO se elige: sale de su `[aparece]` (BANCO).
//!            Ver la tabla en toolchain/tools/fases/fases.py
//!
//! # De donde sale, y el vocabulario ya venia hecho
//!
//! `docs/plan/PLAN_EL_TROQUEL.md`. El dueno lo nombro asi y con el llegaron las
//! tres palabras:
//!
//! ```text
//!    el DIBUJO del troquel   la arquitectura: x86-64 nombra 16 registros
//!    la MATRIZ               los huecos que este emisor puede usar: r12..r15
//!    la REBARBA              lo que no cupo y se va a la pila -- `spill`
//! ```
//!
//! # *** POR QUE `r12`..`r15` Y NO OTROS, comprobado y no supuesto
//!
//! Se conto que registros extendidos emite hoy BMO C: **`r8`, `r9`, `r10` y
//! `r11`, y ninguno mas** (`entrada.rs`, `format.rs`, `intrinsics.rs`,
//! `sintetizadas.rs`). Los cuatro son de los que PISA una llamada, asi que el
//! emisor los usa como temporales y esta bien.
//!
//! `r12`..`r15` estaban **enteros sin usar**, y ademas son de los que una
//! llamada PRESERVA: un valor que vive ahi sobrevive a un `call` sin que nadie
//! lo guarde en medio. Por eso son la matriz y no `r8`..`r11`.
//!
//! [!] Y por eso hay que guardarlos y devolverlos en el prologo y el epilogo:
//! si esta funcion los usa, es ella quien le debe su contenido a quien la llamo.
//!
//! # *** LA REGLA DE SEGURIDAD, Y ES UNA SOLA FRASE
//!
//! > **Una variable local cuya direccion nunca se toma no la puede pisar ningun
//! > puntero.**
//!
//! No hay analisis de alias, no hay grafo, no hay nada: es una propiedad que se
//! ve mirando si en la funcion aparece un `&`. Y cubre justo lo que importa --
//! contadores de bucle, indices, punteros que caminan.
//!
//! ## Y la duda se resuelve por el lado que solo cuesta
//!
//! El escaner de abajo contesta *"de QUE variables se toma la direccion?"*, y
//! en la duda contesta **de todas**: una forma del arbol que no sepa desmontar
//! --un `switch`, hoy-- deja la funcion entera en la pila, **correcta y
//! lenta**, que es el lado barato del error.
//!
//! *** Es la misma decision que `PTE_NUESTRA` en el kernel: *"la duda se
//! resuelve por el lado que solo cuesta RAM"*. Aqui solo cuesta velocidad.
//!
//! ** Hasta el 2026-09-18 la pregunta era *"hay ALGUN `&` en la funcion?"*, y
//! con un si la funcion entera se quedaba sin matriz. El metro lo destapo con
//! `blit`: `main` llama a `bmo_pantalla_abrir(&p)` una vez, y por ese `&p` el
//! contador `i` de un bucle de 6.400 vueltas vivia en `[rbp-0x68]`. La regla
//! siempre fue POR VARIABLE --*"una local cuya direccion nunca se toma"*-- y
//! el escaner la aplicaba por funcion. Ahora devuelve los NOMBRES.
//!
//! # Lo que este fichero NO hace, y hay que decirlo
//!
//! ```text
//!    [ ] no mira las VIDAS. Una variable con registro lo tiene la funcion
//!        entera, aunque solo se use en tres lineas. Eso es `C4` de verdad --
//!        analisis de vivos-- y es otro proyecto
//!    [ ] no reparte por frecuencia real: ordena por cuantas veces APARECE el
//!        nombre, que es una aproximacion. Un uso dentro de un bucle vale mas
//!        que diez fuera, y esto no lo sabe
//!    [ ] no toca los PARAMETROS. Llegan en la pila del llamante y moverlos es
//!        otra conversacion
//! ```

use crate::ast::{Expr, Stmt, TypeSpec};

/// Los huecos de la matriz, en el orden en que se reparten.
///
/// Cuatro y no cinco: `rbx` tambien lo preserva la llamada, pero dejarlo fuera
/// mantiene el numero PAR, y un numero par de `push` no cambia la paridad de
/// alineacion de la pila que el resto del emisor ya tiene.
pub(in crate::codegen) const MATRIZ: [u8; 4] = [12, 13, 14, 15];

/// Cabe un valor de este tipo en un hueco de la matriz?
///
/// Escalares de ocho bytes o menos. Un agregado no cabe, y un flotante vive en
/// `xmm` por otro camino entero (`floats.rs`): meterlo aqui seria mezclar dos
/// ficheros de registros distintos.
pub(in crate::codegen) fn cabe(t: &TypeSpec) -> bool {
    matches!(
        t,
        TypeSpec::Char
            | TypeSpec::UnsignedChar
            | TypeSpec::Short
            | TypeSpec::UnsignedShort
            | TypeSpec::Int
            | TypeSpec::UnsignedInt
            | TypeSpec::Long
            | TypeSpec::UnsignedLong
            | TypeSpec::LongLong
            | TypeSpec::UnsignedLongLong
            | TypeSpec::Ptr(_)
    )
}

/// **De que locales se toma la direccion en esta funcion.** En la duda, de
/// todas.
///
/// Ver la cabecera: `todas` es el comodin, y contesta que si a proposito.
pub(in crate::codegen) fn direcciones_tomadas(cuerpo: &[Stmt]) -> Tomadas {
    let mut t = Tomadas::default();
    for s in cuerpo {
        stmt_toma(s, &mut t);
    }
    t
}

/// Las locales cuya direccion se toma. `todas` = el escaner encontro una forma
/// que no sabe desmontar, y entonces ninguna local va a la matriz.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(in crate::codegen) struct Tomadas {
    nombres: std::collections::BTreeSet<String>,
    todas: bool,
}

impl Tomadas {
    pub(in crate::codegen) fn incluye(&self, nombre: &str) -> bool {
        self.todas || self.nombres.contains(nombre)
    }
}

fn stmt_toma(s: &Stmt, t: &mut Tomadas) {
    match s {
        Stmt::Break | Stmt::Continue | Stmt::Goto(_) | Stmt::Label(_) => {}
        Stmt::Printf(_) | Stmt::PrintfLn(_) => {}
        Stmt::Return(None) => {}
        Stmt::Return(Some(e)) | Stmt::Expr(e) => expr_toma(e, t),
        Stmt::Block(v) => v.iter().for_each(|x| stmt_toma(x, t)),
        Stmt::If(c, a, b) => {
            expr_toma(c, t);
            stmt_toma(a, t);
            if let Some(x) = b {
                stmt_toma(x, t);
            }
        }
        Stmt::While(c, a) => {
            expr_toma(c, t);
            stmt_toma(a, t);
        }
        Stmt::DoWhile(a, c) => {
            stmt_toma(a, t);
            expr_toma(c, t);
        }
        Stmt::For(i, c, p, a) => {
            for o in [i, c, p].into_iter().flatten() {
                expr_toma(o, t);
            }
            stmt_toma(a, t);
        }
        Stmt::DeclAssign(_, _, init) => {
            if let Some(e) = init {
                expr_toma(e, t);
            }
        }
        // ** `Switch` y `DeclInit` NO estan, y es la decision de este fichero:
        // el primero lleva `Case` y el segundo `Escritura`, dos formas mas que
        // desmontar. Una funcion con un `switch` se queda entera en la pila --
        // correcta y lenta-- hasta que alguien las anada aqui. Queda dicho, que
        // es lo que separa un limite de un olvido.
        _ => t.todas = true,
    }
}

/// **La raiz de un `&`**: de que local es la direccion que se toma.
///
/// ```text
///    &x          x
///    &v.campo    v          (la direccion esta DENTRO de v)
///    &t[i]       t          (y de i no: i solo se lee)
///    &*p, &p->c  de NINGUNA local: es a traves de un puntero, y el puntero
///    &p[i]       solo se lee. Se baja a lo de dentro por si lleva otro `&`
///    &(cast)x    lo que sea x
///    otra cosa   la duda: TODAS
/// ```
fn raiz_de(e: &Expr, t: &mut Tomadas) {
    match e {
        Expr::Var(n) => {
            t.nombres.insert(n.clone());
        }
        Expr::Subscript(n, i) => {
            t.nombres.insert(n.clone());
            expr_toma(i, t);
        }
        Expr::Field(base, _) => raiz_de(base, t),
        Expr::Cast(_, x) => raiz_de(x, t),
        Expr::Deref(x) | Expr::Arrow(x, _) => expr_toma(x, t),
        Expr::IndexPtr(x, i) => {
            expr_toma(x, t);
            expr_toma(i, t);
        }
        otro => {
            t.todas = true;
            expr_toma(otro, t);
        }
    }
}

fn expr_toma(e: &Expr, t: &mut Tomadas) {
    match e {
        // *** El que se busca.
        Expr::AddrOf(x) => raiz_de(x, t),

        // Hojas: no hay nada dentro.
        Expr::Int(_)
        | Expr::FloatLit(_)
        | Expr::StringLit(_)
        | Expr::CharLit(_)
        | Expr::Var(_)
        | Expr::PreInc(_)
        | Expr::PreDec(_)
        | Expr::PostInc(_)
        | Expr::PostDec(_) => {}

        // Un operando.
        Expr::Neg(a) | Expr::Not(a) | Expr::BitNot(a) | Expr::Deref(a) => expr_toma(a, t),

        // Dos operandos.
        Expr::Add(a, b)
        | Expr::Sub(a, b)
        | Expr::Mul(a, b)
        | Expr::Div(a, b)
        | Expr::Mod(a, b)
        | Expr::Eq(a, b)
        | Expr::Neq(a, b)
        | Expr::Lt(a, b)
        | Expr::Gt(a, b)
        | Expr::Le(a, b)
        | Expr::Ge(a, b)
        | Expr::BitAnd(a, b)
        | Expr::BitXor(a, b)
        | Expr::BitOr(a, b)
        | Expr::LAnd(a, b)
        | Expr::LOr(a, b)
        | Expr::Shl(a, b)
        | Expr::Shr(a, b) => {
            expr_toma(a, t);
            expr_toma(b, t);
        }

        Expr::Comma(v) => v.iter().for_each(|x| expr_toma(x, t)),

        Expr::Assign(_, a) | Expr::Cast(_, a) | Expr::Subscript(_, a) => expr_toma(a, t),
        Expr::Field(a, _) | Expr::Arrow(a, _) => expr_toma(a, t),
        Expr::AssignDeref(a, b) | Expr::IndexPtr(a, b) => {
            expr_toma(a, t);
            expr_toma(b, t);
        }
        Expr::AssignSubscript(_, a, b) => {
            expr_toma(a, t);
            expr_toma(b, t);
        }
        Expr::AssignField(a, _, b) | Expr::AssignArrow(a, _, b) | Expr::AssignOp(a, _, b) => {
            expr_toma(a, t);
            expr_toma(b, t);
        }
        Expr::AssignIndexPtr(a, b, c) | Expr::Conditional(a, b, c) => {
            expr_toma(a, t);
            expr_toma(b, t);
            expr_toma(c, t);
        }
        // Una llamada puede pasar `&x` como argumento, y por eso hay que bajar
        // a los argumentos: el `&` esta ahi y se ve.
        Expr::Call(_, v) | Expr::Intrinsic(_, v) | Expr::Syscall(_, v) => {
            v.iter().for_each(|x| expr_toma(x, t))
        }
        Expr::CallPtr(f, v) => {
            expr_toma(f, t);
            v.iter().for_each(|x| expr_toma(x, t));
        }

        // ** SIN comodin desde el 2026-09-18. Aqui habia un `_ => true` --"una
        // forma nueva contesta que SI: correcta y lenta"--, y ya no cubria
        // ninguna: las cincuenta estan arriba. Desde que el arbol de C vive en
        // otro crate (`bmo-c-front`), lo mas seguro no es un SI por defecto: es
        // que una forma NUEVA haga que el emisor NO COMPILE hasta que alguien
        // decida, aqui, si toma la direccion de algo.
    }
}


/// **Cuantas veces aparece cada nombre.** Solo para ordenar el reparto.
///
/// [!] Y este SI puede quedarse corto sin peligro, que es la diferencia con el
/// escaner de arriba: contar de menos cambia QUE variable se lleva un hueco, no
/// si era seguro darselo. Por eso aqui el comodin no cuenta nada en vez de
/// gritar.
pub(in crate::codegen) fn contar(cuerpo: &[Stmt], nombre: &str) -> usize {
    cuerpo.iter().map(|s| cuenta_stmt(s, nombre)).sum()
}

fn cuenta_stmt(s: &Stmt, n: &str) -> usize {
    match s {
        Stmt::Return(Some(e)) | Stmt::Expr(e) => cuenta_expr(e, n),
        Stmt::Block(v) => v.iter().map(|x| cuenta_stmt(x, n)).sum(),
        Stmt::If(c, a, b) => {
            cuenta_expr(c, n)
                + cuenta_stmt(a, n)
                + b.as_ref().map_or(0, |x| cuenta_stmt(x, n))
        }
        // ** El cuerpo de un bucle cuenta DOBLE, y es la unica pizca de
        // inteligencia que hay aqui: un uso dentro de un bucle vale mas que uno
        // fuera. No es un perfil --nadie ha contado vueltas-- pero acierta el
        // caso que importa sin medir nada.
        Stmt::While(c, a) => cuenta_expr(c, n) + 2 * cuenta_stmt(a, n),
        Stmt::DoWhile(a, c) => 2 * cuenta_stmt(a, n) + cuenta_expr(c, n),
        Stmt::For(i, c, p, a) => {
            [i, c, p].iter().map(|o| o.as_ref().map_or(0, |e| cuenta_expr(e, n))).sum::<usize>()
                + 2 * cuenta_stmt(a, n)
        }
        Stmt::DeclAssign(_, d, init) => {
            (d == n) as usize + init.as_ref().map_or(0, |e| cuenta_expr(e, n))
        }
        _ => 0,
    }
}

fn cuenta_expr(e: &Expr, n: &str) -> usize {
    match e {
        Expr::Var(v) | Expr::PreInc(v) | Expr::PreDec(v) | Expr::PostInc(v)
        | Expr::PostDec(v) => (v == n) as usize,
        Expr::Assign(v, x) => (v == n) as usize + cuenta_expr(x, n),
        Expr::Neg(a) | Expr::Not(a) | Expr::BitNot(a) | Expr::Deref(a) => cuenta_expr(a, n),
        Expr::Add(a, b)
        | Expr::Sub(a, b)
        | Expr::Mul(a, b)
        | Expr::Div(a, b)
        | Expr::Mod(a, b)
        | Expr::Eq(a, b)
        | Expr::Neq(a, b)
        | Expr::Lt(a, b)
        | Expr::Gt(a, b)
        | Expr::Le(a, b)
        | Expr::Ge(a, b)
        | Expr::BitAnd(a, b)
        | Expr::BitXor(a, b)
        | Expr::BitOr(a, b)
        | Expr::LAnd(a, b)
        | Expr::LOr(a, b)
        | Expr::Shl(a, b)
        | Expr::Shr(a, b) => cuenta_expr(a, n) + cuenta_expr(b, n),
        Expr::Comma(v) => v.iter().map(|x| cuenta_expr(x, n)).sum(),
        Expr::Cast(_, a) | Expr::Subscript(_, a) | Expr::Field(a, _) | Expr::Arrow(a, _) => {
            cuenta_expr(a, n)
        }
        Expr::AssignDeref(a, b) | Expr::IndexPtr(a, b) => cuenta_expr(a, n) + cuenta_expr(b, n),
        Expr::AssignSubscript(v, a, b) => {
            (v == n) as usize + cuenta_expr(a, n) + cuenta_expr(b, n)
        }
        Expr::AssignField(a, _, b) | Expr::AssignArrow(a, _, b) | Expr::AssignOp(a, _, b) => {
            cuenta_expr(a, n) + cuenta_expr(b, n)
        }
        Expr::AssignIndexPtr(a, b, c) | Expr::Conditional(a, b, c) => {
            cuenta_expr(a, n) + cuenta_expr(b, n) + cuenta_expr(c, n)
        }
        Expr::Call(_, v) | Expr::Intrinsic(_, v) | Expr::Syscall(_, v) => {
            v.iter().map(|x| cuenta_expr(x, n)).sum()
        }
        Expr::CallPtr(f, v) => {
            cuenta_expr(f, n) + v.iter().map(|x| cuenta_expr(x, n)).sum::<usize>()
        }
        _ => 0,
    }
}


/// **Cuantos usos (ponderados por bucle) paga un hueco.** Un registro de la
/// matriz cuesta un `push` y un `pop` por llamada, y solo devuelve algo si se
/// opera en el mas veces de las que cuesta. Lo puso el metro el 18-09: al
/// pasar el troquel a POR VARIABLE, dos funciones que corren UNA vez ganaron
/// matriz para variables de dos usos y subieron ocho instrucciones cada una.
/// Seis se ELIGIO midiendo (3, 4, 6 y 8 contra el metro: 324.056, 323.664,
/// 322.960 y 325.504 instrucciones); no es un numero redondo, es el que gano.
pub(in crate::codegen) const UMBRAL_DE_USOS: usize = 6;

/// **El reparto**: que nombres se llevan hueco, y cual.
///
/// `candidatos` llega ya filtrado por tipo y por "no es parametro"; aqui solo
/// se ordena y se corta. El orden es por cuantas veces aparece el nombre en el
/// cuerpo -- una aproximacion, y esta dicho en la cabecera que lo es.
pub(in crate::codegen) fn repartir(mut candidatos: Vec<(String, usize)>) -> Vec<(String, u8)> {
    // Por usos descendente, y a igualdad por nombre: sin el segundo criterio el
    // reparto dependeria del orden del `HashMap`, y **dos compilaciones del
    // mismo fichero darian dos binarios distintos**. Un compilador que no es
    // reproducible no se puede auditar.
    candidatos.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    candidatos
        .into_iter()
        .filter(|(_, usos)| *usos >= UMBRAL_DE_USOS)
        .take(MATRIZ.len())
        .enumerate()
        .map(|(i, (n, _))| (n, MATRIZ[i]))
        .collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// El cuerpo de `main` de un fuente, parseado.
    fn cuerpo(src: &str) -> Vec<Stmt> {
        let p = crate::parse(&format!("int f(int *q); struct s {{ int c; }}; int main() {{ {src} return 0; }}")).expect("parsea");
        p.functions.into_iter().find(|f| f.name == "main").unwrap().body
    }

    fn tomadas(src: &str) -> Vec<String> {
        let t = direcciones_tomadas(&cuerpo(src));
        assert!(!t.todas, "no deberia ser TODAS: {src}");
        t.nombres.into_iter().collect()
    }

    #[test]
    fn el_ampersand_nombra_a_su_variable_y_a_nadie_mas() {
        assert_eq!(tomadas("int x; int y; int *p; p = &x; y = 1;"), ["x"]);
        assert_eq!(tomadas("int x; int y; f(&x); y = 2;"), ["x"]);
        // &t[i]: es t, e i solo se lee
        assert_eq!(tomadas("int t[4]; int i; int *p; i = 0; p = &t[i];"), ["t"]);
        // &v.c: es v
        assert_eq!(tomadas("struct s v; int *p; p = &v.c;"), ["v"]);
    }

    #[test]
    fn a_traves_de_un_puntero_no_se_toma_ninguna_local() {
        assert_eq!(tomadas("int x; int *q; int *p; q = &x; p = &*q;"), ["x"]);
        assert_eq!(tomadas("struct s *q; int *p; p = &q->c;"), Vec::<String>::new());
        assert_eq!(tomadas("int *q; int i; int *p; i = 1; p = &q[i];"), ["q"]);
    }

    #[test]
    fn un_switch_deja_la_funcion_entera_fuera() {
        let t = direcciones_tomadas(&cuerpo("int x; x = 1; switch (x) { case 1: x = 2; break; }"));
        assert!(t.todas);
        assert!(t.incluye("cualquiera"));
    }

    #[test]
    fn el_reparto_exige_el_umbral_y_corta_en_cuatro() {
        let c = |n: &str, u: usize| (n.to_string(), u);
        let r = repartir(vec![c("a", 9), c("b", 2), c("c", 6), c("d", 7), c("e", 8), c("f", 6)]);
        assert_eq!(r, vec![("a".into(), 12), ("e".into(), 13), ("d".into(), 14), ("c".into(), 15)]);
        assert!(repartir(vec![c("b", UMBRAL_DE_USOS - 1)]).is_empty());
    }
}
