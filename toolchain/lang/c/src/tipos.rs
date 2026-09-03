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
        Expr::Subscript(n, _, _) => match amb.tipo_de_variable(n)? {
            TypeSpec::Ptr(base) | TypeSpec::Array(base, _) => Some(*base),
            t => Some(t),
        },
        Expr::AssignSubscript(n, _, _, _) => match amb.tipo_de_variable(n)? {
            TypeSpec::Ptr(base) | TypeSpec::Array(base, _) => Some(*base),
            t => Some(t),
        },
        // `p[i]` ya trae el tipo del elemento DENTRO del nodo: exacto, no una
        // suposicion.
        Expr::IndexPtr(_, _, elem) => Some(elem.clone()),
        Expr::AssignIndexPtr(_, _, elem, _) => Some(elem.clone()),

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

        // -- campos: el tipo VIAJA en el nodo ------------------------------
        Expr::Field(_, _, _, t) | Expr::Arrow(_, _, _, t) => Some(t.clone()),
        Expr::AssignField(_, _, _, t, _) | Expr::AssignArrow(_, _, _, t, _) => Some(t.clone()),

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
            None
        }
        Expr::Sub(a, b) => {
            let ta = tipo_de(amb, a);
            if !es_direccion(&ta) {
                return None;
            }
            // [!] PUNTERO MENOS PUNTERO NO ES UN PUNTERO: es un indice.
            // Decir aqui que sigue siendo puntero haria que `(p - q)->campo`
            // resolviera un offset con toda confianza sobre algo que ya no
            // apunta a ningun sitio.
            if es_direccion(&tipo_de(amb, b)) {
                return Some(TypeSpec::Long);
            }
            Some(decaido(ta?))
        }

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
