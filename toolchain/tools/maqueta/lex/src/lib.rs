//! # MAQUETA -- LEX, the grandfather generation
//!
//! Bytes in, tokens out. **This module does not know whether the document is
//! valid**, what a `div` is, or that a tree will ever be built. It reports raw
//! lexical facts and nothing else.
//!
//! ## Why that sentence is the whole design (L7)
//!
//! `META-KERNEL_HARD.md` L7 says: *the grandfather is the raw fact and does not
//! know what it is used for*, and L7a: *if a generation needs to know something
//! about its consumer, the cut is wrong*.
//!
//! Apply that here and it settles the single biggest question about this
//! project, without anyone having to decide it:
//!
//! > HTML5 error recovery -- the `<p>` that self-closes, the badly nested tag a
//! > browser silently repairs -- **requires the tokenizer to consult the state
//! > of the tree**. That is a grandfather knowing its consumer. **L7a forbids
//! > it.**
//!
//! So MAQUETA cannot be a browser even if someone tried. The law that was
//! already written does not allow it. See `docs/plan/PLAN_MAQUETA.md` section 4.
//!
//! ## The three lexical modes, and why they do not break the law
//!
//! ```text
//!    Outside   between tags       -- free text until the next `<`
//!    InTag     inside `<...>`     -- names, `=`, quoted strings
//!    Style     inside `<style>`   -- selectors and declarations
//! ```
//!
//! Switching modes on the literal byte sequences `<style>` and `</style>` is a
//! **lexical** rule, not a syntactic one: nothing is consulted but the bytes
//! under the cursor. HTML does exactly this for its raw-text elements. A mode
//! driven by *the shape of the tree so far* would be the forbidden thing; a mode
//! driven by seven literal bytes is not.
//!
//! ## Rawness, deliberately
//!
//! `72px` is **two** tokens (`Number`, `Ident`), not one measurement. An
//! attribute is **three** (`Ident`, `Eq`, `Str`), not a pair. Composing them is
//! the father's job -- see `node/`. Emitting a `Measurement` here would be this
//! generation deciding what its consumer needs.
//!
//! ## Errors
//!
//! There are none. A byte this module cannot classify becomes `Kind::Unknown`
//! or `Kind::NonAscii` **with its position**, which is a fact. Turning a fact
//! into a rejection is an opinion, and in this chain only the great-grandson
//! has opinions (`verdict/`).

#![forbid(unsafe_code)]

/// What a token is, at the only level of meaning this generation has: shape.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    // -- both modes ------------------------------------------------------
    /// `div`, `class`, `flex-direction`, `space-between`
    Ident,
    /// `72`, `0`. Never carries a unit: `72px` is `Number` then `Ident`.
    Number,

    // -- markup mode -----------------------------------------------------
    /// `<`
    Lt,
    /// `</`
    LtSlash,
    /// `>`
    Gt,
    /// `/>`
    SlashGt,
    /// `=`
    Eq,
    /// `"pad"`. The span covers the contents, not the quotes.
    Str,
    /// Free text between tags. Whitespace included, on purpose -- see `lex`.
    Text,

    // -- style mode ------------------------------------------------------
    /// The literal `<style>`
    StyleOpen,
    /// The literal `</style>`
    StyleClose,
    /// `.`
    Dot,
    /// `,`
    Comma,
    /// `{`
    LBrace,
    /// `}`
    RBrace,
    /// `:`
    Colon,
    /// `;`
    Semi,
    /// `#` followed by exactly six hex digits. The span covers the six.
    Color,
    /// A `#` that is not a colour. Kept distinct so the father can say
    /// "id selectors do not exist here" instead of "unexpected byte".
    Hash,
    /// `%`. Exists only so the rejection can name what it saw.
    Pct,

    // -- facts this generation cannot classify ---------------------------
    /// A byte that fits no rule, or an unterminated string or comment.
    Unknown,
    /// A run of bytes above 0x7F. Sources are ASCII -- see
    /// `docs/identidad/` and the `n` with a tilde that once grew a `.bex`
    /// from 512 bytes to 492.032.
    NonAscii,
}

