//! **Tokens -> a list of named rules.** Answers: *what does each rule say?*
//!
//! A `Rule` does not know that other rules exist, exactly as `Fila` did not know
//! it had sisters. Which rule beats which is a **relation between two rules**,
//! so it belongs one generation up, in `cascade/`. The same goes for the
//! ordering guardian of `LA_MAQUETA_EXIGE.md` section 5 (tag rules before class
//! rules, so that "last wins" and "most specific wins" agree and the browser
//! preview cannot lie): comparing two rules is not this generation's job.

use crate::value::{self, Keyword, Prop, Shape, Value};
use crate::{Decl, Rule, Selector};
use bmo_maqueta_diag::Error;
use bmo_maqueta_lex::{Kind, Token};

use crate::markup::span_of;

pub fn parse(src: &[u8], toks: &[Token], errors: &mut Vec<Error>) -> Vec<Rule> {
    let mut rules = Vec::new();
    let mut i = 0usize;
    while i < toks.len() {
        match rule(src, toks, &mut i, errors) {
            Some(r) => rules.push(r),
            None => {
                // Nothing salvageable here: skip past the next `}` so a single
                // broken rule does not turn into an error on every line after it.
                while i < toks.len() && toks[i].kind != Kind::RBrace {
                    i += 1;
                }
                i += 1;
            }
        }
    }
    rules
}

fn rule(src: &[u8], toks: &[Token], i: &mut usize, errors: &mut Vec<Error>) -> Option<Rule> {
    let start = *toks.get(*i)?;
    let mut selectors = Vec::new();
    let mut hovers = Vec::new();

    loop {
        let (sel, hov) = selector(src, toks, i, errors)?;
        selectors.push(sel);
        hovers.push(hov);
        match toks.get(*i).map(|t| t.kind) {
            Some(Kind::Comma) => {
                *i += 1;
            }
            Some(Kind::LBrace) => {
                *i += 1;
                break;
            }
            Some(Kind::Ident) | Some(Kind::Dot) => {
                errors.push(Error::new(
                    span_of(&toks[*i]),
                    "los selectores de descendencia no existen",
                    "`.panel .boton` obliga a una caja a conocer a sus ancestros, y en \
                     MAQUETA una pieza no sabe que tiene padre (L7). Es la misma razon \
                     por la que no hay herencia.",
                    "poner la clase directamente en la caja que se quiere estilar, o \
                     separar con `,` si lo que se queria era estilar las dos.",
                ));
                return None;
            }
            other => {
                errors.push(Error::new(
                    toks.get(*i).map(span_of).unwrap_or(span_of(&start)),
                    "falta el `{` de la regla",
                    "despues del selector va el bloque de declaraciones.",
                    &format!(
                        "escribir `{{ ... }}`{}",
                        match other {
                            Some(k) => format!(" en vez de {}", k.name()),
                            None => String::new(),
                        }
                    ),
                ));
                return None;
            }
        }
    }

    let mut decls = Vec::new();
    loop {
        match toks.get(*i).map(|t| t.kind) {
            Some(Kind::RBrace) => {
                *i += 1;
                break;
            }
            Some(Kind::Semi) => {
                *i += 1;
            }
            None => {
                errors.push(Error::new(
                    span_of(&start),
                    "la regla se abrio y no se cerro",
                    "falta el `}`.",
                    "cerrar el bloque.",
                ));
                break;
            }
            _ => {
                if let Some(d) = declaration(src, toks, i, errors) {
                    decls.push(d);
                }
            }
        }
    }

    let hover = hovers.first().copied().unwrap_or(false);
    if hovers.iter().any(|h| *h != hover) {
        errors.push(Error::new(
            span_of(&start),
            "esta regla mezcla selectores con `:hover` y sin el",
            "asi la regla querria decir dos cosas a la vez, y habria que explicar              cual manda. Una regla es entera de reposo o entera de encima.",
            "partirla en dos reglas.",
        ));
    }
    if hover {
        for d in &decls {
            if !d.prop.es_pintura() {
                errors.push(Error::new(
                    d.span,
                    &format!("`{}` no puede ir en una regla `:hover`", d.prop.name()),
                    "una regla de `:hover` solo puede cambiar como se VE una caja,                      nunca donde esta. Si pudiera mover algo, la maquetacion habria que                      recalcularla por cada estado y dentro del aparato -- y entonces                      esto deja de ser un compilador y pasa a ser un motor en ejecucion.",
                    "solo `background-color`, `color`, `border-color` y `border-radius`.",
                ));
            }
        }
    }

    Some(Rule {
        selectors,
        decls,
        span: span_of(&start),
        hover,
    })
}

