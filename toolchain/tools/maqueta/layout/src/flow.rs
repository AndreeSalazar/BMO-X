//! **Where does each box go?** Top-down, and this is where boxes finally see
//! each other -- which is the son's product being *related*, so it is the
//! grandson's whole reason to exist.
//!
//! ## The two rules that answer "what is an undeclared size"
//!
//! The son recorded `None` rather than inventing a number, because the answer
//! depends on the neighbours. Here are the neighbours, so here is the answer:
//!
//! ```text
//!    block child, no width      -> the container's content width   (fills)
//!    flex item,   no main size  -> what its content needs          (shrinks)
//!    flex item,   no cross size -> the container's, if `stretch`
//! ```
//!
//! All three are CSS's, and that is the point: the answers had to be *somebody's*
//! and they had to be the browser's.
//!
//! ## Free space is signed, and stays signed
//!
//! `content - used` goes negative the moment something does not fit, and a box
//! centred inside a smaller one lands at a negative coordinate -- in a browser
//! too. Clamping it to zero here would hide an overflow that `verdict/` is
//! supposed to catch, so positions are `i32` and nothing saturates.
//!
//! It is the same trap the rasterizer wrote down: with the wrong width an
//! overflow flips the SIGN and the shape comes out inside out.

use bmo_maqueta_cascade::{Align, Direction, Display, Justify, Position, Styled};

use crate::measure::{content_size, frame, outer_size};
use crate::{Frame, Rect};

/// Lay one box out inside the border box its parent decided for it.
pub fn place(b: &Styled, border: Rect) -> Frame {
    let inset = b.style.border_width as i32 + b.style.padding[3] as i32;
    let inset_top = b.style.border_width as i32 + b.style.padding[0] as i32;
    let (fw, fh) = frame(b);
    let content = Rect {
        x: border.x + inset,
        y: border.y + inset_top,
        w: border.w.saturating_sub(fw),
        h: border.h.saturating_sub(fh),
    };

    let flow: Vec<&Styled> = b
        .children
        .iter()
        .filter(|c| c.style.position == Position::Static)
        .collect();

    let mut placed: Vec<Frame> = match b.style.display {
        Display::Flex => flex(b, &flow, content),
        Display::Block => block(&flow, content),
    };

    // ★ Absolutely positioned boxes are placed against the canvas, and that is
    // not a simplification: CSS anchors them to the nearest *positioned*
    // ancestor, MAQUETA has no `position:relative`, so there never is one and
    // the anchor is always the initial containing block. Same behaviour, arrived
    // at by having fewer parts.
    for c in b.children.iter().filter(|c| c.style.position == Position::Absolute) {
        let o = outer_size(c);
        placed.push(place(
            c,
            Rect {
                x: c.style.left.unwrap_or(0) as i32,
                y: c.style.top.unwrap_or(0) as i32,
                w: o.w,
                h: o.h,
            },
        ));
    }

    Frame {
        tag: b.tag,
        id: b.id.clone(),
        island: b.island.clone(),
        text: b.text.clone(),
        style: b.style,
        hover: b.hover,
        rect: border,
        content,
        text_at: donde_el_texto(b, content),
        children: placed,
        span: b.span,
    }
}

/// Where the glyphs start.
///
/// ★ In a `block` box the text sits at the top left of the content, which is
/// what a browser does for left-to-right text. In a `flex` box the text is an
/// **anonymous flex item** -- a real CSS concept, not an invention -- so
/// `justify-content` and `align-items` move it, and that is how a label gets
/// centred in its button.
///
/// This is here and not in the emitter on purpose: centring is arithmetic, and
/// arithmetic in a consumer is arithmetic nobody checks. `calc.rs` writes
/// `bx + CALC_BTN/2 - GLIFO_ANCHO/2` by hand, once per label.
fn donde_el_texto(b: &Styled, content: Rect) -> Option<Rect> {
    let t = b.text.as_ref()?;
    let w = t.len() as u32 * crate::GLIFO_ANCHO;
    let h = crate::GLIFO_ALTO;
    if b.style.display != Display::Flex {
        return Some(Rect { x: content.x, y: content.y, w, h });
    }
    let (libre_main, libre_cross, horizontal) = if b.style.direction == Direction::Row {
        (content.w as i64 - w as i64, content.h as i64 - h as i64, true)
    } else {
        (content.h as i64 - h as i64, content.w as i64 - w as i64, false)
    };
    let main = match b.style.justify {
        Justify::Start | Justify::SpaceBetween => 0,
        Justify::Center => libre_main / 2,
        Justify::End => libre_main,
    };
    let cross = match b.style.align {
        Align::Stretch | Align::Start => 0,
        Align::Center => libre_cross / 2,
        Align::End => libre_cross,
    };
    let (dx, dy) = if horizontal { (main, cross) } else { (cross, main) };
    Some(Rect {
        x: (content.x as i64 + dx) as i32,
        y: (content.y as i64 + dy) as i32,
        w,
        h,
    })
}

