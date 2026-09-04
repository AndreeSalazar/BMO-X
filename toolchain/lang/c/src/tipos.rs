//! **EL JUEZ UNICO DE "QUE TIPO ES ESTA EXPRESION".**
//!
//! [carril]  ROJO      de aqui sale el offset que se graba en el arbol
//!
//! [cuesta]  DATO -- lo que este fichero conteste lo escriben DOS consumidores
//!           en sitios distintos: el parser lo mete dentro del nodo (`Arrow`
//!           lleva su `u32`) y el codegen lo usa para escalar. Equivocarse no
//!           da un mensaje: da un programa compilado que guarda en el campo de
//!           al lado, o con el tamano de al lado.
//!
//! [riesgo]  ESPEJO SILENCIO
//!           ESPEJO   -- este fichero NACE de que dos funciones juzgaban esta
//!                       misma pregunta con dos respuestas. La forma vuelve en
//!                       cuanto alguien conteste "que tipo es esto" por su
//!                       cuenta; la pregunta de guardia es la de L6f: **quien
//!                       mas juzga este mismo numero, y con que**.
//!           SILENCIO -- contesta `None` cuando no sabe, y los dos llamantes
//!                       convierten ese `None` en un numero: `unwrap_or(0)` el
//!                       offset, "sera un entero" el paso. Un hueco aqui no da
//!                       un error.
//!
//! # *** POR QUE ESTE FICHERO EXISTE
//!
//! Porque la misma pregunta se contestaba en DOS sitios y no sabian lo mismo:
//!
//! ```text
//!    parser/types.rs      resolve_expr_type()   <- decidia el OFFSET grabado
//!    codegen/indexing.rs  pointee_type()        <- decidia el PASO al emitir
//! ```
//!
//! | forma de C            | el parser | el codegen |
//! |-----------------------|-----------|------------|
//! | array que decae       | NO        | SI         |
//! | `p - 1` (aritmetica)  | NO        | SI         |
//! | `&x`                  | SI        | NO         |
//! | `p++` sigue puntero   | NO        | SI         |
//!
//! ** Y **el mas debil era el que grababa el numero en el arbol.** El codegen
//! sabia mas y llegaba tarde: cuando le llega el `Expr::Arrow`, el `u32` ya
//! esta dentro y nadie lo revisa. Es el patron 55 --dos jueces de la misma
//! magnitud, manda el flojo-- con el agravante de que aqui el flojo escribe.
//!
//! Este arbol ya pago esa forma una vez, y esta escrito en `parser/types.rs`
//! (2026-08-13): *"dos calculos de offset que no coincidian [...] y el
//! desacuerdo estaba a dos ficheros de distancia"*. Se arreglo aquel caso y se
//! dejo la estructura que los fabrica. Los dos fallos de hoy salen de ahi.
//!
//! # La forma, tomada de un compilador de C tipico y NO heredada
//!
//! En GCC el parser no calcula un offset: construye un nodo que NOMBRA el
//! campo, y la disposicion la pone despues una sola maquina desde una sola
//! tabla. Se toma esa FORMA --un juez, consultado por todos-- y no su
//! maquinaria: tres representaciones intermedias y doscientas pasadas son el
//! problema de otro, y ademas GPL.
//!
//! # [!] LO QUE ESTE JUEZ SE NIEGA A CONTESTAR, Y NO ES UN HUECO
//!
//! `entero + entero` devuelve `None` A PROPOSITO. El aviso lo dejo escrito
//! quien lo excluyo la primera vez, y sigue siendo cierto:
//!
//! > *"el tipo de `a + b` pide las conversiones usuales de C, y equivocarse
//! > aqui no da un error, da un `memset` de la medida equivocada"*
//!
//! ** Pero ese motivo **no cubria el caso que rompio**: en `puntero +/- entero`
//! C no tiene ninguna conversion que hacer --el resultado ES del tipo del
//! puntero-- porque las conversiones usuales solo hablan de dos operandos
//! ARITMETICOS. O sea que la prudencia se aplico al unico caso que no la
//! necesitaba, y es justo el que recorre cualquier tabla.