impl Kind {
    /// The name of the shape. Not its meaning -- this generation has none.
    pub fn name(self) -> &'static str {
        match self {
            Kind::Ident => "nombre",
            Kind::Number => "numero",
            Kind::Lt => "`<`",
            Kind::LtSlash => "`</`",
            Kind::Gt => "`>`",
            Kind::SlashGt => "`/>`",
            Kind::Eq => "`=`",
            Kind::Str => "texto entre comillas",
            Kind::Text => "texto",
            Kind::StyleOpen => "`<style>`",
            Kind::StyleClose => "`</style>`",
            Kind::Dot => "`.`",
            Kind::Comma => "`,`",
            Kind::LBrace => "`{`",
            Kind::RBrace => "`}`",
            Kind::Colon => "`:`",
            Kind::Semi => "`;`",
            Kind::Color => "color",
            Kind::Hash => "`#`",
            Kind::Pct => "`%`",
            Kind::Unknown => "byte desconocido",
            Kind::NonAscii => "byte fuera de ASCII",
        }
    }
}

/// A token: a shape, where it starts, how long it is, and where a human should
/// be told to look.
///
/// `line` and `col` are carried rather than recomputed because the error format
/// in `LA_MAQUETA_EXIGE.md` section 6 quotes the source line with a caret under
/// the offending span, and recomputing that from an offset means walking the
/// file again for every message.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Token {
    pub kind: Kind,
    /// Byte offset of the first byte of the span.
    pub start: usize,
    pub len: usize,
    /// 1-based.
    pub line: u32,
    /// 1-based, in bytes. Sources are ASCII so bytes are columns.
    pub col: u32,
}

impl Token {
    /// The bytes this token covers. Slicing is not interpretation.
    pub fn text<'a>(&self, src: &'a [u8]) -> &'a [u8] {
        &src[self.start..self.start + self.len]
    }
}

const STYLE_OPEN: &[u8] = b"<style>";
const STYLE_CLOSE: &[u8] = b"</style>";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Outside,
    InTag,
    Style,
}

/// Turn a source file into tokens.
///
/// Whitespace-only `Text` tokens **are emitted**. Dropping them here would be
/// this generation deciding that indentation is not content -- true for a `div`
/// between two lines, false for the space between two `span`s on one line. The
/// father knows which case it is; the grandfather does not.
pub fn lex(src: &[u8]) -> Vec<Token> {
    let mut lx = Lexer {
        src,
        i: 0,
        line: 1,
        col: 1,
        out: Vec::new(),
    };
    lx.run();
    lx.out
}

struct Lexer<'a> {
    src: &'a [u8],
    i: usize,
    line: u32,
    col: u32,
    out: Vec<Token>,
}

impl<'a> Lexer<'a> {
    fn run(&mut self) {
        let mut mode = Mode::Outside;
        while self.i < self.src.len() {
            mode = match mode {
                Mode::Outside => self.outside(),
                Mode::InTag => self.in_tag(),
                Mode::Style => self.style(),
            };
        }
    }

    // -- cursor ----------------------------------------------------------

    fn at(&self, off: usize) -> u8 {
        *self.src.get(self.i + off).unwrap_or(&0)
    }

    fn starts_with(&self, pat: &[u8]) -> bool {
        self.src.len() >= self.i + pat.len() && &self.src[self.i..self.i + pat.len()] == pat
    }

    /// Advance `n` bytes, keeping line and column honest.
    fn bump(&mut self, n: usize) {
        for _ in 0..n {
            if self.i >= self.src.len() {
                return;
            }
            if self.src[self.i] == b'\n' {
                self.line += 1;
                self.col = 1;
            } else {
                self.col += 1;
            }
            self.i += 1;
        }
    }

