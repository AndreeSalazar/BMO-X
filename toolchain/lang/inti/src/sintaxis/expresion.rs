//! `sintaxis::expresion` -- la precedencia, en un sitio.
//!
//! ## Por que aparte
//!
//! Porque la precedencia es **una tabla disfrazada de funciones**: diez niveles,
//! cada uno llamando al de abajo. Metida entre las sentencias, cambiar un nivel
//! obliga a leer el fichero entero para saber si algo mas se movio.
//!
//! Los niveles son los de `GRAMATICA.md` sec. 6, y estan en el mismo orden a
//! proposito: si alguien cambia el documento y no este fichero, la diferencia se
//! ve poniendo los dos al lado.
//!
//! ```text
//!    10  o si no          (lo mas externo)
//!     9  o
//!     8  y
//!     7  = / no es / es un / < > <= >=
//!     6  bits_y bits_o bits_xor desplaza
//!     5  + -
//!     4  * / entre resto
//!     3  elevado a        (asocia a la DERECHA)
//!     2  -x  no x
//!     1  f(...)  a[i]  a.campo
//! ```

use super::Cursor;
use crate::arbol::*;
use crate::aviso::{codigos, Aviso};
use crate::lexico::{Clase, Signo};
use crate::palabras::Simbolo;

/// El nivel de mas fuera.
pub(crate) fn expresion(c: &mut Cursor) -> Option<Expr> {
    let izq = o_logico(c)?;

    // `o si no` -- se lee aqui porque envuelve a todo lo demas.
    if c.mira().es(Simbolo::O) {
        let guardado = c.i;
        let sitio = c.sitio();
        c.avanza();
        if c.come(Simbolo::Si) && c.come(Simbolo::No) {
            // Con bloque: `o si no` + final de linea + sangria.
            if matches!(c.mira().clase, Clase::FinLinea) {
                let cuerpo = c.bloque();
                return Some(Expr::OSiNo {
                    intento: Box::new(izq),
                    respaldo: Respaldo::Bloque(cuerpo),
                    sitio,
                });
            }
            let valor = expresion(c)?;
            return Some(Expr::OSiNo {
                intento: Box::new(izq),
                respaldo: Respaldo::Valor(Box::new(valor)),
                sitio,
            });
        }
        // Era un `o` normal que ya se comio arriba: se vuelve.
        c.i = guardado;
    }

    Some(izq)
}

fn o_logico(c: &mut Cursor) -> Option<Expr> {
    let mut izq = y_logico(c)?;
    // Cuidado: `o` tambien empieza `o si no`. Si detras viene `si no`, este
    // nivel no lo toca.
    while c.mira().es(Simbolo::O) && !empieza_o_si_no(c) {
        let sitio = c.sitio();
        c.avanza();
        let der = y_logico(c)?;
        izq = binaria(Op::O, izq, der, sitio);
    }
    Some(izq)
}

fn empieza_o_si_no(c: &Cursor) -> bool {
    let a = c.piezas.get(c.i + 1);
    let b = c.piezas.get(c.i + 2);
    matches!(a, Some(p) if p.es(Simbolo::Si)) && matches!(b, Some(p) if p.es(Simbolo::No))
}

fn y_logico(c: &mut Cursor) -> Option<Expr> {
    let mut izq = comparacion(c)?;
    while c.mira().es(Simbolo::Y) {
        let sitio = c.sitio();
        c.avanza();
        let der = comparacion(c)?;
        izq = binaria(Op::Y, izq, der, sitio);
    }
    Some(izq)
}

fn comparacion(c: &mut Cursor) -> Option<Expr> {
    let mut izq = bits(c)?;
    loop {
        let sitio = c.sitio();
        let op = if c.mira().es_signo(Signo::Igual) {
            c.avanza();
            Op::Igual
        } else if c.mira().es_signo(Signo::Menor) {
            c.avanza();
            Op::Menor
        } else if c.mira().es_signo(Signo::Mayor) {
            c.avanza();
            Op::Mayor
        } else if c.mira().es_signo(Signo::MenorIgual) {
            c.avanza();
            Op::MenorIgual
        } else if c.mira().es_signo(Signo::MayorIgual) {
            c.avanza();
            Op::MayorIgual
        } else if c.mira().es(Simbolo::No) {
            // `no es`
            let guardado = c.i;
            c.avanza();
            if c.come(Simbolo::Es) {
                Op::NoEs
            } else {
                c.i = guardado;
                return Some(izq);
            }
        } else if c.mira().es(Simbolo::Es) {
            c.avanza();
            // `es un` pregunta el TIPO; `es` a secas compara.
            if c.come(Simbolo::Un) {
                Op::EsUn
            } else {
                Op::Igual
            }
        } else {
            return Some(izq);
        };
        let der = bits(c)?;
        izq = binaria(op, izq, der, sitio);
    }
}