use crate::ast::{Expr, TypeSpec};

/// Lo unico que el juez necesita del que pregunta: el tipo de un NOMBRE.
///
/// * Es una sola funcion a proposito. El parser la contesta con `var_types` y
/// el codegen con `var_offsets` + `global_offsets`; todo lo demas --campos,
/// elementos, literales-- sale del propio arbol y por eso los dos obtienen la
/// MISMA respuesta sin compartir tablas.
pub(crate) trait Ambito {
    fn tipo_de_variable(&self, nombre: &str) -> Option<TypeSpec>;
    /// El tipo del campo `campo` dentro del agregado `agregado`.
    ///
    /// ** Segunda y ultima pregunta del contrato, y entro el 2026-09-02 al
    /// vaciar `Expr::Field` y `Expr::Arrow`. Antes el tipo del campo viajaba
    /// DENTRO del nodo, asi que el juez no tenia que preguntarlo -- y eso
    /// obligaba al parser a resolverlo en el sitio y el momento en que menos
    /// sabe. Ahora lo contesta cada consumidor con SU tabla, y las dos tablas
    /// las coteja `codegen::cotejar_disposicion`.
    fn tipo_de_campo(&self, agregado: &str, campo: &str) -> Option<TypeSpec>;
    /// Lo que DEVUELVE una funcion.
    ///
    /// *** Tercera y ultima pregunta, y entro el 2026-09-02 porque DOOM no
    /// compilaba: `getSide(secnum,i,0)->sector` es una LLAMADA seguida de
    /// flecha, y sin este brazo el agregado no se resolvia.
    ///
    /// [!] Ojo a lo que eso significa hacia atras: **antes esto no daba error,
    /// daba offset 0**. O sea que esa linea de `p_floor.c` llevaba leyendo el
    /// primer campo de `side_t` donde pedia `sector`. Lo destapo el guardian
    /// nuevo, no una corrida -- que es exactamente para lo que se puso.
    fn tipo_de_retorno(&self, funcion: &str) -> Option<TypeSpec>;
}

/// Un array **decae a puntero** en cuanto se usa en una expresion.
///
/// [!] Importa que esto pase aqui y no en el llamante: `arr + 1` es un
/// `struct vs *`, no un `struct vs[16]`, y quien pregunte por su campo tiene
/// que ver un puntero o volvera a caer en el `None`.
fn decaido(t: TypeSpec) -> TypeSpec {
    match t {
        TypeSpec::Array(base, _) => TypeSpec::Ptr(base),
        otro => otro,
    }
}

fn es_direccion(t: &Option<TypeSpec>) -> bool {
    matches!(t, Some(TypeSpec::Ptr(_)) | Some(TypeSpec::Array(_, _)))
}