/// `:hover`, y nada mas. Devuelve si lo habia.
///
/// * Es la UNICA pseudo-clase, y esta admitida por una razon estructural, no por
/// utilidad: no puede tocar la maquetacion (ver el filtro de arriba), asi que la
/// maquetacion se sigue calculando UNA vez y `layout/` no llega a enterarse de
/// que el hover existe.
fn pseudo(src: &[u8], toks: &[Token], i: &mut usize, errors: &mut Vec<Error>) -> bool {
    if toks.get(*i).map(|t| t.kind) != Some(Kind::Colon) {
        return false;
    }
    let colon = toks[*i];
    let n = match toks.get(*i + 1) {
        Some(t) if t.kind == Kind::Ident => *t,
        _ => {
            errors.push(Error::new(
                span_of(&colon),
                "falta el nombre de la pseudo-clase",
                "solo existe una: `:hover`.",
                "por ejemplo `.tecla:hover`.",
            ));
            *i += 1;
            return false;
        }
    };
    *i += 2;
    if n.text(src) == b"hover" {
        return true;
    }
    errors.push(Error::new(
        span_of(&n),
        &format!(
            "pseudo-clase no soportada -- `:{}`",
            String::from_utf8_lossy(n.text(src))
        ),
        "la unica es `:hover`, y esta porque no puede mover nada. `:active`,          `:focus` y las demas piden un estado que MAQUETA no lleva: quien lleva el          estado es Rust.",
        "`:hover`, o dejarlo en manos del codigo.",
    ));
    false
}

fn selector(
    src: &[u8],
    toks: &[Token],
    i: &mut usize,
    errors: &mut Vec<Error>,
) -> Option<(Selector, bool)> {
    let t = *toks.get(*i)?;
    match t.kind {
        Kind::Dot => {
            *i += 1;
            let n = toks.get(*i)?;
            if n.kind != Kind::Ident {
                errors.push(Error::new(
                    span_of(n),
                    "falta el nombre de la clase despues del `.`",
                    "un selector de clase es un punto y un nombre.",
                    "por ejemplo `.tecla`.",
                ));
                return None;
            }
            *i += 1;
            let clase = Selector::Class(String::from_utf8_lossy(n.text(src)).into_owned());
            Some((clase, pseudo(src, toks, i, errors)))
        }
        Kind::Ident => {
            let raw = t.text(src).to_vec();
            *i += 1;
            match value::Tag::from_name(&raw) {
                Some(tag) => Some((Selector::Tag(tag), pseudo(src, toks, i, errors))),
                None => {
                    errors.push(value::unknown_tag(span_of(&t), &raw));
                    None
                }
            }
        }
        Kind::Hash => {
            errors.push(Error::new(
                span_of(&t),
                "los selectores de id no existen",
                "`id` es la clave de la tabla de golpeo -- por donde se sabe que \
                 boton se pulso. Si ademas estilara, un id valdria para dos cosas y \
                 cambiar una romperia la otra.",
                "usar una clase: `.tecla`.",
            ));
            *i += 1;
            None
        }
        _ => {
            errors.push(Error::new(
                span_of(&t),
                &format!("un selector no puede empezar por {}", t.kind.name()),
                "solo hay dos formas de selector: `etiqueta` y `.clase`.",
                "por ejemplo `div` o `.tecla`.",
            ));
            *i += 1;
            None
        }
    }
}

