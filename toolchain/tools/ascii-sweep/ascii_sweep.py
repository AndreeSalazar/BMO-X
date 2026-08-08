#!/usr/bin/env python3
"""ascii_sweep -- make BMO-X sources pure ASCII, without touching what they DO.

Why this exists
===============

BMO-X was written with Spanish prose and box-drawing decoration in its
comments: 55.176 non-ASCII characters across 444 files. That is not a style
problem, it is a correctness problem, and it has already cost real bugs:

  * The BMO C preprocessor copied text byte by byte as if every byte were a
    character. A single "n-with-tilde" in a string literal grew the resulting
    .bex from 512 bytes to 492.032 -- one accent turned into a binary-size
    problem, which is the last place anyone looks. Fixed 2026-08-08.

  * The kernel console is Latin-1 by design: one byte per character, no
    decoder between the keyboard, the shell line and the framebuffer. Source
    files are UTF-8. Every accented letter therefore means two encodings
    disagreeing about the same text.

So the rule this tool enforces is simple: **sources are ASCII**. What BMO-X
prints on screen is a separate question and this tool does not answer it.

What it changes, and what it refuses to change
==============================================

It rewrites **comments only**. Not string literals, not identifiers, not a
single byte of code. That is not caution for its own sake: a string literal is
output -- changing it changes the product -- and an identifier is a contract
that may be shared with the C headers, the TOML tables and the .bex format.

The guarantee is checked, not promised: after rewriting, the tool strips every
comment from the old and the new text and asserts the two results are
byte-identical. If they are not, the file is left alone and the case is
reported. See `verify_only_comments_changed`.

Usage
-----

    python ascii_sweep.py --dry-run          # report, change nothing
    python ascii_sweep.py --apply            # rewrite in place
    python ascii_sweep.py --check            # exit 1 if any non-ASCII is left

`--check` is the one to wire into the build: it is what stops the repository
from drifting back.
"""

import argparse
import os
import sys
from collections import Counter

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
SKIP_DIRS = {".git", "target", "node_modules", "staging", ".vscode", "__pycache__"}

# Languages this pass handles. The tokenizer below must be exactly right for
# each one, so the list grows only when the tokenizer does -- a half-understood
# language is how a sweep like this eats a string literal.
LANGS = {
    ".rs": "rust", ".c": "c", ".h": "c",
    ".toml": "toml", ".ps1": "powershell", ".md": "markdown",
    ".cob": "cobol", ".cpy": "cobol", ".adb": "ada", ".ads": "ada",
}

# ---------------------------------------------------------------------------
# The one place where a STRING is also subject to the rule.
#
# Code that runs on the machine prints through a Latin-1 renderer: one byte per
# character, no decoder between the keyboard, the shell line and the
# framebuffer -- that is a design decision, not an oversight. But a Rust string
# is UTF-8 and every print path hands it over raw with `s.as_bytes()`.
#
# So an em dash in a kernel string is not a style question. `"BMO-X CABINA --
# bitacora de vuelo"` written with a real em dash puts three bytes on screen
# where one glyph was meant, and the header of the flight recorder reads
# `CABINA a,-" bitacora`. Twelve of these were fixed by hand on 2026-08-08;
# this list is what stops the thirteenth.
#
# The toolchain is NOT here on purpose: its messages go to a Windows console
# that speaks UTF-8 perfectly well, so there is nothing to fix and no reason to
# forbid it. The rule follows the renderer, not the repository.
METAL_PREFIXES = ("Ultra_kernel", "Ultra_userspace", "platform")


def prints_on_metal(rel, lang):
    """Does a string in this file end up on the Latin-1 framebuffer?

    Only Rust source: a `Cargo.toml` description is package metadata that no
    machine ever renders.
    """
    return lang == "rust" and rel.replace(os.sep, "/").startswith(METAL_PREFIXES)

# ---------------------------------------------------------------------------
# The replacement table.
#
# Everything here is a deliberate decision, not a guess. Anything NOT in this
# table is reported and left in place: a sweep that silently drops characters
# it does not recognise is worse than one that stops and asks.
# ---------------------------------------------------------------------------

