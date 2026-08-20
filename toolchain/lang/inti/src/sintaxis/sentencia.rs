//! `sintaxis::sentencia` -- las cosas que se hacen, una por linea.
//!
//! ## La decision que vive aqui
//!
//! **Asignar es una sentencia y nunca una expresion.** Todo lo demas del
//! lenguaje depende de eso: es lo que deja que `=` signifique *igual* en los dos
//! sitios sin una sola ambiguedad, que es la solucion de Quorum y la razon de
//! que no haya `==`.
//!
//! Se ve en el codigo: [`sentencia`] lee una expresion y **despues** mira si
//! viene un `=`. Si viniera, y las expresiones pudieran asignar, esa misma
//! linea tendria dos lecturas.

use super::{expresion::expresion, lee_tipo, Cursor};
use crate::arbol::*;
use crate::aviso::{codigos, Aviso};
use crate::lexico::{Clase, Signo};
use crate::palabras::Simbolo;

pub(crate) fn sentencia(c: &mut Cursor) -> Option<Sent> {
    let sitio = c.sitio();

    if c.mira().es(Simbolo::Si) {
        return si(c);
    }
    if c.mira().es(Simbolo::Para) {
        return para_cada(c);
    }
    if c.mira().es(Simbolo::Repite) {
        return repite(c);
    }
    if c.mira().es(Simbolo::Devuelve) {
        c.avanza();
        // `devuelve` a secas vale: la funcion no devuelve nada.
        let valor = if matches!(c.mira().clase, Clase::FinLinea | Clase::Desangra | Clase::Fin) {
            None
        } else {
            expresion(c)
        };
        c.fin_de_linea();
        return Some(Sent::Devuelve { valor, sitio });
    }
    if c.mira().es(Simbolo::Falla) {
        c.avanza();
        let motivo = expresion(c)?;
        c.fin_de_linea();
        return Some(Sent::Falla { motivo, sitio });
    }
    if c.come(Simbolo::Corta) {
        c.fin_de_linea();
        return Some(Sent::Corta(sitio));
    }
    if c.come(Simbolo::Continua) {
        c.fin_de_linea();
        return Some(Sent::Continua(sitio));
    }
    // Una funcion dentro de otra se detecta a proposito: sin esto saldria un
    // error de sintaxis que no explica nada.
    if c.mira().es(Simbolo::Funcion) {
        c.di(
            Aviso::nuevo(
                codigos::SIN_FUNCION_ANIDADA,
                "En INTI una funcion no puede vivir dentro de otra.",
                sitio,
            )
            .con_habia(
                "No hay funciones anidadas ni anonimas, y el motivo es del perfil: una                  captura hay que guardarla en algun sitio, y en `llano` no hay monton.                  Lo que si hay es la funcion como valor: `f = media` y luego `f(x)`."
                    .to_string(),
            )
            .con_hacer("sacala al margen del fichero y llamala desde aqui"),
        );
        c.hasta_fin_de_linea();
        return None;
    }
    if c.mira().es(Simbolo::Crudo) {
        c.avanza();
        let cuerpo = c.bloque();
        return Some(Sent::Crudo { cuerpo, sitio });
    }
    // `en paralelo`
    if c.mira().es(Simbolo::En) {
        let guardado = c.i;
        c.avanza();
        if c.come(Simbolo::Paralelo) {
            let cuerpo = c.bloque();
            return Some(Sent::Paralelo { cuerpo, sitio });
        }
        c.i = guardado;
    }

    if let Some(l) = llamada_de_sentencia(c) {
        return Some(l);
    }

    asignacion_o_expresion(c)
}