    /// Emit a token for the `n` bytes starting at the cursor, then advance.
    fn emit(&mut self, kind: Kind, n: usize) {
        self.out.push(Token {
            kind,
            start: self.i,
            len: n,
            line: self.line,
            col: self.col,
        });
        self.bump(n);
    }

    /// Emit `n` bytes starting `skip` bytes in -- for quoted strings, whose
    /// span is the contents and whose consumption includes the quotes.
    fn emit_inner(&mut self, kind: Kind, skip: usize, inner: usize, total: usize) {
        self.out.push(Token {
            kind,
            start: self.i + skip,
            len: inner,
            line: self.line,
            col: self.col + skip as u32,
        });
        self.bump(total);
    }

    fn skip_ws(&mut self) {
        while self.i < self.src.len() && is_ws(self.src[self.i]) {
            self.bump(1);
        }
    }

    // -- modes -----------------------------------------------------------

    /// Between tags: free text, comments, and the `<` that opens a tag.
    fn outside(&mut self) -> Mode {
        if self.starts_with(b"<!--") {
            self.skip_until(b"-->", 4, 3);
            return Mode::Outside;
        }
        if self.starts_with(STYLE_OPEN) {
            self.emit(Kind::StyleOpen, STYLE_OPEN.len());
            return Mode::Style;
        }
        if self.starts_with(b"</") {
            self.emit(Kind::LtSlash, 2);
            return Mode::InTag;
        }
        if self.at(0) == b'<' {
            self.emit(Kind::Lt, 1);
            return Mode::InTag;
        }

        // Everything up to the next `<` is text. A non-ASCII run inside it is
        // still reported separately: it has to be pointed at, not swallowed.
        if self.src[self.i] > 0x7F {
            let n = self.run_of(|b| b > 0x7F);
            self.emit(Kind::NonAscii, n);
            return Mode::Outside;
        }
        let n = self.run_of(|b| b != b'<' && b <= 0x7F);
        if n == 0 {
            self.emit(Kind::Unknown, 1);
        } else {
            self.emit(Kind::Text, n);
        }
        Mode::Outside
    }

    /// Inside `<...>`: names, `=`, quoted strings, and the closer.
    fn in_tag(&mut self) -> Mode {
        self.skip_ws();
        if self.i >= self.src.len() {
            return Mode::InTag;
        }
        if self.starts_with(b"/>") {
            self.emit(Kind::SlashGt, 2);
            return Mode::Outside;
        }
        match self.at(0) {
            b'>' => {
                self.emit(Kind::Gt, 1);
                Mode::Outside
            }
            b'=' => {
                self.emit(Kind::Eq, 1);
                Mode::InTag
            }
            b'"' => {
                self.quoted();
                Mode::InTag
            }
            b => {
                self.word_or_unknown(b);
                Mode::InTag
            }
        }
    }

    /// Inside `<style>`: selectors, blocks, declarations.
    fn style(&mut self) -> Mode {
        self.skip_ws();
        if self.i >= self.src.len() {
            return Mode::Style;
        }
        if self.starts_with(STYLE_CLOSE) {
            self.emit(Kind::StyleClose, STYLE_CLOSE.len());
            return Mode::Outside;
        }
        if self.starts_with(b"/*") {
            self.skip_until(b"*/", 2, 2);
            return Mode::Style;
        }
        let one = |k: Kind| -> Option<Kind> { Some(k) };
        let single = match self.at(0) {
            b'.' => one(Kind::Dot),
            b',' => one(Kind::Comma),
            b'{' => one(Kind::LBrace),
            b'}' => one(Kind::RBrace),
            b':' => one(Kind::Colon),
            b';' => one(Kind::Semi),
            b'%' => one(Kind::Pct),
            _ => None,
        };
        if let Some(k) = single {
            self.emit(k, 1);
            return Mode::Style;
        }
        if self.at(0) == b'#' {
            self.hash();
            return Mode::Style;
        }
        self.word_or_unknown(self.at(0));
        Mode::Style
    }

