//! **Tokens -> a tree of named boxes.** Answers: *what is each piece?*
//!
//! ## The enforcement is the struct, not the discipline
//!
//! `Node` has **no parent field**, and that is the whole reason inheritance,
//! descendant selectors and `%` are impossible rather than merely forbidden. A
//! node knows its children -- they are part of what it *is* -- and knows nothing
//! above or beside it. L7: *the father does not know he has brothers*.
//!
//! Anyone tempted later to add `parent: Option<&Node>` should read this first:
//! that one field silently unlocks every feature the contract rejects, and
//! nothing would fail to compile.

use crate::value::{self, Tag};
use crate::Node;
use bmo_maqueta_diag::{Error, Span};
use bmo_maqueta_lex::{Kind, Token};

pub fn span_of(t: &Token) -> Span {
    Span::new(t.start, t.len, t.line, t.col)
}

fn text_of(src: &[u8], t: &Token) -> Vec<u8> {
    t.text(src).to_vec()
}

/// Build the tree. Errors accumulate; parsing carries on wherever the shape
/// still makes sense, because five compilations for five typos is the thing a
/// compiler is supposed to spare you.
pub fn parse(src: &[u8], toks: &[Token], errors: &mut Vec<Error>) -> Option<Node> {
    let mut stack: Vec<Node> = Vec::new();
    let mut roots: Vec<Node> = Vec::new();
    let mut i = 0usize;

    while i < toks.len() {
        let t = &toks[i];
        match t.kind {
            Kind::Lt => {
                i += 1;
                if let Some((node, self_closed)) = open_tag(src, toks, &mut i, errors) {
                    if self_closed {
                        attach(&mut stack, &mut roots, node, errors);
                    } else {
                        stack.push(node);
                    }
                }
            }
            Kind::LtSlash => {
                i += 1;
                close_tag(src, toks, &mut i, &mut stack, &mut roots, errors);
            }
            Kind::Text => {
                let raw = text_of(src, t);
                if !raw.iter().all(|b| b.is_ascii_whitespace()) {
                    add_text(&mut stack, &raw, span_of(t), errors);
                }
                i += 1;
            }
            Kind::NonAscii => {
                errors.push(non_ascii(span_of(t)));
                i += 1;
            }
            Kind::StyleOpen | Kind::StyleClose => {
                // lib.rs lifts the style block out before we get here; seeing
                // one means there were two, and that error is reported there.
                i += 1;
            }
            _ => {
                errors.push(Error::new(
                    span_of(t),
                    &format!("aqui no puede ir {}", t.kind.name()),
                    "fuera de una etiqueta solo hay texto y otras etiquetas.",
                    "revisar si falta un `<` o sobra un caracter.",
                ));
                i += 1;
            }
        }
    }

    for open in stack.iter().rev() {
        errors.push(Error::new(
            open.span,
            &format!("`<{}>` se abrio y no se cerro", open.tag.name()),
            "MAQUETA no cierra etiquetas por su cuenta. Un navegador si, y esa \
             reparacion silenciosa es exactamente lo que L7 prohibe aqui: obligaria \
             al lexer a consultar el arbol.",
            &format!("escribir `</{}>` donde corresponda.", open.tag.name()),
        ));
    }

    if roots.is_empty() {
        return None;
    }
    if roots.len() > 1 {
        for extra in &roots[1..] {
            errors.push(Error::new(
                extra.span,
                "sobra una etiqueta en la raiz",
                "un fichero es UN componente, asi que tiene una sola raiz.",
                "envolver todo en el `<maqueta>` de arriba.",
            ));
        }
    }
    let root = roots.remove(0);
    if root.tag != Tag::Maqueta {
        errors.push(Error::new(
            root.span,
            &format!("la raiz es `<{}>` y tiene que ser `<maqueta>`", root.tag.name()),
            "`<maqueta>` es lo que declara el lienzo, y sin el no hay contra que \
             medir si algo se sale.",
            "envolver el contenido en `<maqueta> ... </maqueta>`.",
        ));
    }
    Some(root)
}