/// **Tipo estatico de una expresion**, hasta donde se puede saber sin
/// inventar. `None` significa *no lo se*, nunca *es un entero*.
pub(crate) fn tipo_de<A: Ambito + ?Sized>(amb: &A, e: &Expr) -> Option<TypeSpec> {
    match e {
        // -- nombres -------------------------------------------------------
        Expr::Var(n) => amb.tipo_de_variable(n),
        Expr::Assign(n, _) => amb.tipo_de_variable(n),
        // ** `p++` SIGUE SIENDO UN PUNTERO. Sin estos brazos `*p++` no sabia a
        // que apuntaba y leia ocho bytes por defecto -- que es EXACTAMENTE la
        // macro `va_arg`, y acerto por casualidad mientras se probo con un
        // tipo cuyo tamano coincide con el de por defecto.
        Expr::PreInc(n) | Expr::PreDec(n) | Expr::PostInc(n) | Expr::PostDec(n) => {
            amb.tipo_de_variable(n)
        }

        // -- llegar a un elemento ------------------------------------------
        Expr::Subscript(n, _) => match amb.tipo_de_variable(n)? {
            TypeSpec::Ptr(base) | TypeSpec::Array(base, _) => Some(*base),
            t => Some(t),
        },
        Expr::AssignSubscript(n, _, _) => match amb.tipo_de_variable(n)? {
            TypeSpec::Ptr(base) | TypeSpec::Array(base, _) => Some(*base),
            t => Some(t),
        },
        // *** `p[i]` es lo mismo que `*(p + i)`: su tipo es a lo que apunta la
        // BASE. Hasta el 2026-09-02 el elemento viajaba dentro del nodo,
        // puesto por el parser; ahora sale de la unica pregunta que este
        // fichero contesta, y por eso NO puede discrepar de `p + i`.
        Expr::IndexPtr(base, _) => apunta_a(amb, base),
        Expr::AssignIndexPtr(base, _, _) => apunta_a(amb, base),

        // -- indireccion ---------------------------------------------------
        Expr::Deref(inner) => match tipo_de(amb, inner)? {
            TypeSpec::Ptr(base) => Some(*base),
            // `*tabla` sobre un ARRAY es su primer elemento. Sin esto el
            // modismo mas comun de C --`sizeof(t)/sizeof(*t)`-- no resolvia.
            TypeSpec::Array(base, _) => Some(*base),
            _ => None,
        },
        // *** CAUSA B: sin este brazo el codegen no reconocia `&arr[5]` como
        // una direccion, el brazo de `Expr::Sub` no veia "puntero menos
        // puntero" sino "puntero menos ENTERO", y MULTIPLICABA el segundo
        // operando por el tamano del elemento. De ahi salia -679168 donde
        // tocaba -5: no se olvido de dividir, multiplico.
        Expr::AddrOf(inner) => Some(TypeSpec::Ptr(Box::new(tipo_de(amb, inner)?))),

        // -- campos: el agregado sale de la BASE ---------------------------
        Expr::Field(base, campo) | Expr::AssignField(base, campo, _) => {
            amb.tipo_de_campo(&agregado_de(amb, base)?, campo)
        }
        Expr::Arrow(base, campo) | Expr::AssignArrow(base, campo, _) => {
            amb.tipo_de_campo(&agregado_apuntado(amb, base)?, campo)
        }

        // *** EL MENOS UNARIO Y EL COMPLEMENTO, que el barrido del 04-09 no
        // cubrio y la verificacion del dia siguiente encontro.
        //
        // ```text
        //    (0 - a) >> 19    1028   correcto -- `Sub` si se recortaba
        //    (-a)    >> 19    18446744073709544452
        // ```
        //
        // ** La misma cuenta escrita de dos maneras daba dos resultados. `-a`
        // sobre un `unsigned int` es `0 - a` **en 32 bits**: da un numero
        // grande y positivo, no un negativo de 64. Sin este brazo el juez
        // contestaba `None` y el codegen dejaba pasar el signo entero.
        //
        // [!] Y esto NO es teorico en DOOM: `R_AddLine` hace `angle2 =
        // -clipangle;`. Ahi se salvaba de milagro --guardar en un `unsigned
        // int` recorta-- pero el mismo `-x` dentro de una expresion mas larga
        // no se salva. **Un arreglo que solo funciona cuando hay una asignacion
        // en medio es el mismo bug con otro traje.**
        Expr::Neg(a) | Expr::BitNot(a) => {
            let t = tipo_de(amb, a)?;
            if ancho(&t) < 4 {
                Some(if es_sin_signo(&t) { TypeSpec::UnsignedInt } else { TypeSpec::Int })
            } else {
                Some(t)
            }
        }

        // ** LOS OTROS DOS QUE SE PUEDEN PASAR DE 32 BITS.
        //
        // `Mul` porque dos numeros de 32 dan uno de 64, y `Shl` porque desplazar
        // saca bits por arriba. `Div`, `Mod` y los bit a bit no pueden producir
        // mas bits de los que entraron, asi que no necesitan recorte y no se
        // les pregunta.
        //
        // [!] En C el tipo de `a << b` es el de A SOLAS --el operando derecho
        // no participa--, que es distinto de todas las demas.
        Expr::Mul(a, b) => conversion_usual(tipo_de(amb, a), tipo_de(amb, b)),
        Expr::Shl(a, _) | Expr::Shr(a, _) => {
            let t = tipo_de(amb, a)?;
            if ancho(&t) < 4 {
                // Promocion a `int`: un `short` desplazado es un `int`.
                Some(if es_sin_signo(&t) { TypeSpec::UnsignedInt } else { TypeSpec::Int })
            } else {
                Some(t)
            }
        }

        // -- lo que se dice a si mismo -------------------------------------
        Expr::Cast(t, _) => Some(t.clone()),
        Expr::Int(_) => Some(TypeSpec::Int),
        Expr::CharLit(_) => Some(TypeSpec::Char),
        Expr::FloatLit(_) => Some(TypeSpec::Double),
        // En C `sizeof("abc")` son CUATRO: el literal es un array con su cero,
        // no un puntero.
        Expr::StringLit(s) => Some(TypeSpec::Array(
            Box::new(TypeSpec::Char),
            s.len() as u32 + 1,
        )),

        // -- *** LA ARITMETICA DE PUNTEROS (CAUSA A) -----------------------
        Expr::Add(a, b) => {
            let ta = tipo_de(amb, a);
            if es_direccion(&ta) {
                return Some(decaido(ta?));
            }
            let tb = tipo_de(amb, b);
            if es_direccion(&tb) {
                // `1 + p` es tan legal como `p + 1`.
                return Some(decaido(tb?));
            }
            // ** Y SI NINGUNO ES PUNTERO, ES UNA CUENTA. Decir `None` aqui era
            // lo que dejaba al codegen sin el ancho, y sin ancho no hay recorte.
            conversion_usual(ta, tb)
        }
        Expr::Sub(a, b) => {
            let ta = tipo_de(amb, a);
            if !es_direccion(&ta) {
                return conversion_usual(ta, tipo_de(amb, b));
            }
            // [!] PUNTERO MENOS PUNTERO NO ES UN PUNTERO: es un indice.
            // Decir aqui que sigue siendo puntero haria que `(p - q)->campo`
            // resolviera un offset con toda confianza sobre algo que ya no
            // apunta a ningun sitio.
            if es_direccion(&tipo_de(amb, b)) {
                return Some(TypeSpec::Long);
            }
            if ta.is_none() || !es_direccion(&ta) {
                return conversion_usual(ta, tipo_de(amb, b));
            }
            Some(decaido(ta?))
        }

        // ** Una LLAMADA vale lo que su funcion declara devolver.
        Expr::Call(nombre, _) => amb.tipo_de_retorno(nombre),

        // -- ramas ---------------------------------------------------------
        Expr::Conditional(_, a, b) => tipo_de(amb, a).or_else(|| tipo_de(amb, b)),
        Expr::Comma(v) => tipo_de(amb, v.last()?),

        // `entero op entero` cae aqui A PROPOSITO. Ver la cabecera.
        _ => None,
    }
}

