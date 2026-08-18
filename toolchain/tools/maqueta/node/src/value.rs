//! **The closed lists, as types.**
//!
//! This file is the contract of `LA_MAQUETA_EXIGE.md` sections 2, 3 and 4
//! turned into Rust enums, and that is not a formality:
//!
//! > There is no `Tag` value for `h1`. There is no `Prop` value for
//! > `box-shadow`. **The father cannot represent them.**
//!
//! So rejecting them is not an opinion this generation holds -- it is a naming
//! failure, which is structural. Opinions live in `verdict/`. The difference is
//! real: the father rejects the *unnameable*, the great-grandson rejects the
//! *unwise* (text that does not fit, a box that escapes its parent).

use bmo_maqueta_diag::{Error, Span};

/// The five tags. `<style>` is not here: the lexer eats it and its contents
/// become rules, so it never reaches the tree.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tag {
    Maqueta,
    Div,
    Span,
    Island,
}

impl Tag {
    pub fn from_name(b: &[u8]) -> Option<Tag> {
        match b {
            b"maqueta" => Some(Tag::Maqueta),
            b"div" => Some(Tag::Div),
            b"span" => Some(Tag::Span),
            b"island" => Some(Tag::Island),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Tag::Maqueta => "maqueta",
            Tag::Div => "div",
            Tag::Span => "span",
            Tag::Island => "island",
        }
    }

    /// An island is filled by another process, so nothing of ours goes inside.
    /// A span holds text and no boxes. See `LA_MAQUETA_EXIGE.md` section 2.
    pub fn takes_boxes(self) -> bool {
        matches!(self, Tag::Maqueta | Tag::Div)
    }
}

/// The seventeen properties. Counted from what `scene/` actually does, not from
/// what CSS offers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Prop {
    // the box
    Width,
    Height,
    Padding,
    Margin,
    // the paint
    BackgroundColor,
    Color,
    BorderWidth,
    BorderColor,
    BorderRadius,
    // the placement
    Display,
    FlexDirection,
    Gap,
    JustifyContent,
    AlignItems,
    // the absolute placement
    Position,
    Left,
    Top,
}