/// Children stack downwards, each filling the width unless it named one.
fn block(flow: &[&Styled], content: Rect) -> Vec<Frame> {
    let mut y = content.y;
    let mut out = Vec::with_capacity(flow.len());
    for c in flow {
        let (fw, _) = frame(c);
        let w = c.style.width.map(|w| w + fw).unwrap_or(content.w);
        let h = outer_size(c).h;
        out.push(place(c, Rect { x: content.x, y, w, h }));
        y += h as i32;
    }
    out
}

fn flex(b: &Styled, flow: &[&Styled], content: Rect) -> Vec<Frame> {
    if flow.is_empty() {
        return Vec::new();
    }
    let row = b.style.direction == Direction::Row;
    let gap = b.style.gap;

    let outers: Vec<_> = flow.iter().map(|c| outer_size(c)).collect();
    let used: u32 = outers
        .iter()
        .map(|s| if row { s.w } else { s.h })
        .sum::<u32>()
        + gap * (flow.len() as u32 - 1);

    let room = if row { content.w } else { content.h };
    let free = room as i64 - used as i64;

    // Signed on purpose: `Center` with more content than room is a negative
    // offset in a browser too, and pretending otherwise would hide the overflow.
    let (lead, between) = match b.style.justify {
        Justify::Start => (0i64, 0i64),
        Justify::Center => (free / 2, 0),
        Justify::End => (free, 0),
        Justify::SpaceBetween => {
            if flow.len() > 1 && free > 0 {
                (0, free / (flow.len() as i64 - 1))
            } else {
                (0, 0)
            }
        }
    };

    let mut main = if row { content.x } else { content.y } as i64 + lead;
    let mut out = Vec::with_capacity(flow.len());

    for (c, o) in flow.iter().zip(&outers) {
        let (fw, fh) = frame(c);
        let (main_len, cross_len) = if row {
            let cross = match (c.style.height, b.style.align) {
                (Some(h), _) => h + fh,
                (None, Align::Stretch) => content.h,
                (None, _) => o.h,
            };
            (o.w, cross)
        } else {
            let cross = match (c.style.width, b.style.align) {
                (Some(w), _) => w + fw,
                (None, Align::Stretch) => content.w,
                (None, _) => o.w,
            };
            (o.h, cross)
        };

        let room_cross = if row { content.h } else { content.w };
        let cross_start = if row { content.y } else { content.x } as i64;
        let cross_pos = match b.style.align {
            Align::Stretch | Align::Start => cross_start,
            Align::Center => cross_start + (room_cross as i64 - cross_len as i64) / 2,
            Align::End => cross_start + room_cross as i64 - cross_len as i64,
        };

        let r = if row {
            Rect {
                x: main as i32,
                y: cross_pos as i32,
                w: main_len,
                h: cross_len,
            }
        } else {
            Rect {
                x: cross_pos as i32,
                y: main as i32,
                w: cross_len,
                h: main_len,
            }
        };
        out.push(place(c, r));
        main += main_len as i64 + gap as i64 + between;
    }
    out
}

/// The size the root ends up with: what it declared, or what its tree needs.
pub fn canvas_of(root: &Styled) -> (u32, u32) {
    match root.canvas {
        Some((w, h)) => (w, h),
        None => {
            let c = content_size(root);
            let (fw, fh) = frame(root);
            (c.w + fw, c.h + fh)
        }
    }
}