    // -- pieces ----------------------------------------------------------

    /// `#RRGGBB` is a colour; any other `#` is just a `#`.
    ///
    /// The distinction is lexical (six hex digits and then a non-name byte),
    /// so it belongs here. What a `#` *means* in a selector -- an id, which
    /// MAQUETA does not have -- is the father's problem.
    fn hash(&mut self) {
        let hex = (1..7).all(|k| is_hex(self.at(k)));
        let ends = !is_name(self.at(7));
        if hex && ends {
            self.emit_inner(Kind::Color, 1, 6, 7);
        } else {
            self.emit(Kind::Hash, 1);
        }
    }

    fn quoted(&mut self) {
        let mut k = 1;
        while self.i + k < self.src.len() && self.src[self.i + k] != b'"' {
            if self.src[self.i + k] == b'\n' {
                break; // a string never crosses a line: the closer was forgotten
            }
            k += 1;
        }
        if self.i + k < self.src.len() && self.src[self.i + k] == b'"' {
            self.emit_inner(Kind::Str, 1, k - 1, k + 1);
        } else {
            // Unterminated. Report the opening quote and carry on from the next
            // byte -- swallowing the rest of the file would turn one mistake
            // into a page of nonsense.
            self.emit(Kind::Unknown, 1);
        }
    }

    fn word_or_unknown(&mut self, b: u8) {
        if b > 0x7F {
            let n = self.run_of(|c| c > 0x7F);
            self.emit(Kind::NonAscii, n);
        } else if b.is_ascii_digit() {
            let n = self.run_of(|c| c.is_ascii_digit());
            self.emit(Kind::Number, n);
        } else if b.is_ascii_alphabetic() || b == b'_' {
            let n = self.run_of(is_name);
            self.emit(Kind::Ident, n);
        } else {
            self.emit(Kind::Unknown, 1);
        }
    }

    /// How many bytes from the cursor satisfy `f`.
    fn run_of(&self, f: impl Fn(u8) -> bool) -> usize {
        let mut n = 0;
        while self.i + n < self.src.len() && f(self.src[self.i + n]) {
            n += 1;
        }
        n
    }

    /// Skip an opener, its contents and its closer. Returns false and emits an
    /// `Unknown` at the opener if the closer never arrives.
    fn skip_until(&mut self, close: &[u8], open_len: usize, close_len: usize) -> bool {
        let mut k = open_len;
        while self.i + k + close.len() <= self.src.len() {
            if &self.src[self.i + k..self.i + k + close.len()] == close {
                self.bump(k + close_len);
                return true;
            }
            k += 1;
        }
        self.emit(Kind::Unknown, open_len);
        false
    }
}

fn is_ws(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\r' | b'\n')
}

/// Bytes that continue a name. `-` is in because CSS is full of it
/// (`flex-direction`, `space-between`).
fn is_name(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'-' || b == b'_'
}

fn is_hex(b: u8) -> bool {
    b.is_ascii_hexdigit()
}