/// `<name attr="v" ...>` or `.../>`. Returns the node and whether it closed
/// itself. The cursor is left after the `>`.
fn open_tag(
    src: &[u8],
    toks: &[Token],
    i: &mut usize,
    errors: &mut Vec<Error>,
) -> Option<(Node, bool)> {
    let name_tok = toks.get(*i)?;
    if name_tok.kind != Kind::Ident {
        errors.push(Error::new(
            span_of(name_tok),
            "falta el nombre de la etiqueta",
            "despues de `<` va un nombre.",
            "por ejemplo `<div>`.",
        ));
        return None;
    }
    let raw = text_of(src, name_tok);
    let tag = match Tag::from_name(&raw) {
        Some(t) => t,
        None => {
            errors.push(value::unknown_tag(span_of(name_tok), &raw));
            // Keep walking to the `>` so one bad tag does not cascade.
            skip_to_tag_end(toks, i);
            return None;
        }
    };
    *i += 1;

    let mut node = Node::new(tag, span_of(name_tok));
    while let Some(t) = toks.get(*i) {
        match t.kind {
            Kind::Gt => {
                *i += 1;
                return Some((node, false));
            }
            Kind::SlashGt => {
                *i += 1;
                return Some((node, true));
            }
            Kind::Ident => {
                attribute(src, toks, i, &mut node, errors);
            }
            _ => {
                errors.push(Error::new(
                    span_of(t),
                    &format!("dentro de la etiqueta no puede ir {}", t.kind.name()),
                    "dentro de `<...>` solo hay nombres de atributo, `=` y valores \
                     entre comillas.",
                    "revisar las comillas del atributo anterior.",
                ));
                *i += 1;
            }
        }
    }
    errors.push(Error::new(
        node.span,
        &format!("`<{}` se quedo sin cerrar el `>`", tag.name()),
        "la etiqueta empieza y el fichero se acaba.",
        "cerrar con `>` o con `/>`.",
    ));
    Some((node, true))
}

