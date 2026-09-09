//! **CARRIL ROJO** -- EL PLEGADO. Lo que se puede saber al compilar.
//!
//! [fase]     EMISION
//!
//! [aparece]  BANCO -- *** y esta es la prueba de que el eje sirve: plegar de
//!            mas puso 5 de 500 filas rojas EN EL ACTO. Una decision pura la
//!            caza el banco; la misma decision dentro del emisor habria salido
//!            en DOOM
//!
//!
//! [carril]  ROJO      es la unica parte de `decidir/` cuyo resultado acaba
//!                     DENTRO de un numero que el programa usa. Lo demas dice
//!                     donde van las cosas; esto dice CUANTO valen.
//!
//! [cuesta]  DATO -- un numero mal plegado no da error: da otro programa. Aqui
//!           se resuelven `1 << 16` --el `FRACUNIT` de toda la aritmetica de
//!           DOOM-- y las flechas del mapa, que se escriben en flotante y se
//!           guardan en punto fijo. Una tabla plegada mal es una tabla
//!           equivocada en cada uso, para siempre y en silencio.
//!
//! [riesgo]  SILENCIO ESPEJO
//!           SILENCIO -- si esto devuelve `None` de mas, el programa sale LENTO
//!                    y correcto: no se entera nadie. Es exactamente asi como un
//!                    `imul` para multiplicar 1 por 8 sobrevivio meses dentro
//!                    del bucle mas caliente de DOOM.
//!           ESPEJO -- estas mismas cuentas las hace el CPU en ejecucion, por la
//!                    ruta que no se pliega. Las dos tienen que dar lo mismo, y
//!                    ese es el unico juez que hay.
//!
//! # *** LA REGLA QUE SEPARA LAS DOS PUERTAS DE ESTE FICHERO
//!
//! `constante_de` se escribio para los INICIALIZADORES, donde el tipo del
//! destino ya decide como se guarda el numero. El emisor de expresiones no tiene
//! esa red, y por eso entra por otra puerta: `constante_para_emitir`.
//!
//! ** La diferencia no es de gusto: **hay operadores cuyo resultado depende del
//! SIGNO**, y este plegador calcula siempre en `i64` con signo.
//!
//! ```text
//!    + - * << & | ^ ~     dan lo mismo con signo y sin el (complemento a dos)
//!    / % >>               NO lo dan. Sin signo, menos uno partido por dos son
//!                         dos mil millones; con signo son cero. Y el
//!                         desplazamiento a la derecha arrastra el bit alto o
//!                         no segun el signo
//! ```
//!
//! *** Asi que la puerta del emisor **se niega a plegar los tres de la derecha**
//! -- y decir que no es lo que la hace utilizable. Un plegador que contesta a
//! todo es un plegador en el que no se puede confiar para nada. Es **L4** de la
//! casa aplicada a una funcion: una regla se prueba diciendo que NO.
//!
//! [!] `constante_de` los sigue plegando, porque su llamante --el
//! inicializador-- conoce el tipo del destino. Cambiarlo romperia las tablas de
//! DOOM para arreglar un caso que en un inicializador no se da. Queda dicho.

use crate::ast::{Expr, TypeSpec};

