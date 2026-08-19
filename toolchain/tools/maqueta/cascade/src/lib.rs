//! # MAQUETA -- CASCADE, the son generation
//!
//! generacion: hijo
//!
//! **Relates a `Node` to a `Rule`** and produces a box whose style is settled.
//! L7: *the son relates two of the father's, and does not know what the relation
//! means.* This one does not know what a screen is, what a pixel is worth, or
//! that anything will ever be drawn.
//!
//! ## What proves the cascade actually happened
//!
//! `Styled` has **no `classes` field**. Classes are the question; the style is
//! the answer, and the answer replaces the question. If classes survived this
//! generation, something downstream could re-match them -- and then "which rule
//! wins" would have two implementations.
//!
//! ## The two decisions that fell out of writing it
//!
//! 1. **Absence is not `auto`.** An undeclared `width` comes out as `None`, not
//!    as a number this generation invented. What an unsaid width becomes depends
//!    on the boxes around it, and those are the grandson's business.
//! 2. **There is no default text colour.** A default would be inheritance from
//!    nowhere. See `style.rs`.
//!
//! ## Errors versus findings, and the line between them
//!
//! The **ordering guardian** is an error: with the rules in the wrong order this
//! generation cannot compute faithfully (`guard.rs`).
//!
//! A rule that matches nothing, or a class nobody defines, is **not** an error
//! here. The cascade computes fine; whether a dead rule is a typo or a
//! work in progress is a judgement, and judgements belong to `verdict/`. So they
//! come out as **facts alongside the tree** -- which is also the honest shape,
//! because this generation is the only one that ever knew.

#![forbid(unsafe_code)]

pub mod guard;
pub mod style;

use bmo_maqueta_diag::{Error, Span};
use bmo_maqueta_node::{Document, Node, Selector, Tag};

pub use style::{Align, Direction, Display, Justify, Position, Style};

/// A box whose style is settled. **No classes** -- see the module header.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Styled {
    pub tag: Tag,
    /// The hit-table key, carried through untouched.
    pub id: Option<String>,
    pub island: Option<String>,
    pub text: Option<String>,
    /// Only ever set on the root.
    pub canvas: Option<(u32, u32)>,
    pub style: Style,
    /// El estilo mientras el puntero esta encima, si alguna regla `:hover` toco
    /// esta caja. `None` = esta caja no reacciona.
    ///
    /// * Es un estilo COMPLETO y no un parche: el consumidor no tiene que
    /// componer nada, solo elegir cual de los dos usa. Y por construccion los dos
    /// dan la MISMA geometria -- el padre no deja que una regla `:hover` toque
    /// nada que no sea pintura.
    pub hover: Option<Style>,
    pub children: Vec<Styled>,
    pub span: Span,
}

/// Something the cascade noticed while matching, that only it could know.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Finding {
    pub span: Span,
    pub what: String,
}

/// The tree with its styles settled, plus what matching revealed.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Cascaded {
    pub root: Styled,
    /// Rules that matched no box at all.
    pub dead_rules: Vec<Finding>,
    /// Classes worn by a box that no rule ever mentions.
    pub orphan_classes: Vec<Finding>,
}

/// Settle every box's style.
pub fn cascade(doc: &Document) -> Result<Cascaded, Vec<Error>> {
    let mut errors = Vec::new();
    guard::check(&doc.rules, &mut errors);
    if !errors.is_empty() {
        return Err(errors);
    }

    let mut used = vec![false; doc.rules.len()];
    let mut defined: Vec<&str> = Vec::new();
    for r in &doc.rules {
        for s in &r.selectors {
            if let Selector::Class(c) = s {
                defined.push(c);
            }
        }
    }

    let mut orphan_classes = Vec::new();
    let root = settle(&doc.root, doc, &mut used, &defined, &mut orphan_classes);

    let dead_rules = doc
        .rules
        .iter()
        .zip(&used)
        .filter(|(_, u)| !**u)
        .map(|(r, _)| Finding {
            span: r.span,
            what: guard::describe(r),
        })
        .collect();

    Ok(Cascaded {
        root,
        dead_rules,
        orphan_classes,
    })
}

fn settle(
    node: &Node,
    doc: &Document,
    used: &mut [bool],
    defined: &[&str],
    orphans: &mut Vec<Finding>,
) -> Styled {
    let mut style = Style::default();
    let mut hover: Option<Style> = None;

    // In file order, and every match overwrites the last. That is the entire
    // cascade -- and `guard.rs` is what makes so short a rule safe.
    for (k, rule) in doc.rules.iter().enumerate() {
        if !rule.selectors.iter().any(|s| matches(s, node)) {
            continue;
        }
        used[k] = true;
        if rule.hover {
            // El estado "encima" parte del de reposo: `guard.rs` garantiza que
            // para cuando llega un `:hover`, el reposo ya esta entero.
            let h = hover.get_or_insert(style);
            for d in &rule.decls {
                h.apply(d);
            }
        } else {
            for d in &rule.decls {
                style.apply(d);
            }
            if let Some(h) = hover.as_mut() {
                for d in &rule.decls {
                    h.apply(d);
                }
            }
        }
    }

    for c in &node.classes {
        if !defined.contains(&c.as_str()) {
            orphans.push(Finding {
                span: node.span,
                what: c.clone(),
            });
        }
    }

    Styled {
        tag: node.tag,
        id: node.id.clone(),
        island: node.island.clone(),
        text: node.text.clone(),
        canvas: match (node.width, node.height) {
            (Some(w), Some(h)) => Some((w, h)),
            _ => None,
        },
        style,
        hover,
        children: node
            .children
            .iter()
            .map(|c| settle(c, doc, used, defined, orphans))
            .collect(),
        span: node.span,
    }
}

fn matches(s: &Selector, node: &Node) -> bool {
    match s {
        Selector::Tag(t) => *t == node.tag,
        Selector::Class(c) => node.classes.iter().any(|k| k == c),
    }
}
