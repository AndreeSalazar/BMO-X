//! # MAQUETA -- DIAG, the shape of a rejection
//!
//! generacion: ninguna -- la forma de un rechazo, compartida
//!
//! **This is not a generation.** It is shared vocabulary, sitting below the
//! whole L7 chain the way `bmo-abi` sits below both the kernel and the
//! toolchain: a contract that belongs to nobody. `node`, `cascade`, `layout` and
//! `verdict` all produce these; none of them owns the type.
//!
//! ## Why a rejection has three parts and not one
//!
//! `LA_MAQUETA_EXIGE.md` section 6: *a rejection that does not show the way out
//! is a wall*. The whole point of this compiler is the inversion --
//!
//! > a browser ignores what it does not understand; a compiler rejects it
//!
//! -- and an inversion that leaves the author stuck is worse than the browser
//! it replaced. So every error carries **what**, **why**, and **instead**:
//!
//! ```text
//! maqueta: calc.maqueta:14:26: propiedad no soportada -- `box-shadow`
//!    14 |   .tecla { width:72px; box-shadow:0 2px 4px #000 }
//!       |                        ^^^^^^^^^^^^^^^^^^^^^^^^^
//!       = por que: una sombra necesita mezcla alfa, y el rasterizador esta en
//!         el escalon 2. La mezcla es el escalon 4.
//!       = en su lugar: `scene/mod.rs` pinta sombras con dos capas solidas. Si
//!         hace falta aqui, se declara con dos `<div>`.
//! ```
//!
//! `instead` may be empty -- some things genuinely have no replacement today --
//! but then the text has to *say so*, which is still an answer.
//!
//! ## Spanish on screen, English in the code
//!
//! The messages are output, so they are Spanish without accents; the
//! identifiers are a contract, so they are English. That frontier is the quote
//! mark, and it is the rule of the house since 2026-08-08.

#![forbid(unsafe_code)]

/// Where something is, in every form a message needs.
///
/// Carried rather than recomputed: rendering quotes the source line with a
/// caret under the span, and deriving line and column from an offset means
/// walking the file again for every single message.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Span {
    pub start: usize,
    pub len: usize,
    /// 1-based.
    pub line: u32,
    /// 1-based, in bytes. Sources are ASCII, so bytes are columns.
    pub col: u32,
}

impl Span {
    pub fn new(start: usize, len: usize, line: u32, col: u32) -> Self {
        Self { start, len, line, col }
    }
}

/// One rejection.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Error {
    pub span: Span,
    /// What happened, one line. Names the offending thing in backticks.
    pub title: String,
    /// Why MAQUETA cannot do it. The reason, not the rule number.
    pub why: String,
    /// What to write instead. Empty only when the honest answer is "nothing".
    pub instead: String,
}

impl Error {
    pub fn new(span: Span, title: &str, why: &str, instead: &str) -> Self {
        Self {
            span,
            title: title.to_string(),
            why: why.to_string(),
            instead: instead.to_string(),
        }
    }
}

/// Render every error against the source, in the format of the contract.
///
/// All of them, never just the first: a compiler that stops at error one turns
/// a five-minute fix into five compilations.
pub fn render(file: &str, src: &[u8], errors: &[Error]) -> String {
    let mut sorted: Vec<&Error> = errors.iter().collect();
    sorted.sort_by_key(|e| (e.span.start, e.span.len));

    let mut out = String::new();
    for e in sorted {
        out.push_str(&render_one(file, src, e));
        out.push('\n');
    }
    out
}

fn render_one(file: &str, src: &[u8], e: &Error) -> String {
    let mut s = String::new();
    s.push_str(&format!(
        "maqueta: {}:{}:{}: {}\n",
        file, e.span.line, e.span.col, e.title
    ));

    let line = source_line(src, e.span.line);
    // The gutter is five columns wide plus " | ", and the caret row has to
    // match it exactly or the caret lands under the wrong character.
    s.push_str(&format!("{:>5} | {}\n", e.span.line, line));
    s.push_str(&format!(
        "{:>5} | {}{}\n",
        "",
        " ".repeat(e.span.col.saturating_sub(1) as usize),
        "^".repeat(e.span.len.max(1)),
    ));

    s.push_str(&wrap_note("por que", &e.why));
    if e.instead.is_empty() {
        s.push_str(&wrap_note(
            "en su lugar",
            "no hay forma de escribir esto hoy. Quitalo.",
        ));
    } else {
        s.push_str(&wrap_note("en su lugar", &e.instead));
    }
    s
}

/// The nth line of the source, without its terminator.
///
/// Tabs become spaces so the caret stays under the thing it points at: a tab in
/// the quoted line and a space in the caret line drift apart by however wide the
/// reader's terminal decides a tab is.
fn source_line(src: &[u8], line: u32) -> String {
    let mut n = 1u32;
    let mut start = 0usize;
    let mut i = 0usize;
    while i < src.len() {
        if src[i] == b'\n' {
            if n == line {
                break;
            }
            n += 1;
            start = i + 1;
        }
        i += 1;
    }
    if n != line {
        return String::new();
    }
    let raw = &src[start..i];
    let raw = raw.strip_suffix(b"\r").unwrap_or(raw);
    raw.iter()
        .map(|&b| if b == b'\t' { ' ' } else { b as char })
        .collect()
}

