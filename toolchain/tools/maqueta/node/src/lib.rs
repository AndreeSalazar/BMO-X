//! # MAQUETA -- NODE, the father generation
//!
//! Tokens in; **named pieces** out. A `Node` is a box that knows what it is; a
//! `Rule` is a rule that knows what it says. Neither knows the other exists.
//!
//! ## What this generation may and may not know (L7)
//!
//! > *the father names it and composes it -- **he does not know he has
//! > brothers***
//!
//! A node knows its children, because they are part of what it *is*. It does
//! **not** know its parent or its siblings, and `Node` has no field for either.
//! That is not a style choice; it is the enforcement:
//!
//! | what CSS wants | what it would need | verdict |
//! |---|---|---|
//! | inheritance (`color` from the parent) | a parent pointer | impossible |
//! | descendant selectors (`.a .b`) | the ancestor chain | impossible |
//! | `%`, `auto` | the container's size | impossible |
//!
//! **The data structure decides the feature set.** Add `parent: Option<..>` and
//! all three become available with nothing failing to compile -- which is why
//! the absence is written down here rather than merely observed.
//!
//! ## What this generation rejects, and what it does not
//!
//! It rejects the **unnameable**: there is no `Tag` for `h1`, no `Prop` for
//! `box-shadow`, no unit but `px`. That is a naming failure, structural.
//!
//! It does *not* reject the **unwise** -- text that does not fit its box, a
//! child that escapes its parent, two rules in the wrong order. Those are
//! opinions about a finished layout, and they belong to `verdict/`.

#![forbid(unsafe_code)]

pub mod markup;
pub mod style;
pub mod value;

use bmo_maqueta_diag::{Error, Span};
use bmo_maqueta_lex::{lex, Kind, Token};

pub use value::{Keyword, Prop, Shape, Tag, Value};

/// One box, named. **No parent, no siblings** -- see the module header.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Node {
    pub tag: Tag,
    pub classes: Vec<String>,
    /// The hit-table key. Never a styling hook -- see `style::selector`.
    pub id: Option<String>,
    /// `<island nombre="...">`: how the process outside finds its rect.
    pub island: Option<String>,
    /// Only ever set on `<maqueta>`. Absent means "the compiler works it out".
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// Text content. A node has text or children, never both.
    pub text: Option<String>,
    pub children: Vec<Node>,
    pub span: Span,
}

impl Node {
    pub fn new(tag: Tag, span: Span) -> Self {
        Self {
            tag,
            classes: Vec::new(),
            id: None,
            island: None,
            width: None,
            height: None,
            text: None,
            children: Vec::new(),
            span,
        }
    }
}

/// One declaration, named and with its value resolved to integers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Decl {
    pub prop: Prop,
    pub value: Value,
    pub span: Span,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Selector {
    Tag(Tag),
    Class(String),
}

/// One rule. **Does not know that other rules exist** -- which one wins is a
/// relation between two, so it lives in `cascade/`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Rule {
    pub selectors: Vec<Selector>,
    pub decls: Vec<Decl>,
    pub span: Span,
    /// `.tecla:hover { ... }` -- applies only while the pointer is over the box.
    ///
    /// * It is a property of the RULE and not of each selector because a rule
    /// where half the selectors hover and half do not has no single meaning, and
    /// a meaning that has to be explained twice is one this compiler refuses.
    pub hover: bool,
}

/// What a `.maqueta` file is, once named: a tree and a list of rules, side by
/// side and still unrelated.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Document {
    pub root: Node,
    pub rules: Vec<Rule>,
}

/// Name everything in a source file.
///
/// Every error is collected, never just the first: five typos should cost one
/// compilation, not five.
pub fn parse(src: &[u8]) -> Result<Document, Vec<Error>> {
    let toks = lex(src);
    let mut errors = Vec::new();
    let (markup_toks, style_toks) = split(&toks, &mut errors);

    let rules = style::parse(src, &style_toks, &mut errors);
    let root = markup::parse(src, &markup_toks, &mut errors);

    match root {
        Some(root) if errors.is_empty() => Ok(Document { root, rules }),
        Some(_) => Err(errors),
        None => {
            if errors.is_empty() {
                errors.push(Error::new(
                    Span::new(0, 0, 1, 1),
                    "el fichero no tiene nada que maquetar",
                    "hace falta al menos un `<maqueta>` con algo dentro.",
                    "empezar por `<maqueta> ... </maqueta>`.",
                ));
            }
            Err(errors)
        }
    }
}

/// Cut the token stream in two: what is markup and what is style.
///
/// The lexer already told us where the boundary is (`StyleOpen`/`StyleClose`
/// are literal byte matches, not a judgement about the tree), so this is a
/// split and not a parse.
fn split(toks: &[Token], errors: &mut Vec<Error>) -> (Vec<Token>, Vec<Token>) {
    let mut markup = Vec::new();
    let mut styles = Vec::new();
    let mut seen = 0u32;
    let mut inside = false;

    for t in toks {
        match t.kind {
            Kind::StyleOpen => {
                seen += 1;
                inside = true;
                if seen == 2 {
                    errors.push(Error::new(
                        markup::span_of(t),
                        "solo puede haber un bloque `<style>`",
                        "con dos, cual gana depende del orden en que se lean -- que es \
                         justo la clase de regla invisible que MAQUETA existe para no \
                         tener.",
                        "juntar las reglas en un solo bloque.",
                    ));
                }
            }
            Kind::StyleClose => inside = false,
            _ if inside => styles.push(*t),
            _ => markup.push(*t),
        }
    }
    (markup, styles)
}