/// **La forma de sentencia**: una llamada sin parentesis.
///
/// ```text
///    escribe "hola"                ->  escribe("hola")
///    escribe "media:", m           ->  escribe("media:", m)
///    guarda "notas.txt", texto     ->  guarda("notas.txt", texto)
/// ```
///
/// ## Por que esto NO rompe la regla de que `f` y `f()` se ven distintos
///
/// Porque solo vale **al principio de una sentencia y con argumentos**. Un
/// nombre solo (`f`) sigue siendo el valor de la funcion; `f()` sigue siendo la
/// llamada sin argumentos. Lo que se gana es el caso que se escribe mil veces
/// al dia, y se gana sin una sola ambiguedad -- porque la decision se toma
/// mirando UNA pieza.
///
/// ## La pieza que decide
///
/// Detras del nombre tiene que venir algo que **empieza un valor y no puede
/// continuar una expresion**: un texto, un numero, otro nombre, un tipo,
/// `cierto`/`falso`/`nada`, o una tabla.
///
/// Lo que queda fuera, y cada uno por su motivo:
///
/// ```text
///    x = 5           `=` continua la sentencia    -> asignacion
///    p.x = 3         `.` continua el nombre       -> asignacion
///    notas[0] = 5    `[` es un indice             -> asignacion
///    total - 1       `-` es ambiguo               -> expresion
///    f(1)            `(` es la llamada de siempre -> se lee como siempre
/// ```
///
/// OJO: el `-` se queda fuera a proposito. `escribe -1` podria ser
/// `escribe(-1)` o una resta, y **una regla que hay que pensar no simplifica
/// nada**. Para pasar un negativo: `escribe(-1)`.
fn llamada_de_sentencia(c: &mut Cursor) -> Option<Sent> {
    let sitio = c.sitio();

    let nombre = match c.mira().clase.clone() {
        Clase::Nombre(n) => n,
        Clase::Palabra(s) if super::es_nombrable(s) => c.vocab.texto(s).to_string(),
        _ => return None,
    };

    if !empieza_un_argumento(c.piezas.get(c.i + 1)) {
        return None;
    }
    c.avanza();

    let mut argumentos = Vec::new();
    loop {
        let valor = super::expresion::expresion(c)?;
        argumentos.push(Argumento {
            nombre: None,
            valor,
        });
        if !c.come_signo(Signo::Coma) {
            break;
        }
    }
    c.fin_de_linea();

    Some(Sent::Expresion(Expr::Llamada {
        que: Box::new(Expr::Nombre(nombre, sitio)),
        argumentos,
        sitio,
    }))
}

fn empieza_un_argumento(p: Option<&crate::lexico::Pieza>) -> bool {
    match p {
        Some(p) => match &p.clase {
            Clase::Texto(_) | Clase::Numero(_) | Clase::Nombre(_) | Clase::Tipo(_) => true,
            Clase::Signo(Signo::LlaveAbre) => true,
            Clase::Palabra(s) => matches!(
                s,
                Simbolo::Cierto | Simbolo::Falso | Simbolo::Nada
            ),
            _ => false,
        },
        None => false,
    }
}

fn si(c: &mut Cursor) -> Option<Sent> {
    let sitio = c.sitio();
    c.avanza(); // si

    let mut ramas = Vec::new();
    let cond = expresion(c)?;
    let cuerpo = c.bloque();
    ramas.push((cond, cuerpo));

    let mut sino = None;
    while c.mira().es(Simbolo::Sino) {
        c.avanza();
        if c.come(Simbolo::Si) {
            let cond = expresion(c)?;
            let cuerpo = c.bloque();
            ramas.push((cond, cuerpo));
            continue;
        }
        sino = Some(c.bloque());
        break;
    }

    Some(Sent::Si { ramas, sino, sitio })
}

fn para_cada(c: &mut Cursor) -> Option<Sent> {
    let sitio = c.sitio();
    c.avanza(); // para
    c.exige(Simbolo::Cada, "El bucle se escribe `para cada x en lista`.");

    // `para cada a en ...`: `a` es palabra clave y aun asi es un nombre
    // perfectamente normal para un elemento.
    let nombre = match c.mira().clase.clone() {
        Clase::Palabra(s) if super::es_nombrable(s) => {
            let n = c.vocab.texto(s).to_string();
            c.avanza();
            n
        }
        Clase::Nombre(n) => {
            c.avanza();
            n
        }
        _ => {
            let hay = c.mira().como_se_llama();
            c.di(
                Aviso::nuevo(
                    codigos::PAREJA_ROTA,
                    "Aqui va el nombre que toma cada elemento.",
                    sitio,
                )
                .con_habia(format!("Hay {}.", hay))
                .con_hacer("por ejemplo `para cada alumno en alumnos`"),
            );
            c.hasta_fin_de_linea();
            return None;
        }
    };

    c.exige(Simbolo::En, "El bucle se escribe `para cada x en lista`.");
    let desde = expresion(c)?;
    // `0 hasta 10`: un rango NO es un valor en INTI, asi que vive en el bucle.
    let hasta = if c.come(Simbolo::Hasta) {
        expresion(c)
    } else {
        None
    };
    let cuerpo = c.bloque();

    Some(Sent::ParaCada {
        nombre,
        desde,
        hasta,
        cuerpo,
        sitio,
    })
}