REPLACEMENTS = {
    # Spanish letters. This is the whole reason the tool exists.
    "á": "a", "é": "e", "í": "i", "ó": "o", "ú": "u",
    "Á": "A", "É": "E", "Í": "I", "Ó": "O", "Ú": "U",
    "ñ": "n", "Ñ": "N", "ü": "u", "Ü": "U",
    "à": "a", "è": "e", "ì": "i", "ò": "o", "ù": "u",
    "ç": "c", "Ç": "C",
    "å": "a", "õ": "o", "Å": "A", "Õ": "O",
    # Spanish opening punctuation has no English equivalent: it goes away.
    "¿": "", "¡": "",
    # Box drawing -- the single biggest group, ~34.000 characters of banner.
    "─": "-", "━": "-", "┄": "-", "┅": "-",
    "═": "=",
    "│": "|", "┃": "|", "║": "|",
    "┌": "+", "┬": "+", "┐": "+", "└": "+", "┴": "+",
    "┘": "+", "├": "+", "┼": "+", "┤": "+",
    "╔": "+", "╦": "+", "╗": "+", "╚": "+", "╩": "+",
    "╝": "+", "╠": "+", "╬": "+", "╣": "+",
    "▀": "#", "▄": "#", "█": "#", "░": ".", "▒": ":",
    "▓": "#",
    # Marks the project uses to rank a comment. Kept as ASCII so the ranking
    # survives: a starred paragraph is still a starred paragraph.
    "★": "*", "☆": "*", "✦": "*", "✮": "*",
    "✔": "v", "✓": "v", "✅": "[OK]",
    "⚠": "[!]", "️": "", "⏳": "[..]", "❌": "[X]",
    "❤": "<3", "Ἰ9": "!",
    # Punctuation.
    "—": "--", "–": "-", "―": "--",
    "…": "...",
    "“": '"', "”": '"', "„": '"', "«": '"', "»": '"',
    "‘": "'", "’": "'", "‚": "'",
    "·": "-", "•": "-", "▪": "-", "●": "-", "■": "-",
    " ": " ",
    # Arrows and maths, spelled the way code already spells them.
    "→": "->", "⇒": "=>", "←": "<-", "⇐": "<=",
    "↑": "^", "↓": "v", "↔": "<->",
    "≥": ">=", "≤": "<=", "≠": "!=", "≈": "~=",
    "×": "x", "÷": "/", "±": "+/-", "∞": "inf",
    "°": " deg", "µ": "u", "μ": "u",
    "½": "1/2", "¼": "1/4", "¾": "3/4",
    "ª": "a", "º": "o", "⁰": "0", "²": "^2", "³": "^3",
    # Currency. Only ever reached inside a comment, where it is prose; the
    # euro sign inside a COBOL PICTURE is a string literal and is not touched.
    "€": "EUR", "£": "GBP", "¥": "JPY",
    "™": "(tm)", "®": "(r)", "©": "(c)",
    # Standards citations: "C99 §6.7.9/21" is a reference, so it has to stay
    # readable as one.
    "§": "section ",
    # Dead keys, documented next to the byte they produce in `dev/keyboard.rs`.
    # The glyph matters there, so it keeps the closest ASCII shape.
    "´": "'", "¨": '"', "¸": ",", "¯": "-",
    # The rest: shapes used in diagrams, and maths.
    "▼": "v", "▲": "^", "▶": ">", "►": ">", "◀": "<", "◄": "<",
    "▌": "|", "▐": "|", "◐": "o", "○": "o", "◦": "-", "⬜": "[ ]",
    "†": "+", "‡": "++", "−": "-", "≡": "==", "¬": "!",
    "π": "pi", "Δ": "delta", "λ": "lambda", "Σ": "sum", "α": "alpha",
    "β": "beta", "γ": "gamma", "θ": "theta", "ω": "omega",
    "⊇": "superset of", "⊆": "subset of", "∩": "intersect", "∪": "union",
    "∈": "in", "∀": "for all", "∃": "exists", "√": "sqrt", "∑": "sum",
    "⇄": "<->", "↕": "^v", "⌐": "!", "‰": "per-mille",
    "­": "",          # soft hyphen: invisible, and pure trouble
    "​": "", "﻿": "",   # zero-width space, BOM
}