pub(in crate::codegen) fn constante_de(e: &Expr) -> Option<i64> {
    match e {
        Expr::Int(n) => Some(*n),
        // `int x = -5` es `Neg(Int(5))` en el AST, no `Int(-5)`.
        Expr::Neg(interior) => constante_de(interior).map(|v| -v),
        Expr::Add(a, b) => {
            Some(constante_de(a)?.wrapping_add(constante_de(b)?))
        }
        Expr::Sub(a, b) => {
            Some(constante_de(a)?.wrapping_sub(constante_de(b)?))
        }
        Expr::Mul(a, b) => {
            Some(constante_de(a)?.wrapping_mul(constante_de(b)?))
        }
        Expr::Div(a, b) => constante_de(a)?.checked_div(constante_de(b)?),
        // * Lo que faltaba, y cada linea es una tabla de DOOM.
        //
        //   'M'        `midiheader[] = {'M','T','h','d', ...}`  (mus2mid.c)
        //   1 << 16    `FRACUNIT`, o sea la unidad de TODA la aritmetica
        //              del juego: `xspeed[] = {FRACUNIT, 47000, ...}`
        //   ~ | & ^    mascaras de banderas en las tablas de estados
        //
        // El evaluador se habia quedado en las cuatro operaciones de la
        // aritmetica, y una tabla que no se puede plegar no da un valor
        // malo: **da un error y el fichero entero no compila**.
        Expr::CharLit(c) => Some(*c as i64),
        Expr::Mod(a, b) => constante_de(a)?.checked_rem(constante_de(b)?),
        Expr::Shl(a, b) => {
            Some(constante_de(a)?.wrapping_shl(constante_de(b)? as u32))
        }
        Expr::Shr(a, b) => {
            Some(constante_de(a)?.wrapping_shr(constante_de(b)? as u32))
        }
        Expr::BitAnd(a, b) => Some(constante_de(a)? & constante_de(b)?),
        Expr::BitOr(a, b) => Some(constante_de(a)? | constante_de(b)?),
        Expr::BitXor(a, b) => Some(constante_de(a)? ^ constante_de(b)?),
        Expr::BitNot(a) => Some(!constante_de(a)?),
        Expr::Not(a) => Some((constante_de(a)? == 0) as i64),
        // Un cast no cambia el VALOR de una constante entera, solo su
        // anchura -- y la anchura la pone el subobjeto al escribirlo.
        //
        // * Y si dentro hay COMA FLOTANTE, se pliega y se trunca.
        //
        // `(fixed_t)(-.867*FRACUNIT)` -- asi escribe `am_map.c` las flechas
        // del mapa, y es la forma normal de meter un numero real en punto
        // fijo: se calcula en flotante **al compilar** y lo que se guarda
        // es un entero. El programa no lleva un solo `float` dentro.
        //
        // Truncar hacia cero es lo que dice C de una conversion de
        // flotante a entero, y por eso se hace con `as i64` y no
        // redondeando: redondear daria otro numero, y el numero es el dato.
        Expr::Cast(t, a) => constante_de(a).or_else(|| {
            if matches!(t, TypeSpec::Float | TypeSpec::Double) {
                return None;
            }
            constante_flotante(a).map(|f| f as i64)
        }),
        _ => None,
    }
}

/// Pliega una expresion constante que tiene coma flotante dentro.
///
/// Solo se usa cuando el resultado va a un ENTERO: mientras BMO C no tenga
/// la ruta SSE en los datos, un global que se quede en flotante sigue
/// diciendo que no puede. Aqui el flotante es una forma de ESCRIBIR el
/// numero, no de guardarlo.
pub(in crate::codegen) fn constante_flotante(e: &Expr) -> Option<f64> {
    // Lo que ya se pliega como entero, se pliega como entero: asi el
    // desplazamiento, las mascaras y el resto de operaciones que solo
    // existen sobre enteros no hay que escribirlas dos veces. `FRACUNIT`
    // es `(1<<16)`, y sin esta linea el `-.867*FRACUNIT` de DOOM no
    // llegaba a plegarse por culpa del desplazamiento.
    if let Some(n) = constante_de(e) {
        return Some(n as f64);
    }
    Some(match e {
        Expr::FloatLit(f) => *f,
        Expr::Int(n) => *n as f64,
        Expr::CharLit(c) => *c as f64,
        Expr::Neg(a) => -constante_flotante(a)?,
        Expr::Add(a, b) => constante_flotante(a)? + constante_flotante(b)?,
        Expr::Sub(a, b) => constante_flotante(a)? - constante_flotante(b)?,
        Expr::Mul(a, b) => constante_flotante(a)? * constante_flotante(b)?,
        Expr::Div(a, b) => {
            let d = constante_flotante(b)?;
            if d == 0.0 {
                return None;
            }
            constante_flotante(a)? / d
        }
        Expr::Cast(_, a) => constante_flotante(a)?,
        _ => return None,
    })
}