fn bits(c: &mut Cursor) -> Option<Expr> {
    let mut izq = suma(c)?;
    loop {
        let sitio = c.sitio();
        let op = if c.come(Simbolo::BitsY) {
            Op::BitsY
        } else if c.come(Simbolo::BitsO) {
            Op::BitsO
        } else if c.come(Simbolo::BitsXor) {
            Op::BitsXor
        } else if c.mira().es(Simbolo::Desplaza) {
            c.avanza();
            if c.come(Simbolo::Izquierda) {
                Op::DesplazaIzquierda
            } else if c.come(Simbolo::Derecha) {
                Op::DesplazaDerecha
            } else {
                let s = c.sitio();
                c.di(
                    Aviso::nuevo(
                        codigos::PAREJA_ROTA,
                        "Un desplazamiento dice hacia donde.",
                        s,
                    )
                    .con_hacer("escribe `desplaza izquierda 3` o `desplaza derecha 3`"),
                );
                Op::DesplazaIzquierda
            }
        } else {
            return Some(izq);
        };
        let der = suma(c)?;
        izq = binaria(op, izq, der, sitio);
    }
}

fn suma(c: &mut Cursor) -> Option<Expr> {
    let mut izq = producto(c)?;
    loop {
        let sitio = c.sitio();
        let op = if c.come_signo(Signo::Mas) {
            Op::Suma
        } else if c.come_signo(Signo::Menos) {
            Op::Resta
        } else {
            return Some(izq);
        };
        let der = producto(c)?;
        izq = binaria(op, izq, der, sitio);
    }
}

fn producto(c: &mut Cursor) -> Option<Expr> {
    let mut izq = potencia(c)?;
    loop {
        let sitio = c.sitio();
        let op = if c.come_signo(Signo::Por) {
            Op::Por
        } else if c.come_signo(Signo::Barra) {
            Op::Divide
        } else if c.come(Simbolo::Entre) {
            Op::Entre
        } else if c.come(Simbolo::Resto) {
            Op::Resto
        } else {
            return Some(izq);
        };
        let der = potencia(c)?;
        izq = binaria(op, izq, der, sitio);
    }
}

/// `elevado` asocia a la DERECHA: `2 elevado 3 elevado 2` es
/// `2 elevado (3 elevado 2)`, que es lo que dicen las matematicas.
///
/// Se escribia `elevado a` y se quito el `a` **porque `a` tambien es un nombre
/// que la gente usa**. Una palabra clave de una letra que ademas es el nombre
/// mas comun de una variable es una trampa, y la mas barata de quitar es la que
/// no hacia falta.
fn potencia(c: &mut Cursor) -> Option<Expr> {
    let izq = unaria(c)?;
    if c.mira().es(Simbolo::Elevado) {
        let sitio = c.sitio();
        c.avanza();
        let der = potencia(c)?;
        return Some(binaria(Op::Elevado, izq, der, sitio));
    }
    Some(izq)
}

fn unaria(c: &mut Cursor) -> Option<Expr> {
    let sitio = c.sitio();
    if c.come_signo(Signo::Menos) {
        let v = unaria(c)?;
        return Some(Expr::Unaria {
            op: OpUno::Menos,
            valor: Box::new(v),
            sitio,
        });
    }
    if c.mira().es(Simbolo::No) && !empieza_no_es(c) {
        c.avanza();
        let v = unaria(c)?;
        return Some(Expr::Unaria {
            op: OpUno::No,
            valor: Box::new(v),
            sitio,
        });
    }
    sufijo(c)
}

fn empieza_no_es(c: &Cursor) -> bool {
    matches!(c.piezas.get(c.i + 1), Some(p) if p.es(Simbolo::Es))
}