fn attribute(
    src: &[u8],
    toks: &[Token],
    i: &mut usize,
    node: &mut Node,
    errors: &mut Vec<Error>,
) {
    let name_tok = toks[*i];
    let name = text_of(src, &name_tok);
    *i += 1;

    if toks.get(*i).map(|t| t.kind) != Some(Kind::Eq) {
        errors.push(Error::new(
            span_of(&name_tok),
            &format!(
                "el atributo `{}` no tiene valor",
                String::from_utf8_lossy(&name)
            ),
            "en MAQUETA todo atributo lleva valor. Los atributos sueltos son una \
             comodidad de HTML que aqui solo serviria para escribir erratas que \
             compilan.",
            "escribirlo como `nombre=\"valor\"`.",
        ));
        return;
    }
    *i += 1;

    let val_tok = match toks.get(*i) {
        Some(t) if t.kind == Kind::Str => *t,
        Some(t) => {
            errors.push(Error::new(
                span_of(t),
                "el valor del atributo tiene que ir entre comillas",
                "sin comillas no se sabe donde acaba el valor.",
                "por ejemplo `class=\"pad\"`.",
            ));
            *i += 1;
            return;
        }
        None => return,
    };
    let val = text_of(src, &val_tok);
    let vspan = span_of(&val_tok);
    *i += 1;

    match name.as_slice() {
        b"class" => {
            for word in val.split(|b| b.is_ascii_whitespace()).filter(|w| !w.is_empty()) {
                if !is_name(word) {
                    errors.push(bad_name(vspan, word, "una clase"));
                    continue;
                }
                node.classes.push(String::from_utf8_lossy(word).into_owned());
            }
        }
        b"id" => {
            if !is_name(&val) {
                errors.push(bad_name(vspan, &val, "un id"));
            } else {
                node.id = Some(String::from_utf8_lossy(&val).into_owned());
            }
        }
        b"nombre" => {
            if node.tag != Tag::Island {
                errors.push(Error::new(
                    span_of(&name_tok),
                    "`nombre` es solo de `<island>`",
                    "el nombre es como el proceso de fuera encuentra su rect. En \
                     cualquier otra caja no lo lee nadie.",
                    "usar `id` si lo que hace falta es la tabla de golpeo.",
                ));
            } else if !is_name(&val) {
                errors.push(bad_name(vspan, &val, "el nombre de una isla"));
            } else {
                node.island = Some(String::from_utf8_lossy(&val).into_owned());
            }
        }
        b"ancho" | b"alto" => {
            if node.tag != Tag::Maqueta {
                errors.push(Error::new(
                    span_of(&name_tok),
                    &format!(
                        "`{}` es solo de `<maqueta>`",
                        String::from_utf8_lossy(&name)
                    ),
                    "el tamano de una caja se declara en el estilo, no en el marcado: \
                     mezclarlos daria dos sitios donde buscar el mismo numero.",
                    "`width` y `height` en el bloque `<style>`.",
                ));
            } else {
                match parse_u32(&val) {
                    Some(n) if name == b"ancho" => node.width = Some(n),
                    Some(n) => node.height = Some(n),
                    None => errors.push(Error::new(
                        vspan,
                        "esto no es un numero de pixeles",
                        "`ancho` y `alto` son enteros, sin unidad y sin signo.",
                        "por ejemplo `ancho=\"322\"`.",
                    )),
                }
            }
        }
        other => {
            errors.push(Error::new(
                span_of(&name_tok),
                &format!(
                    "atributo no soportado -- `{}`",
                    String::from_utf8_lossy(other)
                ),
                "la lista de atributos esta CERRADA. Un atributo que se acepta y no \
                 se lee es una linea que parece hacer algo y no hace nada.",
                "`class`, `id`, `nombre` (solo en `<island>`) y `ancho`/`alto` \
                 (solo en `<maqueta>`).",
            ));
        }
    }
}

fn close_tag(
    src: &[u8],
    toks: &[Token],
    i: &mut usize,
    stack: &mut Vec<Node>,
    roots: &mut Vec<Node>,
    errors: &mut Vec<Error>,
) {
    let name_tok = match toks.get(*i) {
        Some(t) if t.kind == Kind::Ident => *t,
        _ => {
            errors.push(Error::new(
                toks.get(*i).map(span_of).unwrap_or(Span::new(0, 0, 1, 1)),
                "falta el nombre en la etiqueta de cierre",
                "despues de `</` va el nombre de lo que se cierra.",
                "por ejemplo `</div>`.",
            ));
            return;
        }
    };
    let raw = text_of(src, &name_tok);
    *i += 1;
    if toks.get(*i).map(|t| t.kind) == Some(Kind::Gt) {
        *i += 1;
    }

    let open = match stack.pop() {
        Some(n) => n,
        None => {
            errors.push(Error::new(
                span_of(&name_tok),
                &format!("`</{}>` cierra algo que no esta abierto", String::from_utf8_lossy(&raw)),
                "no hay ninguna etiqueta abierta en este punto.",
                "borrar el cierre, o abrir la etiqueta antes.",
            ));
            return;
        }
    };
    if Tag::from_name(&raw) != Some(open.tag) {
        errors.push(Error::new(
            span_of(&name_tok),
            &format!(
                "se cierra `</{}>` pero lo abierto es `<{}>`",
                String::from_utf8_lossy(&raw),
                open.tag.name()
            ),
            "MAQUETA no reordena etiquetas mal anidadas. Un navegador si, y hacerlo \
             obliga al lexer a mirar el arbol -- lo que L7a prohibe.",
            &format!("cerrar `</{}>` aqui.", open.tag.name()),
        ));
    }
    attach(stack, roots, open, errors);
}