/// **La puerta del EMISOR de expresiones.** Ver la regla en la cabecera.
///
/// Pliega solo lo que da igual con signo que sin el. Todo lo demas devuelve
/// `None`, y entonces el emisor hace lo de siempre: emitir el calculo.
///
/// ** Esto es lo que mata el `imul` de `d8 = d8 + 1`: la suma de punteros
/// construye un producto de dos constantes en el AST --el uno y el tamano del
/// elemento-- y hasta hoy lo multiplicaba el CPU en cada vuelta.
pub(in crate::codegen) fn constante_para_emitir(e: &Expr) -> Option<i64> {
    // *** LA RAIZ TIENE QUE SER `+`, `-` O `*`, Y ESO LO DIJO EL BANCO.
    //
    // La primera version plegaba cualquier expresion constante, y **cinco de
    // las 500 filas se pusieron rojas en el acto** -- entre ellas la del propio
    // escalado de DOOM y el censo de signo entero.
    //
    // El motivo no era el plegado: era el RECORTE. El emisor aplica
    // `recortar_a_32` en unas ramas y no en otras, y la de un literal suelto es
    // de las que no:
    //
    // ```text
    //    Expr::Int(0x80000000)   la rama larga NO recorta -> 2.147.483.648
    //    plegado + recorte       `movsxd` extiende el signo -> ...FF80000000
    //                            y `span >= 0x80000000` pasa a ser falso
    // ```
    //
    // ** Add, Sub y Mul son las tres ramas que SI llaman a `recortar_a_32` justo
    // despues de `emit_binop`. Plegar solo esas hace que el camino corto y el
    // largo sean el mismo por construccion, y no por revision.
    //
    // *** Y son exactamente las que hacian falta: la suma de punteros construye
    // un `Mul` de dos constantes, que es el `imul` que se venia a matar.
    //
    // [!] Las demas --`&`, `|`, `^`, `<<`, la negacion-- se pliegan el dia que
    // se compruebe UNA POR UNA que su recorte coincide. Ampliar esta lista sin
    // eso es como se rompen las tablas de DOOM.
    if !matches!(e, Expr::Add(..) | Expr::Sub(..) | Expr::Mul(..)) {
        return None;
    }
    if !seguro_sin_signo(e) {
        return None;
    }
    constante_de(e)
}

/// El resultado de esta expresion es el mismo con signo y sin el?
///
/// Se recorre el arbol ENTERO y no solo la raiz: en `(a / b) + 1` la suma esta
/// arriba y la division dentro, y la que decide es la division.
fn seguro_sin_signo(e: &Expr) -> bool {
    match e {
        Expr::Int(_) | Expr::CharLit(_) => true,
        Expr::Neg(a) | Expr::BitNot(a) | Expr::Not(a) => seguro_sin_signo(a),
        // Un cast no cambia el VALOR de un entero, solo su anchura -- y la
        // anchura la aplica `recortar_a_32` despues de esto, igual que en la
        // ruta que no se pliega.
        Expr::Cast(_, a) => seguro_sin_signo(a),
        Expr::Add(a, b)
        | Expr::Sub(a, b)
        | Expr::Mul(a, b)
        | Expr::Shl(a, b)
        | Expr::BitAnd(a, b)
        | Expr::BitOr(a, b)
        | Expr::BitXor(a, b) => seguro_sin_signo(a) && seguro_sin_signo(b),
        // ** `Div`, `Mod` y `Shr` NO estan, y esa ausencia ES la regla.
        _ => false,
    }
}

/// **Emitir esta expresion toca SOLO `rax`?**
///
/// === *** LA PREGUNTA QUE QUITA LA PILA (2026-09-09) ====================
///
/// `emit_binop` guarda el operando izquierdo en la PILA mientras calcula el
/// derecho, y tiene que hacerlo: en una maquina de pila, calcular el derecho
/// puede pisar cualquier registro. Ese `push`/`pop` es lo que hace que un
/// `d8 = d8 + 1` pase por memoria dos veces para sumar ocho.
///
/// ** Pero si el derecho es una CONSTANTE, emitirlo es un `mov rax, imm` y no
/// pisa nada mas. Entonces el izquierdo cabe en `rdx` y la pila sobra:
///
/// ```text
///    con pila    emitir a ; push rax ; emitir b ; pop rdx ; <op>
///    sin ella    emitir a ; mov rdx,rax ; emitir b ; <op>
/// ```
///
/// Dos accesos a memoria menos **en cada operacion binaria con una constante a
/// la derecha**, que en C es casi todas: los indices, los tamanos, las mascaras
/// y los pasos de puntero.
///
/// [!] Y lo que se contesta aqui es exactamente *"que emite `emit_expr`"*, asi
/// que las dos ramas tienen que ir juntas: un literal, o algo que
/// `constante_para_emitir` ya pliega. Si `emit_expr` aprendiera a emitir un
/// literal de otra forma que tocara mas registros, **esta funcion mentiria** --
/// por eso vive al lado de la que decide el plegado y no en el emisor.
pub(in crate::codegen) fn solo_toca_rax(e: &Expr) -> bool {
    matches!(e, Expr::Int(_) | Expr::CharLit(_)) || constante_para_emitir(e).is_some()
}
