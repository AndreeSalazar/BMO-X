//! # MAQUETA -- LAYOUT, the grandson generation
//!
//! generacion: nieto
//!
//! **Relates a box to the boxes around it** and settles where each one lands.
//! L7: it does not know what is done with the numbers. There is no screen here,
//! no colour is read, nothing is painted, and no opinion is held about whether
//! the result is any good -- that is `verdict/`.
//!
//! ## * The two tables come from ONE pass, and that is the point
//!
//! `calc.rs` computes the geometry of a key twice today: once in `paint_calc` to
//! draw it, and once in `key_at` to know which key was pressed. Two copies of one
//! piece of arithmetic, and a whole class of bug -- the button that is drawn in
//! one place and answers in another.
//!
//! This generation knows every final rect, so [`Laid::hits`] and
//! [`Laid::islands`] fall out of the same tree that gets painted. The
//! duplication does not get fixed; it stops being possible.
//!
//! ## Coordinates are absolute, and signed
//!
//! Every rect is already in canvas coordinates, so the emitter never adds
//! anything up. And `x`/`y` are `i32` because a box centred in something smaller
//! than itself lands at a negative coordinate **in a browser too**. Clamping
//! would hide the overflow that `verdict/` exists to catch.

#![forbid(unsafe_code)]

pub mod flow;
pub mod measure;

use bmo_maqueta_cascade::{Cascaded, Style, Styled};
use bmo_maqueta_diag::Span;
use bmo_maqueta_node::Tag;

/// [!] **A SECOND COPY, and it is known.**
///
/// The real ones live in `Ultra_userspace/userland/src/pantalla.rs`, which is a
/// different workspace built for `x86_64-unknown-none`; this crate runs on
/// Windows. There is no shared home for them today -- `platform/shared/bmo-dibujo`
/// is geometry only and deliberately does not carry the font.
///
/// A second copy of a number is exactly the failure that `bmo.h` turned into a
/// fourth copy, so this one **cannot diverge in silence**: the test
/// `las_medidas_del_glifo_siguen_siendo_las_del_kernel` reads `pantalla.rs` from
/// disk and fails the moment the two disagree.
///
/// The real fix is moving the glyph metrics into `platform/shared/`. Written
/// down as debt rather than done here, because that is a change to Ring 3.
pub const GLIFO_ANCHO: u32 = 8;
pub const GLIFO_ALTO: u32 = 16;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

impl Rect {
    pub fn right(&self) -> i64 {
        self.x as i64 + self.w as i64
    }
    pub fn bottom(&self) -> i64 {
        self.y as i64 + self.h as i64
    }
    /// Does `self` fit entirely inside `outer`?
    pub fn inside(&self, outer: &Rect) -> bool {
        self.x >= outer.x
            && self.y >= outer.y
            && self.right() <= outer.right()
            && self.bottom() <= outer.bottom()
    }
}

/// A box that has landed.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Frame {
    pub tag: Tag,
    pub id: Option<String>,
    pub island: Option<String>,
    pub text: Option<String>,
    /// Carried, not read: this generation never looks at a colour.
    pub style: Style,
    /// ** Carried and never read EITHER -- and that is the proof that `:hover`
    /// was admitted safely. The father forbids a hover rule from touching
    /// anything but paint, so both styles give the same geometry, so this
    /// generation has nothing to do with it. The layout is computed once.
    pub hover: Option<Style>,
    /// The border box, in canvas coordinates.
    pub rect: Rect,
    /// Inside the border and the padding -- where the text and children live.
    pub content: Rect,
    /// Where the glyphs start, if this box carries text.
    ///
    /// * Separate from `content` because a box can centre its text, and centring
    /// is **layout**: `calc.rs` writes `bx + CALC_BTN/2 - GLIFO_ANCHO/2` by hand
    /// for every label. Leaving it to the emitter would put arithmetic in a
    /// consumer, which is where it stops being checkable.
    pub text_at: Option<Rect>,
    pub children: Vec<Frame>,
    pub span: Span,
}

impl Frame {
    fn walk<'a>(&'a self, out: &mut Vec<&'a Frame>) {
        out.push(self);
        for c in &self.children {
            c.walk(out);
        }
    }
}

/// Everything settled: one tree, one canvas.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Laid {
    pub root: Frame,
    pub canvas: (u32, u32),
}

impl Laid {
    /// Every box, in paint order.
    pub fn all(&self) -> Vec<&Frame> {
        let mut v = Vec::new();
        self.root.walk(&mut v);
        v
    }

    /// The hit table: which id owns which rect.
    ///
    /// * Same source as the paint list. That is what replaces `calc.rs`'s
    /// `button()`, `key_at()` and `contains()` -- three functions that are the
    /// same arithmetic written twice.
    pub fn hits(&self) -> Vec<(&str, Rect)> {
        self.all()
            .into_iter()
            .filter_map(|f| f.id.as_deref().map(|id| (id, f.rect)))
            .collect()
    }

    /// The islands: which rect another process fills.
    pub fn islands(&self) -> Vec<(&str, Rect)> {
        self.all()
            .into_iter()
            .filter_map(|f| f.island.as_deref().map(|n| (n, f.rect)))
            .collect()
    }
}

/// Settle where every box lands.
///
/// No `Result`: this generation cannot fail. Everything it could complain about
/// -- a box outside its parent, text that does not fit, a rect of zero width --
/// is a **judgement about a finished layout**, and judgements are `verdict/`'s.
pub fn lay(c: &Cascaded) -> Laid {
    let canvas = flow::canvas_of(&c.root);
    let root = flow::place(
        &c.root,
        Rect {
            x: 0,
            y: 0,
            w: canvas.0,
            h: canvas.1,
        },
    );
    Laid { root, canvas }
}

/// Convenience for callers that only have a styled tree.
pub fn lay_styled(root: &Styled) -> Laid {
    lay(&Cascaded {
        root: root.clone(),
        dead_rules: Vec::new(),
        orphan_classes: Vec::new(),
    })
}
