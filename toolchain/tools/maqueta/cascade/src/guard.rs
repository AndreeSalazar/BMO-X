//! **Do the two readings of this stylesheet agree?**
//!
//! MAQUETA's cascade is one sentence: *last wins*. CSS's is not -- a browser
//! ranks by **specificity** first, and only breaks ties by order. Those two give
//! the same answer most of the time, and when they differ nothing crashes: the
//! file simply looks one way in the browser preview and another on the Ryzen.
//!
//! ★★ **That is the worst failure this project can have.** The whole reason the
//! tags are HTML's and the properties are CSS's is so a `.maqueta` can be opened
//! in a browser while you write it. A preview that lies is worse than no
//! preview, because it is trusted.
//!
//! ## The rule, and why it is enough
//!
//! Give every rule a level -- **1 if any of its selectors is a class, else 0** --
//! and require the levels to never go down.
//!
//! ```text
//!    div    { ... }        level 0
//!    span   { ... }        level 0
//!    .pad   { ... }        level 1
//!    .tecla { ... }        level 1     <- fine
//!
//!    .pad   { ... }        level 1
//!    div    { ... }        level 0     <- refused
//! ```
//!
//! It is enough, and the check is short:
//!
//! - **Two rules of the same level.** CSS ties on specificity and falls back to
//!   source order; MAQUETA uses source order. Same answer.
//! - **A class rule after a tag rule.** CSS prefers the class (0,1,0 over
//!   0,0,1); MAQUETA prefers the later one, which is the class. Same answer.
//! - **A tag rule after a class rule.** CSS still prefers the class; MAQUETA
//!   would prefer the tag. **Different answer** -- and this is what gets refused.
//!
//! A rule is scored by its **highest** selector, so `div, .a { }` counts as a
//! class rule. That is right for the same reason: a node reached through `.a`
//! carries the class score, and it is the highest score that a browser compares.
//!
//! ## Why this is an error here and not a finding for `verdict/`
//!
//! This generation reads a stylesheet in the wrong order and **cannot compute
//! faithfully** -- it would produce a tree that does not match the documented
//! contract. That is the same kind of failure as the father being unable to name
//! `<h1>`, not the same kind as text overflowing its box.

use bmo_maqueta_diag::Error;
use bmo_maqueta_node::{Rule, Selector};

/// 0 = etiqueta, 1 = clase, 2 = `:hover`.
///
/// ★ El nivel 2 sale gratis y no es capricho: en CSS `.tecla:hover` puntua
/// (0,2,0) y le gana a `.tecla` (0,1,0) este donde este, igual que una clase le
/// gana a una etiqueta. La misma regla --los niveles no bajan-- cubre los tres
/// sin una linea de forma nueva.
fn level(r: &Rule) -> u8 {
    if r.hover {
        return 2;
    }
    u8::from(r.selectors.iter().any(|s| matches!(s, Selector::Class(_))))
}

/// How a rule reads, for a message. Public because `lib.rs` names rules in its
/// findings too, and two spellings of the same rule in two messages is the kind
/// of small lie that makes people distrust the whole output.
pub fn describe(r: &Rule) -> String {
    let sufijo = if r.hover { ":hover" } else { "" };
    r.selectors
        .iter()
        .map(|s| match s {
            Selector::Tag(t) => format!("{}{sufijo}", t.name()),
            Selector::Class(c) => format!(".{c}{sufijo}"),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Refuse any stylesheet where "last wins" and "most specific wins" could
/// disagree.
pub fn check(rules: &[Rule], errors: &mut Vec<Error>) {
    // El nivel mas alto visto hasta aqui, y quien lo puso. Basta con eso: la
    // regla es que los niveles NO BAJEN.
    let mut techo: Option<(u8, String)> = None;

    for r in rules {
        let n = level(r);
        match &techo {
            Some((alto, quien)) if n < *alto => {
                errors.push(Error::new(
                    r.span,
                    &format!(
                        "`{}` va despues de `{}`, y tiene que ir antes",
                        describe(r),
                        quien
                    ),
                    "MAQUETA resuelve por ORDEN --gana la ultima-- y un navegador \
                     resuelve por ESPECIFICIDAD, donde una clase le gana a una \
                     etiqueta y `:hover` le gana a la clase, esten donde esten. Con \
                     este orden las dos lecturas dan resultados distintos, y entonces \
                     la previsualizacion en navegador MIENTE. Un boceto en el que no \
                     se puede confiar es peor que no tenerlo.",
                    "las reglas de etiqueta arriba, luego las de clase, y las de \
                     `:hover` al final. Con ese orden, `gana la ultima` y `gana la \
                     mas especifica` dan siempre lo mismo.",
                ));
            }
            _ => techo = Some((n, describe(r))),
        }
    }
}