impl Prop {
    pub fn from_name(b: &[u8]) -> Option<Prop> {
        Some(match b {
            b"width" => Prop::Width,
            b"height" => Prop::Height,
            b"padding" => Prop::Padding,
            b"margin" => Prop::Margin,
            b"background-color" => Prop::BackgroundColor,
            b"color" => Prop::Color,
            b"border-width" => Prop::BorderWidth,
            b"border-color" => Prop::BorderColor,
            b"border-radius" => Prop::BorderRadius,
            b"display" => Prop::Display,
            b"flex-direction" => Prop::FlexDirection,
            b"gap" => Prop::Gap,
            b"justify-content" => Prop::JustifyContent,
            b"align-items" => Prop::AlignItems,
            b"position" => Prop::Position,
            b"left" => Prop::Left,
            b"top" => Prop::Top,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Prop::Width => "width",
            Prop::Height => "height",
            Prop::Padding => "padding",
            Prop::Margin => "margin",
            Prop::BackgroundColor => "background-color",
            Prop::Color => "color",
            Prop::BorderWidth => "border-width",
            Prop::BorderColor => "border-color",
            Prop::BorderRadius => "border-radius",
            Prop::Display => "display",
            Prop::FlexDirection => "flex-direction",
            Prop::Gap => "gap",
            Prop::JustifyContent => "justify-content",
            Prop::AlignItems => "align-items",
            Prop::Position => "position",
            Prop::Left => "left",
            Prop::Top => "top",
        }
    }

    /// What shape of value this property accepts. Knowing that is naming, which
    /// is this generation's whole job.
    pub fn shape(self) -> Shape {
        match self {
            Prop::Width
            | Prop::Height
            | Prop::BorderWidth
            | Prop::BorderRadius
            | Prop::Gap
            | Prop::Left
            | Prop::Top => Shape::OnePx,
            Prop::Padding | Prop::Margin => Shape::OneOrFourPx,
            Prop::BackgroundColor | Prop::Color | Prop::BorderColor => Shape::Color,
            Prop::Display => Shape::Words(&[Keyword::Block, Keyword::Flex]),
            Prop::FlexDirection => Shape::Words(&[Keyword::Row, Keyword::Column]),
            Prop::JustifyContent => Shape::Words(&[
                Keyword::Start,
                Keyword::Center,
                Keyword::End,
                Keyword::SpaceBetween,
            ]),
            Prop::AlignItems => {
                Shape::Words(&[Keyword::Start, Keyword::Center, Keyword::End])
            }
            Prop::Position => Shape::Words(&[Keyword::Absolute]),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shape {
    OnePx,
    OneOrFourPx,
    Color,
    Words(&'static [Keyword]),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Keyword {
    Block,
    Flex,
    Row,
    Column,
    Start,
    Center,
    End,
    SpaceBetween,
    Absolute,
}

impl Keyword {
    pub fn from_name(b: &[u8]) -> Option<Keyword> {
        Some(match b {
            b"block" => Keyword::Block,
            b"flex" => Keyword::Flex,
            b"row" => Keyword::Row,
            b"column" => Keyword::Column,
            b"start" => Keyword::Start,
            b"center" => Keyword::Center,
            b"end" => Keyword::End,
            b"space-between" => Keyword::SpaceBetween,
            b"absolute" => Keyword::Absolute,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Keyword::Block => "block",
            Keyword::Flex => "flex",
            Keyword::Row => "row",
            Keyword::Column => "column",
            Keyword::Start => "start",
            Keyword::Center => "center",
            Keyword::End => "end",
            Keyword::SpaceBetween => "space-between",
            Keyword::Absolute => "absolute",
        }
    }
}

/// A resolved value. Everything is an integer number of pixels or a packed
/// `0x00RRGGBB`, because that is what BMO-X draws with.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Value {
    Px(u32),
    /// top, right, bottom, left -- CSS order, so the browser preview agrees.
    Px4([u32; 4]),
    Color(u32),
    Word(Keyword),
}

// ------------------------------------------------------------------------
//  The rejection table
// ------------------------------------------------------------------------

/// Real CSS that MAQUETA does not have, each with **why** and **instead**.
///
/// ★ This table is the difference between a compiler and a toy. "unknown
/// property `box-shadow`" tells the author nothing; naming the reason and the
/// way out is what makes rejecting better than ignoring. Without it, the
/// inversion this project is built on -- *a compiler rejects what it does not
/// understand* -- just leaves people stuck.
pub fn known_rejection(name: &[u8]) -> Option<(&'static str, &'static str)> {
    Some(match name {
        b"box-shadow" => (
            "una sombra necesita mezcla alfa, y el rasterizador esta en el escalon 2 \
             (triangulo). La mezcla es el escalon 4.",
            "`scene/mod.rs` pinta las sombras de ventana con dos capas de color \
             solido. Si hace falta aqui, se declaran con dos `<div>`.",
        ),
        b"opacity" | b"filter" | b"backdrop-filter" => (
            "no hay mezcla alfa: el pixel de BMO-X es `u32` en `0x00RRGGBB` y el \
             canal alto no se lee.",
            "elegir el color ya mezclado, o esperar al escalon 4 del rasterizador.",
        ),
        b"z-index" => (
            "no hay capas: las cajas se pintan en el orden en que estan escritas.",
            "mover la caja en el fichero. El orden del texto ES el orden de pintado.",
        ),
        b"overflow" | b"overflow-x" | b"overflow-y" => (
            "nada se recorta ni se desplaza: una caja que no cabe es un ERROR del \
             veredicto, no un caso a manejar en ejecucion.",
            "dar sitio a la caja, o repartir con `display:flex` y `gap`.",
        ),
        b"float" | b"clear" => (
            "el flotado existe para rodear texto con imagenes, y aqui no hay ninguna \
             de las dos cosas.",
            "`display:flex` con `flex-direction:row`.",
        ),
        b"font-family" | b"font-size" | b"font-weight" | b"font" | b"line-height" => (
            "hay UNA fuente, de mapa de bits y ancho fijo. Que ese sea el caso es lo \
             que hace posible medir texto al compilar (`len * GLIFO_ANCHO`), asi que \
             no es una carencia: es el cimiento.",
            "nada. El tamano del texto no se elige.",
        ),
        b"text-align" => (
            "no esta implementada, y no es gratis: alinear texto es colocar una caja \
             dentro de otra, o sea maquetacion.",
            "meter el texto en su `<span>` y colocarlo con `justify-content`.",
        ),
        b"transition" | b"animation" | b"transform" => (
            "MAQUETA compila una imagen QUIETA. Lo que se mueve es codigo, y esa \
             frontera es lo que impide que esto acabe siendo un navegador.",
            "Rust, en el bucle de fotograma del compositor.",
        ),
        b"border" => (
            "el atajo mezcla grosor, estilo y color, y de los tres solo existen dos.",
            "`border-width` y `border-color`, por separado.",
        ),
        b"grid" | b"grid-template-columns" | b"grid-template-rows" | b"grid-area" => (
            "`grid` no esta: `flex` cubre todo lo que hace `scene/` hoy, contado.",
            "`display:flex` anidado. Una rejilla de N columnas son N `<div>` en una \
             fila, y filas en una columna.",
        ),
        b"inherit" | b"initial" | b"unset" => (
            "no hay herencia: en MAQUETA una pieza no sabe que tiene padre (L7), y \
             eso es lo que mantiene al padre sin conocer a sus ancestros.",
            "declarar el valor donde hace falta. Con estilos de ambito corto, \
             repetirlo cuesta menos que la regla que lo evitaba.",
        ),
        _ => return None,
    })
}

// ------------------------------------------------------------------------
//  Errors this file produces
// ------------------------------------------------------------------------

pub fn unknown_tag(span: Span, name: &[u8]) -> Error {
    let n = String::from_utf8_lossy(name).into_owned();
    let promises: &[&str] = &[
        "h1", "h2", "h3", "p", "button", "a", "ul", "li", "table", "img", "input", "form",
    ];
    if promises.contains(&n.as_str()) {
        return Error::new(
            span,
            &format!("etiqueta no soportada -- `<{n}>`"),
            "esa etiqueta PROMETE semantica que MAQUETA no tiene (papel, foco, \
             navegacion, tipografia). Aceptarla y no honrarla seria la mentira que \
             este compilador existe para evitar.",
            "`<div>` o `<span>`: son las dos unicas etiquetas de HTML que no \
             prometen nada, que es justo lo que hay aqui.",
        );
    }
    Error::new(
        span,
        &format!("etiqueta no soportada -- `<{n}>`"),
        "la lista de etiquetas esta CERRADA, y lo que no esta en ella no compila.",
        "`<maqueta>`, `<div>`, `<span>`, `<island>` o `<style>`. La lista entera \
         esta en la seccion 2 de `LA_MAQUETA_EXIGE.md`.",
    )
}

pub fn unknown_prop(span: Span, name: &[u8]) -> Error {
    let n = String::from_utf8_lossy(name).into_owned();
    if let Some((why, instead)) = known_rejection(name) {
        return Error::new(span, &format!("propiedad no soportada -- `{n}`"), why, instead);
    }
    Error::new(
        span,
        &format!("propiedad no soportada -- `{n}`"),
        "la lista de propiedades esta CERRADA: diecisiete, contadas sobre lo que el \
         escritorio hace de verdad hoy.",
        "la lista entera esta en la seccion 3 de `LA_MAQUETA_EXIGE.md`. Anadir una \
         empieza por anadirla ahi.",
    )
}
