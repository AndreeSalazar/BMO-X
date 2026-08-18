//! **How big does this box want to be?** Bottom-up, and it asks nobody above.
//!
//! Two sizes per box, and the difference is the CSS box model:
//!
//! ```text
//!    content    what `width`/`height` name -- CSS's default `content-box`
//!    outer      content + padding + border on all four sides
//! ```
//!
//! `content-box` is the confusing one of the two, and it is chosen anyway,
//! because it is what a browser does when nobody says otherwise. Fidelity of the
//! preview beats convenience of the author -- that trade is the whole reason the
//! tags are HTML's in the first place.

use bmo_maqueta_cascade::{Direction, Display, Position, Styled};

use crate::{GLIFO_ALTO, GLIFO_ANCHO};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Size {
    pub w: u32,
    pub h: u32,
}

/// Padding plus border, horizontally and vertically.
pub fn frame(b: &Styled) -> (u32, u32) {
    let p = b.style.padding;
    let d = b.style.border_width * 2;
    (p[1] + p[3] + d, p[0] + p[2] + d)
}

/// The content size this box settles on: what it declared, or what it needs.
pub fn content_size(b: &Styled) -> Size {
    let want = intrinsic(b);
    Size {
        w: b.style.width.unwrap_or(want.w),
        h: b.style.height.unwrap_or(want.h),
    }
}

/// The border box: what the parent has to make room for.
pub fn outer_size(b: &Styled) -> Size {
    let c = content_size(b);
    let (fw, fh) = frame(b);
    Size {
        w: c.w + fw,
        h: c.h + fh,
    }
}

/// What the box needs if nothing constrains it -- CSS's `max-content`.
fn intrinsic(b: &Styled) -> Size {
    if let Some(t) = &b.text {
        // ★ The measurement that makes this whole compiler possible. The font is
        // a fixed-width bitmap, so text is arithmetic and not a rendering
        // problem: no shaping, no kerning, no fallback, no line breaking. And
        // `len()` is the character count because the father already refused
        // every byte above 0x7F.
        return Size {
            w: t.len() as u32 * GLIFO_ANCHO,
            h: GLIFO_ALTO,
        };
    }

    // Absolutely positioned children are out of the flow, so they contribute
    // nothing to what their parent needs -- same as CSS.
    let flow: Vec<&Styled> = b
        .children
        .iter()
        .filter(|c| c.style.position == Position::Static)
        .collect();

    if flow.is_empty() {
        return Size { w: 0, h: 0 };
    }

    let sizes: Vec<Size> = flow.iter().map(|c| outer_size(c)).collect();
    let gaps = b.style.gap * (flow.len() as u32 - 1);

    match (b.style.display, b.style.direction) {
        (Display::Flex, Direction::Row) => Size {
            w: sizes.iter().map(|s| s.w).sum::<u32>() + gaps,
            h: sizes.iter().map(|s| s.h).max().unwrap_or(0),
        },
        (Display::Flex, Direction::Column) => Size {
            w: sizes.iter().map(|s| s.w).max().unwrap_or(0),
            h: sizes.iter().map(|s| s.h).sum::<u32>() + gaps,
        },
        // Block: children stack, each as wide as it needs. `gap` does nothing
        // here, exactly as in CSS, and `verdict/` is where saying so belongs.
        (Display::Block, _) => Size {
            w: sizes.iter().map(|s| s.w).max().unwrap_or(0),
            h: sizes.iter().map(|s| s.h).sum::<u32>(),
        },
    }
}