/// **A que apunta esta expresion**, ya decaido el array.
///
/// Es `tipo_de` mas un paso, y por eso las dos preguntas no pueden volver a
/// divergir: quien pregunta por el PASO y quien pregunta por el OFFSET leen la
/// misma respuesta.
pub(crate) fn apunta_a<A: Ambito + ?Sized>(amb: &A, e: &Expr) -> Option<TypeSpec> {
    match tipo_de(amb, e)? {
        TypeSpec::Ptr(base) | TypeSpec::Array(base, _) => Some(*base),
        _ => None,
    }
}

/// Nombre del struct/union del que una expresion ES valor directo (`base.campo`).
pub(crate) fn agregado_de<A: Ambito + ?Sized>(amb: &A, e: &Expr) -> Option<String> {
    match tipo_de(amb, e)? {
        TypeSpec::StructRef(s) | TypeSpec::UnionRef(s) => Some(s),
        _ => None,
    }
}

/// Nombre del struct/union al que APUNTA una expresion (`base->campo`).
///
/// [!] Pasa por `apunta_a`, o sea que hereda la decadencia de arrays y la
/// aritmetica de punteros. **Ese es exactamente el arreglo del 02-09**:
/// `(tope - 1)->next` no resolvia porque esta pregunta no se hacia aqui.
pub(crate) fn agregado_apuntado<A: Ambito + ?Sized>(amb: &A, e: &Expr) -> Option<String> {
    match apunta_a(amb, e)? {
        TypeSpec::StructRef(s) | TypeSpec::UnionRef(s) => Some(s),
        _ => None,
    }
}