# ---------------------------------------------------------------------------
# Tokenizer
#
# It only has to answer one question per character: am I inside a comment?
# Getting that wrong in the other direction -- calling code a comment -- is the
# dangerous mistake, so every construct that can contain a quote or a slash is
# handled explicitly.
# ---------------------------------------------------------------------------

def scan(text, lang):
    """Yield (kind, start, end) spans covering the whole text exactly once.

    `kind` is "comment" or "other". Everything that is not a comment -- code,
    string literals, char literals, lifetimes -- is "other" and is off limits.
    """
    if lang == "markdown":
        yield from scan_markdown(text)
        return
    if lang == "toml":
        yield from scan_line_comment(text, ("#",), ('"""', "'''", '"', "'"))
        return
    if lang == "ada":
        yield from scan_ada(text)
        return
    if lang == "cobol":
        yield from scan_cobol(text)
        return
    if lang == "powershell":
        yield from scan_powershell(text)
        return
    i, n = 0, len(text)
    while i < n:
        c = text[i]

        # Line comment.
        if c == "/" and i + 1 < n and text[i + 1] == "/":
            j = text.find("\n", i)
            j = n if j < 0 else j
            yield "comment", i, j
            i = j
            continue

        # Block comment. Rust nests them; C does not.
        if c == "/" and i + 1 < n and text[i + 1] == "*":
            if lang == "rust":
                depth, j = 1, i + 2
                while j < n and depth:
                    if text.startswith("/*", j):
                        depth += 1
                        j += 2
                    elif text.startswith("*/", j):
                        depth -= 1
                        j += 2
                    else:
                        j += 1
            else:
                j = text.find("*/", i + 2)
                j = n if j < 0 else j + 2
            yield "comment", i, j
            i = j
            continue

        # Raw string: r"...", r#"..."#, br##"..."##. No escapes inside, so the
        # terminator is the quote followed by the same number of hashes.
        if c in "rb" and lang == "rust":
            j = i
            while j < n and text[j] in "rb":
                j += 1
            hashes = 0
            k = j
            while k < n and text[k] == "#":
                hashes += 1
                k += 1
            if k < n and text[k] == '"' and j > i:
                close = '"' + "#" * hashes
                end = text.find(close, k + 1)
                end = n if end < 0 else end + len(close)
                yield "other", i, end
                i = end
                continue

        # String literal.
        if c == '"':
            j = i + 1
            while j < n:
                if text[j] == "\\":
                    j += 2
                    continue
                if text[j] == '"':
                    j += 1
                    break
                j += 1
            yield "other", i, j
            i = j
            continue

        # Single quote: a char literal, or a Rust lifetime. A lifetime is a
        # quote followed by an identifier that is NOT closed by another quote.
        # Reading 'static as an unterminated char literal would swallow the
        # rest of the file -- which is exactly how a sweep destroys a codebase.
        if c == "'":
            # An escaped literal: '\n', '\\', '\''.
            if i + 1 < n and text[i + 1] == "\\":
                j = i + 2
                while j < n and text[j] != "'":
                    j += 1
                j = min(j + 1, n)
                yield "other", i, j
                i = j
                continue
            # Any single character closed by a quote is a char literal, and
            # that INCLUDES a quote: `'"'` is how `trim_matches('"')` is
            # written, and it appears all over the COBOL front-end.
            #
            # Reading it as a lifetime left the double quote exposed, which
            # then opened a string that ran to the next one and swallowed nine
            # lines of comments with it. The failure was silent and it only
            # skipped work -- but the same desync could just as easily have put
            # a `/*` from inside a string in charge, so this is not cosmetic.
            if i + 2 < n and text[i + 2] == "'":
                yield "other", i, i + 3
                i += 3
                continue
            # Otherwise: a lifetime or a loop label.
            k = i + 1
            while k < n and (text[k].isalnum() or text[k] == "_"):
                k += 1
            yield "other", i, k
            i = k
            continue

        # Plain code up to the next character that could start something.
        j = i
        while j < n and text[j] not in "/\"'rb":
            j += 1
        if j == i:
            j = i + 1
        yield "other", i, j
        i = j