/// A `= label: text` note, wrapped and hung under the label.
fn wrap_note(label: &str, text: &str) -> String {
    const WIDTH: usize = 66;
    let head = format!("{:>5} = {}: ", "", label);
    let hang = " ".repeat(head.len());

    let mut out = String::new();
    let mut line = head;
    let mut fresh = true;
    for word in text.split_whitespace() {
        if !fresh && line.len() + 1 + word.len() > WIDTH {
            out.push_str(line.trim_end());
            out.push('\n');
            line = hang.clone();
            fresh = true;
        }
        if !fresh {
            line.push(' ');
        }
        line.push_str(word);
        fresh = false;
    }
    out.push_str(line.trim_end());
    out.push('\n');
    out
}

// ========================================================================
//  Tests
// ========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &[u8] = b".pad { display:flex }\n.tecla { box-shadow:0 2px }\n";

    fn err() -> Error {
        // `box-shadow` starts at column 10 of line 2 and is ten bytes long.
        Error::new(
            Span::new(31, 10, 2, 10),
            "propiedad no soportada -- `box-shadow`",
            "una sombra necesita mezcla alfa, y el rasterizador esta en el escalon 2.",
            "dos `<div>` de color solido, como hace `scene/mod.rs`.",
        )
    }

    #[test]
    fn the_header_carries_file_line_and_column() {
        let out = render("calc.maqueta", SRC, &[err()]);
        assert!(out.starts_with("maqueta: calc.maqueta:2:10: propiedad no soportada"));
    }

    #[test]
    fn the_caret_lands_under_the_span() {
        let out = render("f.maqueta", SRC, &[err()]);
        let lines: Vec<&str> = out.lines().collect();
        let quoted = lines[1];
        let carets = lines[2];

        let bar = quoted.find('|').unwrap();
        assert_eq!(carets.find('|'), Some(bar), "the gutters must line up");

        let at = carets.find('^').unwrap();
        // What sits under the first caret has to be the offending text.
        assert!(quoted[at..].starts_with("box-shadow"));
        assert_eq!(carets.matches('^').count(), 10);
    }

    #[test]
    fn both_notes_are_present() {
        let out = render("f.maqueta", SRC, &[err()]);
        assert!(out.contains("= por que:"));
        assert!(out.contains("= en su lugar:"));
    }

    #[test]
    fn an_empty_instead_still_answers() {
        // "There is no way out" is an answer. Silence is not.
        let mut e = err();
        e.instead = String::new();
        let out = render("f.maqueta", SRC, &[e]);
        assert!(out.contains("no hay forma de escribir esto hoy"));
    }

    #[test]
    fn errors_come_out_in_source_order_not_discovery_order() {
        let late = err();
        let early = Error::new(Span::new(1, 3, 1, 2), "primero", "porque si", "nada");
        let out = render("f.maqueta", SRC, &[late, early]);
        assert!(out.find("primero").unwrap() < out.find("box-shadow").unwrap());
    }

    #[test]
    fn every_error_is_reported_not_just_the_first() {
        let out = render("f.maqueta", SRC, &[err(), err(), err()]);
        assert_eq!(out.matches("maqueta: f.maqueta").count(), 3);
    }

    #[test]
    fn a_tab_does_not_drift_the_caret() {
        // The quoted line and the caret line must agree on how wide a tab is,
        // and the only way to agree is for neither to contain one.
        let src = b"\t\t.a { width:50% }\n";
        let out = render("f.maqueta", src, &[Error::new(
            Span::new(13, 3, 1, 14),
            "unidad no soportada",
            "porque",
            "px",
        )]);
        assert!(!out.contains('\t'));
        let lines: Vec<&str> = out.lines().collect();
        let at = lines[2].find('^').unwrap();
        assert!(lines[1][at..].starts_with("50%"));
    }

    #[test]
    fn a_long_note_wraps_and_hangs_under_its_label() {
        let e = Error::new(
            Span::new(0, 1, 1, 1),
            "titulo",
            "una razon deliberadamente larga que no cabe en una sola linea de la \
             terminal y que por tanto tiene que partirse en varias sin perder la \
             sangria de su etiqueta",
            "",
        );
        let out = render("f.maqueta", SRC, &[e]);
        let note: Vec<&str> = out.lines().filter(|l| !l.contains('=')).collect();
        // The continuation rows carry no `=`, so they are the ones above.
        assert!(note.iter().any(|l| l.starts_with("      ") && !l.contains('|')));
        assert!(out.lines().all(|l| l.len() <= 80));
    }

    #[test]
    fn a_line_number_past_the_end_does_not_panic() {
        let e = Error::new(Span::new(0, 1, 99, 1), "t", "w", "i");
        let out = render("f.maqueta", SRC, &[e]);
        assert!(out.contains("maqueta: f.maqueta:99:1"));
    }

    #[test]
    fn a_zero_length_span_still_gets_one_caret() {
        // Happens at end of file: "the tag was never closed" points at nothing.
        let e = Error::new(Span::new(0, 0, 1, 1), "t", "w", "i");
        let out = render("f.maqueta", SRC, &[e]);
        assert_eq!(out.lines().nth(2).unwrap().matches('^').count(), 1);
    }

    #[test]
    fn carriage_returns_do_not_reach_the_message() {
        let src = b".a { }\r\n.b { }\r\n";
        let out = render("f.maqueta", src, &[Error::new(Span::new(8, 2, 2, 1), "t", "w", "i")]);
        assert!(!out.contains('\r'));
    }
}
