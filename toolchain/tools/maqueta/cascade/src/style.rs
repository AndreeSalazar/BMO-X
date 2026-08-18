//! **What a resolved style is, and how one declaration changes it.**
//!
//! ## Absence is not `auto`
//!
//! `width` comes out as `Option<u32>`, and `None` does **not** mean the CSS
//! keyword `auto` -- that is still refused by the father. It means *this file
//! did not say*, and deciding what an unsaid width becomes is a question about
//! a box **next to other boxes**, which is the grandson's job (`layout/`).
//!
//! Resolving it here would be this generation reaching for something it cannot
//! see. Recording the absence is the honest move, and it is what lets the
//! grandson give a block box its parent's width and a flex item its content's
//! width without either rule living in two places.
//!
//! ## ★ There is no default text colour, and that is deliberate
//!
//! Without inheritance, a default `color` would be inheritance from nowhere: a
//! value nobody wrote, that looks intentional, and that ages into a lie exactly
//! like `INFO_ES_ESCRIBIBLE => 0` did. So `color` is `None` until someone says
//! it, and text that never got one is a finding for `verdict/`.
//!
//! It costs one word in the markup -- `class="ink"` -- and the palette it points
//! at is `tema/tema.maqueta`. That is the Arch bargain the whole project is
//! built on: nothing implicit, and the explicit thing is one readable line.

use bmo_maqueta_node::{Decl, Keyword, Prop, Value};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Display {
    #[default]
    Block,
    Flex,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Direction {
    #[default]
    Row,
    Column,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Justify {
    #[default]
    Start,
    Center,
    End,
    SpaceBetween,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Align {
    #[default]
    Start,
    Center,
    End,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Position {
    #[default]
    Static,
    Absolute,
}

/// Every one of the seventeen properties, resolved.
///
/// `Option` means *nobody said*; everything else carries the value CSS uses when
/// nobody says, so that the browser preview and MAQUETA start from the same
/// place.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Style {
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// top, right, bottom, left.
    pub padding: [u32; 4],
    pub margin: [u32; 4],
    /// `None` = paint nothing. A box that is only a container is legitimate.
    pub background: Option<u32>,
    /// `None` = nobody said. See the module header: there is no default.
    pub color: Option<u32>,
    pub border_width: u32,
    pub border_color: Option<u32>,
    pub border_radius: u32,
    pub display: Display,
    pub direction: Direction,
    pub gap: u32,
    pub justify: Justify,
    pub align: Align,
    pub position: Position,
    pub left: Option<u32>,
    pub top: Option<u32>,
}

impl Style {
    /// Fold one declaration in. Later calls overwrite earlier ones, which is the
    /// whole of MAQUETA's cascade: **last wins**. What makes that safe is the
    /// guardian in `guard.rs`.
    pub fn apply(&mut self, d: &Decl) {
        match (d.prop, d.value) {
            (Prop::Width, Value::Px(n)) => self.width = Some(n),
            (Prop::Height, Value::Px(n)) => self.height = Some(n),
            (Prop::Padding, Value::Px4(v)) => self.padding = v,
            (Prop::Margin, Value::Px4(v)) => self.margin = v,
            (Prop::BackgroundColor, Value::Color(c)) => self.background = Some(c),
            (Prop::Color, Value::Color(c)) => self.color = Some(c),
            (Prop::BorderWidth, Value::Px(n)) => self.border_width = n,
            (Prop::BorderColor, Value::Color(c)) => self.border_color = Some(c),
            (Prop::BorderRadius, Value::Px(n)) => self.border_radius = n,
            (Prop::Gap, Value::Px(n)) => self.gap = n,
            (Prop::Left, Value::Px(n)) => self.left = Some(n),
            (Prop::Top, Value::Px(n)) => self.top = Some(n),

            (Prop::Display, Value::Word(Keyword::Block)) => self.display = Display::Block,
            (Prop::Display, Value::Word(Keyword::Flex)) => self.display = Display::Flex,
            (Prop::FlexDirection, Value::Word(Keyword::Row)) => self.direction = Direction::Row,
            (Prop::FlexDirection, Value::Word(Keyword::Column)) => {
                self.direction = Direction::Column
            }
            (Prop::JustifyContent, Value::Word(k)) => {
                self.justify = match k {
                    Keyword::Center => Justify::Center,
                    Keyword::End => Justify::End,
                    Keyword::SpaceBetween => Justify::SpaceBetween,
                    _ => Justify::Start,
                }
            }
            (Prop::AlignItems, Value::Word(k)) => {
                self.align = match k {
                    Keyword::Center => Align::Center,
                    Keyword::End => Align::End,
                    _ => Align::Start,
                }
            }
            (Prop::Position, Value::Word(Keyword::Absolute)) => {
                self.position = Position::Absolute
            }

            // The father checked every shape before this generation saw it, so
            // no other pairing exists. Ignoring rather than panicking keeps a
            // bug in one generation from taking down the next -- and if one ever
            // arrives, `verdict/` sees a style that is simply missing a value.
            _ => {}
        }
    }
}