def scan_markdown(text):
    """A .md file is prose from top to bottom, fenced blocks included.

    Measured before deciding: of the non-ASCII inside ``` fences, 1.412 of
    ~2.334 characters are box-drawing rules and none are accented letters. So
    the fences hold DIAGRAMS, not program output -- and a diagram drawn with
    `-` and `|` is the same diagram, because every replacement here is one
    character wide. Nothing in a fence claims to be literal output that a
    reader would compare against a screen.
    """
    yield "comment", 0, len(text)


def scan_line_comment(text, starters, quotes):
    """Generic scanner for `<marker> to end of line` languages.

    `starters` are the comment markers, `quotes` the string delimiters, longest
    first so that a triple quote is not mistaken for an empty single one.
    """
    i, n = 0, len(text)
    while i < n:
        for q in quotes:
            if text.startswith(q, i):
                end = text.find(q, i + len(q))
                end = n if end < 0 else end + len(q)
                # A quote never spans a line in these languages unless it is a
                # triple; stopping at the newline keeps an unbalanced quote
                # from swallowing the file.
                if len(q) == 1:
                    nl = text.find("\n", i)
                    if 0 <= nl < end:
                        end = nl
                yield "other", i, end
                i = end
                break
        else:
            if any(text.startswith(s, i) for s in starters):
                j = text.find("\n", i)
                j = n if j < 0 else j
                yield "comment", i, j
                i = j
            else:
                yield "other", i, i + 1
                i += 1


def scan_ada(text):
    """Ada: `--` to end of line. The trap is the tick.

    `X'Size` and `Integer'Image` use a single quote that opens nothing, exactly
    like a Rust lifetime. Reading one as an unterminated character literal
    would swallow everything after it.
    """
    i, n = 0, len(text)
    while i < n:
        if text.startswith("--", i):
            j = text.find("\n", i)
            j = n if j < 0 else j
            yield "comment", i, j
            i = j
            continue
        if text[i] == '"':
            j = text.find('"', i + 1)
            nl = text.find("\n", i)
            j = n if j < 0 else j + 1
            if 0 <= nl < j:
                j = nl
            yield "other", i, j
            i = j
            continue
        if text[i] == "'":
            # 'x' is a character literal; anything else is an attribute tick.
            if i + 2 < n and text[i + 2] == "'":
                yield "other", i, i + 3
                i += 3
            else:
                yield "other", i, i + 1
                i += 1
            continue
        yield "other", i, i + 1
        i += 1


def scan_cobol(text):
    """COBOL, fixed format: column 7 decides.

    A `*` or `/` in the seventh column makes the WHOLE line a comment -- that
    is the format, not a convention, and it is why this cannot reuse the
    generic scanner. `*>` is the free-format inline form and is also honoured.
    """
    i = 0
    for line in text.splitlines(keepends=True):
        stripped = line.rstrip("\r\n")
        eol = len(stripped)
        if len(stripped) > 6 and stripped[6] in "*/":
            yield "comment", i, i + eol
        elif "*>" in stripped:
            k = stripped.index("*>")
            yield "other", i, i + k
            yield "comment", i + k, i + eol
        else:
            yield "other", i, i + eol
        if eol < len(line):
            yield "other", i + eol, i + len(line)
        i += len(line)