fn declaration(
    src: &[u8],
    toks: &[Token],
    i: &mut usize,
    errors: &mut Vec<Error>,
) -> Option<Decl> {
    let name_tok = *toks.get(*i)?;
    if name_tok.kind != Kind::Ident {
        errors.push(Error::new(
            span_of(&name_tok),
            &format!("aqui esperaba el nombre de una propiedad, y hay {}", name_tok.kind.name()),
            "dentro de un bloque van declaraciones `nombre: valor`.",
            "revisar si falta un `;` en la linea anterior.",
        ));
        *i += 1;
        return None;
    }
    let raw = name_tok.text(src).to_vec();
    *i += 1;

    let prop = match Prop::from_name(&raw) {
        Some(p) => p,
        None => {
            errors.push(value::unknown_prop(span_of(&name_tok), &raw));
            skip_value(toks, i);
            return None;
        }
    };

    if toks.get(*i).map(|t| t.kind) != Some(Kind::Colon) {
        errors.push(Error::new(
            span_of(&name_tok),
            &format!("falta el `:` despues de `{}`", prop.name()),
            "una declaracion es `nombre: valor`.",
            "anadir los dos puntos.",
        ));
        skip_value(toks, i);
        return None;
    }
    *i += 1;

    let v = read_value(src, toks, i, prop, errors)?;
    Some(Decl {
        prop,
        value: v,
        span: span_of(&name_tok),
    })
}

fn read_value(
    src: &[u8],
    toks: &[Token],
    i: &mut usize,
    prop: Prop,
    errors: &mut Vec<Error>,
) -> Option<Value> {
    // `inherit` and friends are VALUES, not properties, so the rejection table
    // keyed by property name would never have seen them. Catching them here is
    // worth the special case: they are the exact words someone reaches for when
    // they hit the missing feature, and a generic "quiere un color" would send
    // them looking in the wrong place.
    if let Some(t) = toks.get(*i) {
        if t.kind == Kind::Ident {
            let w = t.text(src);
            if w == b"inherit" || w == b"initial" || w == b"unset" {
                *i += 1;
                errors.push(Error::new(
                    span_of(t),
                    &format!("`{}` no existe", String::from_utf8_lossy(w)),
                    "no hay herencia: en MAQUETA una pieza no sabe que tiene padre \
                     (L7), asi que no hay nada de donde heredar ni a donde volver.",
                    "declarar el valor donde hace falta. Con estilos de ambito corto, \
                     repetirlo cuesta menos que la regla que lo evitaba.",
                ));
                skip_value(toks, i);
                return None;
            }
        }
    }
    match prop.shape() {
        Shape::OnePx => measure(src, toks, i, prop, errors).map(Value::Px),
        Shape::OneOrFourPx => {
            let first = measure(src, toks, i, prop, errors)?;
            if !at_value_start(toks, *i) {
                return Some(Value::Px4([first; 4]));
            }
            let mut four = [first, 0, 0, 0];
            for slot in four.iter_mut().skip(1) {
                *slot = measure(src, toks, i, prop, errors)?;
            }
            if at_value_start(toks, *i) {
                errors.push(Error::new(
                    span_of(&toks[*i]),
                    &format!("`{}` acepta uno o cuatro valores, no mas", prop.name()),
                    "uno son los cuatro lados; cuatro son arriba, derecha, abajo e \
                     izquierda, en el orden de CSS.",
                    "quitar lo que sobra.",
                ));
                skip_value(toks, i);
            }
            Some(Value::Px4(four))
        }
        Shape::Color => {
            let t = *toks.get(*i)?;
            if t.kind == Kind::Color {
                *i += 1;
                let hex = t.text(src);
                let mut n = 0u32;
                for &c in hex {
                    n = (n << 4) | (c as char).to_digit(16).unwrap_or(0);
                }
                return Some(Value::Color(n));
            }
            errors.push(Error::new(
                span_of(&t),
                &format!("`{}` quiere un color `#RRGGBB`", prop.name()),
                "no hay nombres de color, ni `rgb()`, ni `rgba()`: el pixel de BMO-X \
                 es `u32` en `0x00RRGGBB` y no hay mezcla alfa.",
                "por ejemplo `#182434`. La paleta del sistema esta en \
                 `toolchain/tools/maqueta/tema/tema.maqueta`.",
            ));
            skip_value(toks, i);
            None
        }
        Shape::Words(allowed) => {
            let t = *toks.get(*i)?;
            if t.kind == Kind::Ident {
                let raw = t.text(src).to_vec();
                if let Some(k) = Keyword::from_name(&raw) {
                    if allowed.contains(&k) {
                        *i += 1;
                        return Some(Value::Word(k));
                    }
                }
                *i += 1;
                errors.push(Error::new(
                    span_of(&t),
                    &format!(
                        "`{}` no es un valor de `{}`",
                        String::from_utf8_lossy(&raw),
                        prop.name()
                    ),
                    "cada propiedad tiene su lista cerrada de palabras.",
                    &format!("aqui van: {}.", list(allowed)),
                ));
                skip_value(toks, i);
                return None;
            }
            errors.push(Error::new(
                span_of(&t),
                &format!("`{}` quiere una palabra", prop.name()),
                "esta propiedad no lleva numero.",
                &format!("aqui van: {}.", list(allowed)),
            ));
            skip_value(toks, i);
            None
        }
    }
}