/// Llamadas, indices y campos, que se encadenan: `a.b(c)[d].e`.
///
/// Es `pub(crate)` por un motivo concreto: **el destino de una asignacion se lee
/// con ESTO y no con `expresion`**. Si se leyera con el parser completo, el
/// nivel de comparacion se comeria el `=` de `x = 1` y la asignacion no
/// existiria nunca. Es el precio exacto de que `=` signifique igual en los dos
/// sitios, y se paga aqui, en una linea.
pub(crate) fn sufijo(c: &mut Cursor) -> Option<Expr> {
    let mut e = primaria(c)?;
    loop {
        let sitio = c.sitio();
        if c.come_signo(Signo::ParenAbre) {
            let argumentos = argumentos(c);
            c.exige_signo(Signo::ParenCierra, "Una llamada cierra su parentesis.");
            e = Expr::Llamada {
                que: Box::new(e),
                argumentos,
                sitio,
            };
            continue;
        }
        if c.come_signo(Signo::CorcheteAbre) {
            let i = expresion(c)?;
            c.exige_signo(Signo::CorcheteCierra, "Un indice cierra su corchete.");
            e = Expr::Indice {
                que: Box::new(e),
                indice: Box::new(i),
                sitio,
            };
            continue;
        }
        if c.come_signo(Signo::Punto) {
            // Detras de un punto, **cualquier palabra vale**: un campo que se
            // llame `y` o `de` es legitimo, y ahi no hay nada que confundir
            // porque en esa posicion no cabe un operador.
            if let Clase::Palabra(s) = c.mira().clase {
                c.avanza();
                let nombre = c.vocab.texto(s).to_string();
                e = Expr::Campo {
                    que: Box::new(e),
                    nombre,
                    sitio,
                };
                continue;
            }
            match c.mira().clase.clone() {
                Clase::Nombre(n) => {
                    c.avanza();
                    e = Expr::Campo {
                        que: Box::new(e),
                        nombre: n,
                        sitio,
                    };
                }
                _ => {
                    let hay = c.mira().como_se_llama();
                    c.di(
                        Aviso::nuevo(
                            codigos::PAREJA_ROTA,
                            "Despues del punto va el nombre de un campo.",
                            sitio,
                        )
                        .con_habia(format!("Hay {}.", hay))
                        .con_hacer("por ejemplo `alumno.nota`"),
                    );
                    return Some(e);
                }
            }
            continue;
        }
        return Some(e);
    }
}

fn argumentos(c: &mut Cursor) -> Vec<Argumento> {
    let mut v = Vec::new();
    if c.mira().es_signo(Signo::ParenCierra) {
        return v;
    }
    loop {
        // `nombre: valor` -- argumento por nombre.
        let nombre = match (c.mira().clase.clone(), c.piezas.get(c.i + 1)) {
            (Clase::Nombre(n), Some(p)) if p.es_signo(Signo::DosPuntos) => {
                c.avanza();
                c.avanza();
                Some(n)
            }
            _ => None,
        };
        match expresion(c) {
            Some(valor) => v.push(Argumento { nombre, valor }),
            None => return v,
        }
        if !c.come_signo(Signo::Coma) {
            return v;
        }
    }
}

/// Las palabras clave que **tambien valen como nombre** cuando lo que toca es
/// un valor.
///
/// ## Por que existe esta lista
///
/// `y` y `o` son operadores... y son los nombres de variable mas usados del
/// mundo despues de `x`. Un lenguaje en el que no se puede escribir `x, y`
/// tiene un problema, y `p.y` tampoco compilaria.
///
/// La salida no es quitar los operadores: es que la palabra signifique una cosa
/// **en posicion de operador** y otra **en posicion de valor**. El parser
/// siempre sabe cual espera, asi que no hay ambiguedad: no se elige por
/// adivinacion, se elige por el sitio. Es lo mismo que hacen `await` en
/// JavaScript o `record` en Java.
const NOMBRABLES: &[Simbolo] = &[
    Simbolo::Y,
    Simbolo::O,
    Simbolo::A,
    Simbolo::Un,
    Simbolo::En,
    Simbolo::De,
];