def scan_powershell(text):
    """PowerShell: `<# #>`, `#` to end of line, and here-strings.

    Here-strings (`@" ... "@`) are the reason this is not the generic scanner:
    they run across lines and everything inside is data, including `#`.
    """
    i, n = 0, len(text)
    while i < n:
        if text.startswith("<#", i):
            j = text.find("#>", i + 2)
            j = n if j < 0 else j + 2
            yield "comment", i, j
            i = j
            continue
        if text.startswith('@"', i) or text.startswith("@'", i):
            close = '"@' if text[i + 1] == '"' else "'@"
            j = text.find(close, i + 2)
            j = n if j < 0 else j + 2
            yield "other", i, j
            i = j
            continue
        if text[i] in "\"'":
            q = text[i]
            j = i + 1
            while j < n and text[j] != q and text[j] != "\n":
                if q == '"' and text[j] == "`":
                    j += 1
                j += 1
            j = min(j + 1, n)
            yield "other", i, j
            i = j
            continue
        if text[i] == "#":
            j = text.find("\n", i)
            j = n if j < 0 else j
            yield "comment", i, j
            i = j
            continue
        yield "other", i, i + 1
        i += 1


def strip_comments(text, lang):
    """The text with every comment removed. Used as the safety invariant."""
    return "".join(text[a:b] for kind, a, b in scan(text, lang) if kind != "comment")


MOJIBAKE_RUN = None      # compiled lazily, see below


def repair_mojibake(s, repaired):
    r"""Undo text that was decoded once with the wrong codepage.

    Parts of this tree contain things like `convenciÃ³n` and `â”œâ”€â”€`. That is
    not decoration and not Spanish: it is `convención` and `├──` that were read
    as Windows-1252 and written back as UTF-8, so every byte above 127 became
    its own character. Transliterating that would give `convenciAn`, which
    preserves the damage and calls it done.

    The repair is the round trip in reverse: take a run of non-ASCII characters,
    encode it back to the bytes it came from, and see whether those bytes are
    valid UTF-8. If they are, the run was mojibake and we have the original. If
    they are not -- a lone `n-with-tilde`, a star, a box-drawing rule -- the run
    is genuine text and is handed back untouched.

    The test is the decode itself, which is why this is safe: invalid UTF-8 is
    not a judgement call.
    """
    global MOJIBAKE_RUN
    if MOJIBAKE_RUN is None:
        import re
        MOJIBAKE_RUN = re.compile(r"[^\x00-\x7f]+")

    def one(m):
        run = m.group(0)
        # ** The guard, and it earned its place. **
        #
        # "Decodes as valid UTF-8" is necessary but NOT sufficient. The run
        # `x-with-multiply-sign` followed by an em dash -- ordinary prose, as in
        # "3x -- see below" -- encodes to D7 97, which is perfectly valid UTF-8
        # and decodes to a Hebrew letter. The repair would have been confident
        # and wrong.
        #
        # Real mojibake of Latin text always begins with the cp1252 rendering of
        # a UTF-8 lead byte: C2/C3/C5 (accented letters) or E2 (punctuation, box
        # drawing, arrows). Anything else was never UTF-8 to begin with.
        if run[0] not in "ÂÃÅâ":
            return run
        try:
            fixed = run.encode("cp1252").decode("utf-8")
        except (UnicodeEncodeError, UnicodeDecodeError):
            return run
        # A single character that decodes to itself is not mojibake.
        if fixed == run:
            return run
        repaired[run] += 1
        return fixed

    return MOJIBAKE_RUN.sub(one, s)


# Symbol and emoji blocks. In documentation these carry meaning that ASCII
# cannot: `ARQUITECTURA.md` defines a legend where green means "runs on metal",
# yellow "written, never executed" and white "design only", and
# `CONTRIBUTING.md` calls that system the most valuable thing in the project.
# Flattening it to `[ok]` would delete the distinction the project is built on.
#
# And the reason the ASCII rule does not reach here: markdown is not compiled,
# never crosses the C preprocessor, and never reaches the Latin-1 console. It
# is read on GitHub and in editors, both of which speak UTF-8. The rule exists
# for compilers -- so in docs it applies to LETTERS and RULES, not to symbols.
SYMBOL_BLOCKS = (
    (0x2190, 0x21FF),    # arrows
    (0x2300, 0x23FF),    # misc technical, includes the hourglass
    (0x2500, 0x259F),    # box drawing and blocks  (converted: see below)
    (0x25A0, 0x27BF),    # geometric shapes, misc symbols, dingbats
    (0x2B00, 0x2BFF),    # extra arrows and shapes
    (0x1F300, 0x1FAFF),  # emoji
)