fn repite(c: &mut Cursor) -> Option<Sent> {
    let sitio = c.sitio();
    c.avanza(); // repite

    // `repite` a secas: infinito a proposito.
    if matches!(c.mira().clase, Clase::FinLinea) {
        let cuerpo = c.bloque();
        return Some(Sent::Repite {
            forma: Repeticion::Siempre,
            cuerpo,
            sitio,
        });
    }

    if c.come(Simbolo::Mientras) {
        let cond = expresion(c)?;
        let cuerpo = c.bloque();
        return Some(Sent::Repite {
            forma: Repeticion::Mientras(cond),
            cuerpo,
            sitio,
        });
    }

    let cuantas = expresion(c)?;
    c.exige(Simbolo::Veces, "Se escribe `repite 10 veces`.");
    let cuerpo = c.bloque();
    Some(Sent::Repite {
        forma: Repeticion::Veces(cuantas),
        cuerpo,
        sitio,
    })
}

fn asignacion_o_expresion(c: &mut Cursor) -> Option<Sent> {
    let sitio = c.sitio();
    let cambiante = c.come(Simbolo::Cambiante);

    // Se decide ANTES de leer nada: hay un `=` suelto en esta linea?
    //
    // Hay que mirar por delante porque el destino se lee con `sufijo` y una
    // expresion con `expresion`, y son dos parsers distintos. La alternativa
    // --leer una expresion y arrepentirse-- no funciona: el nivel de
    // comparacion ya se habria comido el `=`.
    if !cambiante && !hay_igual_suelto(c) {
        let e = super::expresion::expresion(c)?;
        c.fin_de_linea();
        return Some(Sent::Expresion(e));
    }

    let destino = super::expresion::sufijo(c)?;

    // El tipo explicito solo cabe en una declaracion: `cambiante x es entero32 = 0`.
    let tipo = if c.come(Simbolo::Es) { lee_tipo(c) } else { None };

    if c.come_signo(Signo::Igual) {
        if !destino.es_destino() {
            c.di(
                Aviso::nuevo(
                    codigos::PAREJA_ROTA,
                    "A esto no se le puede asignar un valor.",
                    destino.sitio(),
                )
                .con_habia(
                    "A la izquierda de un `=` va un nombre, un campo (`p.x`) o una posicion (`notas[0]`)."
                        .to_string(),
                )
                .con_hacer("guarda el resultado en un nombre"),
            );
        }
        let valor = super::expresion::expresion(c)?;
        c.fin_de_linea();
        return Some(Sent::Asigna {
            destino,
            cambiante,
            tipo,
            valor,
            sitio,
        });
    }

    if cambiante {
        c.di(
            Aviso::nuevo(
                codigos::SIN_VALOR,
                "Un nombre `cambiante` nace con un valor.",
                sitio,
            )
            .con_habia(
                "En INTI no se declara sin valor. Por eso leer una variable sin inicializar --que en C es comportamiento indefinido-- aqui no se puede ni escribir."
                    .to_string(),
            )
            .con_hacer("dale un valor: `cambiante x = 0`"),
        );
        c.hasta_fin_de_linea();
        return None;
    }

    c.fin_de_linea();
    Some(Sent::Expresion(destino))
}

/// Hay un `=` en lo que queda de linea, **fuera de cualquier pareja**?
///
/// La profundidad importa: en `escribe(a = b)` ese `=` es una comparacion
/// dentro de la llamada, no una asignacion de la linea.
fn hay_igual_suelto(c: &Cursor) -> bool {
    let mut j = c.i;
    let mut hondo = 0i32;
    while let Some(p) = c.piezas.get(j) {
        match p.clase {
            Clase::FinLinea | Clase::Fin | Clase::Sangra | Clase::Desangra => return false,
            Clase::Signo(s) if s.abre() => hondo += 1,
            Clase::Signo(Signo::ParenCierra)
            | Clase::Signo(Signo::CorcheteCierra)
            | Clase::Signo(Signo::LlaveCierra) => hondo -= 1,
            Clase::Signo(Signo::Igual) if hondo == 0 => return true,
            _ => {}
        }
        j += 1;
    }
    false
}