fn primaria(c: &mut Cursor) -> Option<Expr> {
    let sitio = c.sitio();

    // Una palabra que en este sitio es un nombre.
    if let Clase::Palabra(s) = c.mira().clase {
        if NOMBRABLES.contains(&s) {
            c.avanza();
            let texto = c.vocab.texto(s).to_string();
            return Some(quiza_de(c, Expr::Nombre(texto, sitio), sitio));
        }
    }

    match c.mira().clase.clone() {
        Clase::Numero(n) => {
            c.avanza();
            Some(Expr::Numero(n, sitio))
        }
        Clase::Texto(t) => {
            c.avanza();
            Some(Expr::Texto(t, sitio))
        }
        Clase::Nombre(n) => {
            c.avanza();
            Some(quiza_de(c, Expr::Nombre(n, sitio), sitio))
        }
        Clase::Tipo(t) => {
            c.avanza();
            Some(Expr::Tipo(t, sitio))
        }
        Clase::Palabra(Simbolo::Cierto) => {
            c.avanza();
            Some(Expr::Logico(true, sitio))
        }
        Clase::Palabra(Simbolo::Falso) => {
            c.avanza();
            Some(Expr::Logico(false, sitio))
        }
        Clase::Palabra(Simbolo::Nada) => {
            c.avanza();
            Some(Expr::Nada(sitio))
        }
        // `valor de r`, `motivo de r`, `fallo r`: se leen como llamadas de la
        // biblioteca, porque eso es lo que son. Tenerlas como nodos propios
        // habria metido en el arbol tres formas que no anaden nada.
        Clase::Palabra(s @ (Simbolo::Valor | Simbolo::Motivo | Simbolo::Fallo)) => {
            c.avanza();
            let nombre = s.clave().to_lowercase();
            c.come(Simbolo::De);
            let arg = sufijo(c)?;
            Some(Expr::Llamada {
                que: Box::new(Expr::Nombre(nombre, sitio)),
                argumentos: vec![Argumento {
                    nombre: None,
                    valor: arg,
                }],
                sitio,
            })
        }
        Clase::Signo(Signo::ParenAbre) => {
            c.avanza();
            let e = expresion(c)?;
            c.exige_signo(Signo::ParenCierra, "Este parentesis se abrio y hay que cerrarlo.");
            Some(e)
        }
        Clase::Signo(Signo::CorcheteAbre) => {
            c.avanza();
            let mut v = Vec::new();
            if !c.mira().es_signo(Signo::CorcheteCierra) {
                loop {
                    v.push(expresion(c)?);
                    if !c.come_signo(Signo::Coma) {
                        break;
                    }
                }
            }
            c.exige_signo(Signo::CorcheteCierra, "Una lista cierra su corchete.");
            Some(Expr::Lista(v, sitio))
        }
        Clase::Signo(Signo::LlaveAbre) => {
            c.avanza();
            let mut v = Vec::new();
            if !c.mira().es_signo(Signo::LlaveCierra) {
                loop {
                    let k = expresion(c)?;
                    c.exige_signo(Signo::DosPuntos, "Una tabla se escribe `{clave: valor}`.");
                    let val = expresion(c)?;
                    v.push((k, val));
                    if !c.come_signo(Signo::Coma) {
                        break;
                    }
                }
            }
            c.exige_signo(Signo::LlaveCierra, "Una tabla cierra su llave.");
            Some(Expr::Tabla(v, sitio))
        }
        otra => {
            let hay = Pieza_como(&otra);
            c.di(
                Aviso::nuevo(codigos::PAREJA_ROTA, "Aqui falta un valor.", sitio)
                    .con_habia(format!("Hay {}, y eso no vale por si solo.", hay))
                    .con_hacer("escribe un numero, un texto, un nombre o una llamada"),
            );
            None
        }
    }
}

/// `cuenta de lista` es `cuenta(lista)`. **El mismo nodo**: `de` no es un
/// operador, es la forma de llamar con un argumento escrita como una frase, y
/// por eso no cuesta gramatica.
fn quiza_de(c: &mut Cursor, base: Expr, sitio: crate::aviso::Sitio) -> Expr {
    if !c.mira().es(Simbolo::De) {
        return base;
    }
    c.avanza();
    match sufijo(c) {
        Some(arg) => Expr::Llamada {
            que: Box::new(base),
            argumentos: vec![Argumento {
                nombre: None,
                valor: arg,
            }],
            sitio,
        },
        None => base,
    }
}

fn binaria(op: Op, izquierda: Expr, derecha: Expr, sitio: crate::aviso::Sitio) -> Expr {
    Expr::Binaria {
        op,
        izquierda: Box::new(izquierda),
        derecha: Box::new(derecha),
        sitio,
    }
}

#[allow(non_snake_case)]
fn Pieza_como(c: &Clase) -> String {
    crate::lexico::Pieza::nueva(c.clone(), crate::aviso::Sitio::default()).como_se_llama()
}