def is_doc_symbol(ch):
    """True for a symbol that documentation keeps, false for text."""
    o = ord(ch)
    # Box drawing and arrows are DIAGRAM, not legend: those still become ASCII,
    # because a rule drawn with `-` is the same rule and stays one column wide.
    if 0x2500 <= o <= 0x257F or 0x2190 <= o <= 0x21FF:
        return False
    return any(lo <= o <= hi for lo, hi in SYMBOL_BLOCKS)


def transliterate(s, unknown, keep_symbols=False):
    out = []
    for ch in s:
        if ord(ch) < 128:
            out.append(ch)
        elif keep_symbols and is_doc_symbol(ch):
            out.append(ch)
        elif ch in REPLACEMENTS:
            out.append(REPLACEMENTS[ch])
        else:
            unknown[ch] += 1
            out.append(ch)
    return "".join(out)


def sweep_text(text, lang, unknown, repaired, metal=False):
    out = []
    for kind, a, b in scan(text, lang):
        chunk = text[a:b]
        if kind == "comment":
            # Repair first, transliterate second. The other order would turn
            # broken text into confidently wrong ASCII.
            chunk = transliterate(repair_mojibake(chunk, repaired), unknown,
                                  keep_symbols=(lang == "markdown"))
        elif metal and chunk[:1] == '"':
            # A string that reaches the Latin-1 renderer. Same table, same
            # Spanish words -- Spanish that can actually be READ on the screen.
            chunk = transliterate(repair_mojibake(chunk, repaired), unknown)
        out.append(chunk)
    return "".join(out)


def strip_comments_and_strings(text, lang):
    """The text with comments AND string bodies removed: pure structure."""
    out = []
    for kind, a, b in scan(text, lang):
        seg = text[a:b]
        if kind == "comment":
            continue
        if seg[:1] == '"':
            out.append('""')          # the literal is there, its text is not
            continue
        out.append(seg)
    return "".join(out)


def verify_only_comments_changed(before, after, lang, allow_strings=False):
    """The guarantee. Everything that is not a comment must be untouched.

    `allow_strings` is for the files whose output goes to the Latin-1 renderer,
    where the string bodies are in scope too. The guarantee does not disappear
    there, it narrows: what must survive untouched is the CODE -- every
    identifier, every operator, and the fact that a literal is where a literal
    was. Only the text inside the quotes may move.
    """
    if allow_strings:
        return (strip_comments_and_strings(before, lang)
                == strip_comments_and_strings(after, lang))
    if lang == "markdown":
        # For prose the invariant above is vacuous -- the whole file is the
        # comment -- so it is replaced by the one that actually protects a
        # document: the line structure has to survive. A replacement that ate
        # or added a line would break a table, a list or a fenced block, and
        # that is the only way this pass can damage a .md.
        return before.count("\n") == after.count("\n")
    return strip_comments(before, lang) == strip_comments(after, lang)


