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
//! El escaner de abajo contesta *"esta funcion tiene un `&`?"*, y **su brazo
//! comodin dice que SI**. O sea que una forma del arbol que este fichero no
//! conozca --hoy o el dia que nazca una nueva-- hace que la funcion entera se
//! quede en la pila: **correcta y lenta**, que es el lado barato del error.
//!
//! *** Es la misma decision que `PTE_NUESTRA` en el kernel: *"la duda se
//! resuelve por el lado que solo cuesta RAM"*. Aqui solo cuesta velocidad.
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

/// **Se toma alguna direccion en esta funcion?** En la duda, SI.
///
/// Ver la cabecera: el comodin contesta `true` a proposito.
pub(in crate::codegen) fn hay_direcciones(cuerpo: &[Stmt]) -> bool {
    cuerpo.iter().any(stmt_toma)
}

fn stmt_toma(s: &Stmt) -> bool {
    match s {
        Stmt::Break | Stmt::Continue | Stmt::Goto(_) | Stmt::Label(_) => false,
        Stmt::Printf(_) | Stmt::PrintfLn(_) => false,
        Stmt::Return(None) => false,
        Stmt::Return(Some(e)) | Stmt::Expr(e) => expr_toma(e),
        Stmt::Block(v) => v.iter().any(stmt_toma),
        Stmt::If(c, a, b) => {
            expr_toma(c) || stmt_toma(a) || b.as_ref().is_some_and(|x| stmt_toma(x))
        }
        Stmt::While(c, a) => expr_toma(c) || stmt_toma(a),
        Stmt::DoWhile(a, c) => stmt_toma(a) || expr_toma(c),
        Stmt::For(i, c, p, a) => {
            [i, c, p].iter().any(|o| o.as_ref().is_some_and(expr_toma)) || stmt_toma(a)
        }
        Stmt::DeclAssign(_, _, init) => init.as_ref().is_some_and(expr_toma),
        // ** `Switch` y `DeclInit` NO estan, y es la decision de este fichero:
        // el primero lleva `Case` y el segundo `Escritura`, dos formas mas que
        // desmontar. Una funcion con un `switch` se queda entera en la pila --
        // correcta y lenta-- hasta que alguien las anada aqui. Queda dicho, que
        // es lo que separa un limite de un olvido.
        _ => true,
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

fn expr_toma(e: &Expr) -> bool {
    match e {
        // *** El que se busca.
        Expr::AddrOf(_) => true,

        // Hojas: no hay nada dentro.
        Expr::Int(_)
        | Expr::FloatLit(_)
        | Expr::StringLit(_)
        | Expr::CharLit(_)
        | Expr::Var(_)
        | Expr::PreInc(_)
        | Expr::PreDec(_)
        | Expr::PostInc(_)
        | Expr::PostDec(_) => false,

        // Un operando.
        Expr::Neg(a) | Expr::Not(a) | Expr::BitNot(a) | Expr::Deref(a) => expr_toma(a),

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
        | Expr::Shr(a, b) => expr_toma(a) || expr_toma(b),

        Expr::Comma(v) => v.iter().any(expr_toma),

        // ** Escribir en un sitio NO es tomar su direccion. `*p = x`, `a[i] = x`
        // y `s.c = x` escriben a traves de algo que ya existe -- y si ese algo
        // vino de un `&`, el `&` esta ahi dentro y se ve al bajar.
        Expr::Assign(_, a) | Expr::Cast(_, a) | Expr::Subscript(_, a) => expr_toma(a),
        Expr::Field(a, _) | Expr::Arrow(a, _) => expr_toma(a),
        Expr::AssignDeref(a, b) | Expr::IndexPtr(a, b) => expr_toma(a) || expr_toma(b),
        Expr::AssignSubscript(_, a, b) => expr_toma(a) || expr_toma(b),
        Expr::AssignField(a, _, b) | Expr::AssignArrow(a, _, b) | Expr::AssignOp(a, _, b) => {
            expr_toma(a) || expr_toma(b)
        }
        Expr::AssignIndexPtr(a, b, c) | Expr::Conditional(a, b, c) => {
            expr_toma(a) || expr_toma(b) || expr_toma(c)
        }
        // Una llamada puede pasar `&x` como argumento, y por eso hay que bajar
        // a los argumentos: el `&` esta ahi y se ve.
        Expr::Call(_, v) | Expr::Intrinsic(_, v) | Expr::Syscall(_, v) => v.iter().any(expr_toma),
        Expr::CallPtr(f, v) => expr_toma(f) || v.iter().any(expr_toma),

        // ** Y aqui sigue el comodin que hace segura la funcion entera. Hoy no
        // cubre ninguna forma que exista --las cincuenta estan arriba-- pero el
        // dia que nazca una nueva, contestar que SI deja la funcion en la pila:
        // correcta y lenta, que es el lado barato del error.
        _ => true,
    }
}

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
        .take(MATRIZ.len())
        .enumerate()
        .map(|(i, (n, _))| (n, MATRIZ[i]))
        .collect()
}