// ========================================================================
//  Tests
// ========================================================================
//
// This generation is testable in three seconds precisely because it knows
// nothing: every case below is bytes in, shapes out, with no document, no tree
// and no stylesheet anywhere in sight.

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(src: &str) -> Vec<Kind> {
        lex(src.as_bytes()).iter().map(|t| t.kind).collect()
    }

    fn texts(src: &str) -> Vec<String> {
        lex(src.as_bytes())
            .iter()
            .map(|t| String::from_utf8_lossy(t.text(src.as_bytes())).into_owned())
            .collect()
    }

    #[test]
    fn empty_source_has_no_tokens() {
        assert!(lex(b"").is_empty());
    }

    #[test]
    fn a_tag_is_its_parts_and_not_a_tag() {
        // The point of the grandfather: `<div>` is three shapes, not one tag.
        assert_eq!(kinds("<div>"), [Kind::Lt, Kind::Ident, Kind::Gt]);
    }

    #[test]
    fn closing_tag() {
        assert_eq!(kinds("</div>"), [Kind::LtSlash, Kind::Ident, Kind::Gt]);
    }

    #[test]
    fn self_closing_tag() {
        assert_eq!(
            kinds("<island nombre=\"v\"/>"),
            [Kind::Lt, Kind::Ident, Kind::Ident, Kind::Eq, Kind::Str, Kind::SlashGt]
        );
    }

    #[test]
    fn attribute_is_three_tokens_not_a_pair() {
        let t = texts("<div class=\"pad\">");
        assert_eq!(t, ["<", "div", "class", "=", "pad", ">"]);
    }

    #[test]
    fn text_between_tags_survives_whitespace() {
        // Whitespace is kept: the father decides whether it is content.
        assert_eq!(kinds("<div> hola </div>").get(3), Some(&Kind::Text));
        assert_eq!(texts("<div> hola </div>")[3], " hola ");
    }

    #[test]
    fn style_switches_mode_on_literal_bytes() {
        let k = kinds("<style>.a{gap:6px}</style>");
        assert_eq!(
            k,
            [
                Kind::StyleOpen,
                Kind::Dot,
                Kind::Ident,
                Kind::LBrace,
                Kind::Ident,
                Kind::Colon,
                Kind::Number,
                Kind::Ident,
                Kind::RBrace,
                Kind::StyleClose,
            ]
        );
    }

    #[test]
    fn a_measurement_is_two_tokens() {
        // `72px` is deliberately NOT one token. Composing it is the father's job.
        let t = texts("<style>.a{width:72px}</style>");
        assert!(t.contains(&"72".to_string()));
        assert!(t.contains(&"px".to_string()));
    }

    #[test]
    fn colour_is_six_hex_digits_and_the_span_excludes_the_hash() {
        let src = "<style>.a{color:#182434}</style>";
        let toks = lex(src.as_bytes());
        let c = toks.iter().find(|t| t.kind == Kind::Color).unwrap();
        assert_eq!(c.text(src.as_bytes()), b"182434");
    }

    #[test]
    fn a_hash_that_is_not_a_colour_stays_a_hash() {
        // `#pad` is an id selector. MAQUETA has none -- but saying so is the
        // father's opinion, not this generation's.
        let k = kinds("<style>#pad{}</style>");
        assert_eq!(k[1], Kind::Hash);
        assert_eq!(k[2], Kind::Ident);
    }

    #[test]
    fn a_short_hash_is_not_mistaken_for_a_colour() {
        // Five hex digits is not a colour. It comes back as `#` and a number,
        // which is exactly what happened -- and the father is the one that gets
        // to call it a mistake.
        let k = kinds("<style>.a{color:#182}</style>");
        assert!(k.contains(&Kind::Hash));
        assert!(!k.contains(&Kind::Color));
    }

    #[test]
    fn a_colour_at_the_very_end_of_the_file_is_still_a_colour() {
        // The lookahead past the six digits reads off the end. It must come
        // back "not a name byte" rather than panicking or refusing.
        let k = kinds("<style>.a{color:#182434");
        assert!(k.contains(&Kind::Color));
    }

    #[test]
    fn seven_hex_digits_are_not_a_colour() {
        // The seventh byte is a name byte, so the shape is not `#RRGGBB`.
        let k = kinds("<style>.a{color:#1824345}</style>");
        assert!(k.contains(&Kind::Hash));
        assert!(!k.contains(&Kind::Color));
    }

    #[test]
    fn percent_is_reported_so_the_rejection_can_name_it() {
        let k = kinds("<style>.a{width:50%}</style>");
        assert!(k.contains(&Kind::Pct));
    }

    #[test]
    fn hyphenated_names_are_one_name() {
        let t = texts("<style>.a{flex-direction:column}</style>");
        assert!(t.contains(&"flex-direction".to_string()));
        assert!(t.contains(&"column".to_string()));
    }

    #[test]
    fn comments_leave_no_trace() {
        assert_eq!(kinds("<!-- nota --><div>"), [Kind::Lt, Kind::Ident, Kind::Gt]);
        assert_eq!(
            kinds("<style>/* nota */.a{}</style>"),
            [Kind::StyleOpen, Kind::Dot, Kind::Ident, Kind::LBrace, Kind::RBrace, Kind::StyleClose]
        );
    }

    #[test]
    fn an_unterminated_comment_is_reported_and_does_not_eat_the_file() {
        let k = kinds("<!-- nota");
        assert_eq!(k[0], Kind::Unknown);
    }

    #[test]
    fn an_unterminated_string_is_reported_at_its_opening_quote() {
        let k = kinds("<div class=\"pad>");
        assert_eq!(k[3], Kind::Eq);
        assert_eq!(k[4], Kind::Unknown);
    }

    #[test]
    fn non_ascii_is_one_token_per_run_not_one_per_byte() {
        // Sources are ASCII. The run is reported whole so the message can point
        // at the word, not at three bytes of one letter.
        //
        // [!] The offending bytes are written as escapes ON PURPOSE. A test for
        // "this file rejects non-ASCII" that carries a literal accented letter
        // is a source file breaking the rule it is testing -- and `ascii-sweep`
        // would be right to flag it. \xC3\xB1 is an n with a tilde in UTF-8.
        let toks = lex(b"<div>\xC3\xB1</div>");
        let n: Vec<_> = toks.iter().filter(|t| t.kind == Kind::NonAscii).collect();
        assert_eq!(n.len(), 1, "one run, not one token per byte");
        assert_eq!(n[0].len, 2, "both bytes of the letter, together");
    }

    #[test]
    fn non_ascii_inside_a_tag_is_reported_too() {
        // The outside-of-tags path and the in-tag path are different code.
        let toks = lex(b"<div cl\xC3\xA1se=\"a\">");
        assert!(toks.iter().any(|t| t.kind == Kind::NonAscii));
    }

    #[test]
    fn line_and_column_are_one_based_and_survive_newlines() {
        let src = "<div>\n  <span>";
        let toks = lex(src.as_bytes());
        assert_eq!((toks[0].line, toks[0].col), (1, 1)); // `<`
        let span = toks.iter().find(|t| t.kind == Kind::Ident && t.line == 2).unwrap();
        assert_eq!(span.col, 4); // two spaces, then `<`, then the name
    }

    #[test]
    fn the_column_of_a_quoted_value_points_at_the_value() {
        //          1234567890123456
        let src = "<div class=\"pad\">";
        let toks = lex(src.as_bytes());
        let s = toks.iter().find(|t| t.kind == Kind::Str).unwrap();
        assert_eq!(s.col, 13);
        assert_eq!(s.text(src.as_bytes()), b"pad");
    }

    #[test]
    fn markup_inside_style_is_not_markup() {
        // Proof the mode is real: a `<` in style mode is not a tag opener.
        let k = kinds("<style>a<b</style>");
        assert_eq!(k[0], Kind::StyleOpen);
        assert!(!k.contains(&Kind::Lt));
        assert_eq!(*k.last().unwrap(), Kind::StyleClose);
    }

    #[test]
    fn no_input_produces_an_error_because_this_generation_has_no_opinions() {
        // Every one of these is malformed. None of them panics, and none of
        // them refuses: they all come back as facts with positions.
        for bad in ["<", "</", "<<>>", "=", "\"", "<style>", "}{", "#", "%%%"] {
            let _ = lex(bad.as_bytes());
        }
    }
}