/// Hang a finished node on its parent, or on the root list if there is none.
fn attach(stack: &mut [Node], roots: &mut Vec<Node>, node: Node, errors: &mut Vec<Error>) {
    match stack.last_mut() {
        Some(parent) => {
            if !parent.tag.takes_boxes() {
                errors.push(Error::new(
                    node.span,
                    &format!("`<{}>` no puede llevar cajas dentro", parent.tag.name()),
                    match parent.tag {
                        Tag::Island => {
                            "una isla la rellena OTRO proceso: lo que hubiera dentro \
                             no lo pintaria nadie."
                        }
                        _ => {
                            "un `<span>` lleva texto. Meter cajas dentro es flujo en \
                             linea, y eso no esta implementado."
                        }
                    },
                    "sacar la caja fuera.",
                ));
                return;
            }
            if parent.text.is_some() {
                errors.push(mixed(node.span));
                return;
            }
            parent.children.push(node);
        }
        None => roots.push(node),
    }
}

fn add_text(stack: &mut [Node], raw: &[u8], span: Span, errors: &mut Vec<Error>) {
    let node = match stack.last_mut() {
        Some(n) => n,
        None => {
            errors.push(Error::new(
                span,
                "hay texto fuera de la raiz",
                "todo lo que se pinta va dentro de `<maqueta>`.",
                "meter el texto en un `<div>` o un `<span>`.",
            ));
            return;
        }
    };
    if !node.children.is_empty() {
        errors.push(mixed(span));
        return;
    }
    if node.tag == Tag::Island {
        errors.push(Error::new(
            span,
            "una isla no lleva texto",
            "la rellena otro proceso; lo que se escriba aqui no lo pinta nadie.",
            "dejarla vacia: `<island nombre=\"...\"></island>`.",
        ));
        return;
    }
    let s = String::from_utf8_lossy(raw).trim().to_string();
    match &mut node.text {
        Some(t) => {
            t.push(' ');
            t.push_str(&s);
        }
        None => node.text = Some(s),
    }
}

fn mixed(span: Span) -> Error {
    Error::new(
        span,
        "no se pueden mezclar texto y cajas en la misma etiqueta",
        "mezclarlos es flujo en linea, que es la parte cara de un motor de \
         maquetacion y no esta implementada. Aceptarlo a medias daria un texto \
         colocado de una forma que nadie escribio.",
        "meter el texto en su propio `<span>`, hermano de las cajas.",
    )
}

fn non_ascii(span: Span) -> Error {
    Error::new(
        span,
        "byte fuera de ASCII",
        "las fuentes de BMO-X son ASCII, y no por estetica: una sola letra acentuada \
         en un literal llego a hacer crecer un `.bex` de 512 bytes a 492.032. Las \
         cadenas de pantalla son espanol SIN tilde.",
        "escribir el texto sin tildes ni enes con virgulilla.",
    )
}

fn bad_name(span: Span, raw: &[u8], what: &str) -> Error {
    Error::new(
        span,
        &format!("`{}` no vale como {what}", String::from_utf8_lossy(raw)),
        "un nombre empieza por letra o `_` y sigue con letras, cifras, `-` o `_`.",
        "por ejemplo `tecla-op`.",
    )
}

fn skip_to_tag_end(toks: &[Token], i: &mut usize) {
    while let Some(t) = toks.get(*i) {
        *i += 1;
        if t.kind == Kind::Gt || t.kind == Kind::SlashGt {
            return;
        }
    }
}

fn is_name(b: &[u8]) -> bool {
    !b.is_empty()
        && (b[0].is_ascii_alphabetic() || b[0] == b'_')
        && b.iter().all(|&c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
}

fn parse_u32(b: &[u8]) -> Option<u32> {
    if b.is_empty() || !b.iter().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let mut n: u32 = 0;
    for &c in b {
        n = n.checked_mul(10)?.checked_add((c - b'0') as u32)?;
    }
    Some(n)
}