/// **El tipo de una cuenta entre enteros**, o sea las conversiones aritmeticas
/// usuales de C recortadas a lo que este compilador tiene.
///
/// # *** POR QUE ESTO NO PODIA SEGUIR CONTESTANDO `None`
///
/// Este juez nacio para encontrar PUNTEROS, asi que a `entero + entero` decia
/// "no se" y con eso bastaba. El 04-09 dejo de bastar.
///
/// DOOM hace, en una sola expresion:
///
/// ```c
///    angle2 = (angle2 + ANG90) >> ANGLETOFINESHIFT;
/// ```
///
/// La suma de dos `unsigned int` **tiene que envolver a 32 bits**, y el codegen
/// la hacia en registros de 64 sin recortar. El `>>` veia un numero de treinta y
/// tres bits y devolvia 9212 donde tocaba 1020. Ese numero es un indice de
/// `viewangletox[]`, que tiene 4096 entradas.
///
/// ** Y el codegen no podia recortar porque no sabia el ancho: preguntaba aqui
/// y se le contestaba `None`. **El recorte que faltaba no era una instruccion:
/// era una respuesta.**
///
/// [!] Se para en `Long` y no distingue `long long`: este compilador no tiene
/// mas anchos que 32 y 64, y un juez que finge saber mas de lo que el codegen
/// puede emitir es peor que uno corto.
fn conversion_usual(ta: Option<TypeSpec>, tb: Option<TypeSpec>) -> Option<TypeSpec> {
    let (a, b) = (ta?, tb?);
    if ancho(&a) == 8 || ancho(&b) == 8 {
        // Uno de los dos ya es de 64: manda el ancho, y el signo lo pone quien
        // lo aporta. No hay recorte que hacer.
        return Some(if ancho(&a) == 8 { a } else { b });
    }
    // Los dos caben en 32: promocion a `int`, y si alguno no lleva signo, el
    // resultado tampoco. Es la regla de C y es la que decide si el recorte del
    // codegen ensancha con ceros o con el bit de signo.
    if es_sin_signo(&a) || es_sin_signo(&b) {
        Some(TypeSpec::UnsignedInt)
    } else {
        Some(TypeSpec::Int)
    }
}

/// El ancho en bytes de lo que este compilador sabe emitir. 8 para todo lo que
/// no quepa en 32, que incluye los punteros.
fn ancho(t: &TypeSpec) -> u32 {
    match t {
        TypeSpec::Char | TypeSpec::UnsignedChar => 1,
        TypeSpec::Short | TypeSpec::UnsignedShort => 2,
        TypeSpec::Int | TypeSpec::UnsignedInt => 4,
        _ => 8,
    }
}

fn es_sin_signo(t: &TypeSpec) -> bool {
    matches!(
        t,
        TypeSpec::UnsignedChar | TypeSpec::UnsignedShort | TypeSpec::UnsignedInt
    )
}

/// **Cabe el resultado de `e` en 32 bits?** `Some(sin_signo)` si si.
///
/// Es lo unico que el codegen necesita saber para decidir si recorta, y con
/// que: con ceros o con el bit de signo.
pub(crate) fn recorte_de<A: Ambito + ?Sized>(amb: &A, e: &Expr) -> Option<bool> {
    let t = tipo_de(amb, e)?;
    if ancho(&t) == 4 {
        Some(es_sin_signo(&t))
    } else {
        None
    }
}