/// A number followed by `px`. The only unit there is.
fn measure(
    src: &[u8],
    toks: &[Token],
    i: &mut usize,
    prop: Prop,
    errors: &mut Vec<Error>,
) -> Option<u32> {
    let t = *toks.get(*i)?;
    if t.kind != Kind::Number {
        errors.push(Error::new(
            span_of(&t),
            &format!("`{}` quiere un numero de pixeles", prop.name()),
            "esta propiedad es una medida.",
            "por ejemplo `72px`.",
        ));
        skip_value(toks, i);
        return None;
    }
    let digits = t.text(src);
    let mut n: u32 = 0;
    let mut overflow = false;
    for &c in digits {
        match n.checked_mul(10).and_then(|x| x.checked_add((c - b'0') as u32)) {
            Some(v) => n = v,
            None => overflow = true,
        }
    }
    if overflow {
        errors.push(Error::new(
            span_of(&t),
            "el numero no cabe",
            "las medidas son enteros de 32 bits sin signo.",
            "un numero de pixeles razonable para una pantalla.",
        ));
        *i += 1;
        return None;
    }
    *i += 1;

    match toks.get(*i).map(|t| (t.kind, *t)) {
        Some((Kind::Ident, u)) => {
            let unit = u.text(src);
            if unit == b"px" {
                *i += 1;
                Some(n)
            } else {
                *i += 1;
                errors.push(Error::new(
                    span_of(&u),
                    &format!("unidad no soportada -- `{}`", String::from_utf8_lossy(unit)),
                    "solo existe `px`, y en enteros. `em` y `rem` se miden contra una \
                     tipografia que aqui no se puede elegir; `vh` y `vw` contra un \
                     contenedor que una pieza no conoce (L7).",
                    "un numero exacto de pixeles.",
                ));
                None
            }
        }
        Some((Kind::Pct, u)) => {
            *i += 1;
            errors.push(Error::new(
                span_of(&u),
                "unidad no soportada -- `%`",
                "los porcentajes exigen conocer el contenedor, y en MAQUETA una pieza \
                 no sabe que tiene padre (L7).",
                "un pixel exacto, o `display:flex` en el padre repartiendo con `gap`.",
            ));
            None
        }
        _ if n == 0 => Some(0),
        _ => {
            errors.push(Error::new(
                span_of(&t),
                "falta la unidad",
                "las medidas llevan `px` siempre, menos el cero.",
                &format!("`{n}px`."),
            ));
            None
        }
    }
}

fn at_value_start(toks: &[Token], i: usize) -> bool {
    matches!(
        toks.get(i).map(|t| t.kind),
        Some(Kind::Number) | Some(Kind::Color) | Some(Kind::Ident)
    )
}

/// Walk to the end of the current declaration so one bad value does not turn
/// into a complaint about every token after it.
fn skip_value(toks: &[Token], i: &mut usize) {
    while let Some(t) = toks.get(*i) {
        if t.kind == Kind::Semi || t.kind == Kind::RBrace {
            return;
        }
        *i += 1;
    }
}

fn list(ks: &[Keyword]) -> String {
    ks.iter()
        .map(|k| format!("`{}`", k.name()))
        .collect::<Vec<_>>()
        .join(", ")
}