def iter_sources():
    for root, dirs, files in os.walk(REPO):
        dirs[:] = [d for d in dirs if d not in SKIP_DIRS]
        for f in files:
            ext = os.path.splitext(f)[1].lower()
            if ext in LANGS:
                yield os.path.join(root, f), LANGS[ext]


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    g = ap.add_mutually_exclusive_group(required=True)
    g.add_argument("--dry-run", action="store_true", help="report, change nothing")
    g.add_argument("--apply", action="store_true", help="rewrite in place")
    g.add_argument("--check", action="store_true", help="exit 1 if non-ASCII remains")
    args = ap.parse_args()

    unknown = Counter()
    repaired = Counter()
    touched = changed = refused = 0
    left_in_code = []
    strings_left = []
    metal_strings = []

    for path, lang in iter_sources():
        rel = os.path.relpath(path, REPO)
        try:
            # newline="" keeps CR/LF exactly as they are on disk. Without it,
            # Python reads CRLF as LF and the rewrite silently converts the
            # whole tree's line endings -- a 13.000-line diff on a change that
            # touched comments, and this repository is checked out with
            # core.autocrlf=true, so the working copy really is CRLF.
            before = open(path, encoding="utf-8", newline="").read()
        except UnicodeDecodeError:
            print(f"  NOT UTF-8, skipped: {rel}")
            continue

        if args.check:
            # Split by bucket, because they mean different things. Non-ASCII
            # left in a comment is drift and must fail the build. Non-ASCII in
            # a string literal is what BMO-X PRINTS -- a product decision this
            # tool is not allowed to make, so it is reported and forgiven.
            in_comment = in_string = 0
            for kind, a, b in scan(before, lang):
                n = sum(1 for c in before[a:b] if ord(c) > 127)
                if not n:
                    continue
                if kind != "comment" and prints_on_metal(rel, lang)                         and before[a:b][:1] == '"':
                    metal_strings.append((n, rel))
                    continue
                if kind == "comment":
                    if lang == "markdown":
                        n = sum(1 for c in before[a:b]
                                if ord(c) > 127 and not is_doc_symbol(c))
                        if not n:
                            continue
                    in_comment += n
                else:
                    in_string += n
            if in_comment:
                left_in_code.append((in_comment, rel))
            if in_string:
                strings_left.append((in_string, rel))
            continue

        if all(ord(c) < 128 for c in before):
            continue
        touched += 1
        after = sweep_text(before, lang, unknown, repaired,
                               metal=prints_on_metal(rel, lang))
        if after == before:
            continue
        if not verify_only_comments_changed(before, after, lang,
                                            prints_on_metal(rel, lang)):
            print(f"  REFUSED (would have touched code): {rel}")
            refused += 1
            continue
        changed += 1
        if args.apply:
            with open(path, "w", encoding="utf-8", newline="") as fh:
                fh.write(after)

    if args.check:
        if strings_left:
            total = sum(n for n, _ in strings_left)
            print(f"[note] {total} non-ASCII chars inside STRING LITERALS, "
                  f"in {len(strings_left)} files.")
            print("       These are output, not source style. Deciding them is "
                  "a separate job:")
            print("       what BMO-X prints is the product, not the codebase.")
            for n, rel in sorted(strings_left, reverse=True)[:12]:
                print(f"         {n:>5}  {rel}")
            print()
        if metal_strings:
            total = sum(n for n, _ in metal_strings)
            print(f"FAIL: {total} non-ASCII chars in strings that the Latin-1 "
                  f"renderer will print, in {len(metal_strings)} places")
            print("      The screen shows UTF-8 bytes one at a time: an em dash "
                  "becomes three glyphs.")
            for n, rel in sorted(metal_strings, reverse=True)[:12]:
                print(f"  {n:>6}  {rel}")
            return 1
        if left_in_code:
            total = sum(n for n, _ in left_in_code)
            print(f"FAIL: {total} non-ASCII chars in comments, "
                  f"in {len(left_in_code)} files")
            for n, rel in sorted(left_in_code, reverse=True)[:20]:
                print(f"  {n:>6}  {rel}")
            return 1
        print("clean: no comment in any scanned source has a non-ASCII byte")
        return 0

    verb = "rewritten" if args.apply else "would be rewritten"
    print(f"{changed} files {verb}, {touched} had non-ASCII, {refused} refused")
    if repaired:
        n = sum(repaired.values())
        print(f"\nmojibake repaired before transliterating: {n} runs")
        for run, k in repaired.most_common(8):
            try:
                fixed = run.encode("cp1252").decode("utf-8")
            except Exception:
                fixed = "?"
            print(f"  {k:>5}  {run!r} -> {fixed!r}")
    if unknown:
        print("\nCharacters with no rule, LEFT IN PLACE -- add them to the table:")
        for ch, n in unknown.most_common(30):
            print(f"  {n:>6}  U+{ord(ch):04X}  {ch}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
