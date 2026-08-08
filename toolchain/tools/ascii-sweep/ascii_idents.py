#!/usr/bin/env python3
"""ascii_idents -- de-accent Rust identifiers. Same spelling, ASCII letters.

This is the companion to `ascii_sweep` and it stops exactly where that one
does: it does NOT translate. `senalando` stays `senalando`; turning it into
`pointing` is a different job with different risks, and mixing the two would
make the diff impossible to review.

Why it is needed at all
=======================

`ascii_sweep` proved its guarantee by refusing to touch anything outside a
comment -- which left a real case behind. In `platform/drivers/usb/input/foco.rs`
the tilde is not in a comment and not in a string: it is in the NAMES.

    senalando: Option<usize>          <- the field
    pub fn senalada(&self) -> ...     <- the method

Rust accepts Unicode identifiers, so this compiled and nobody noticed. But once
the comments around them were swept to ASCII, the comment and the code spelled
the same word two different ways -- and grep stopped finding one from the other.

What it will not touch
======================

Strings, because those are output. Comments, because `ascii_sweep` already did
them. Only identifiers, and only their accents.

The check that makes it safe is the compiler: a rename that misses an
occurrence, or invents one, does not build. So the verification is
`cargo check --workspace`, and it is not optional.
"""

import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from ascii_sweep import REPO, SKIP_DIRS, LANGS, scan, REPLACEMENTS

# An identifier character that is a letter but not ASCII.
IDENT = re.compile(r"[A-Za-z_À-ɏ][A-Za-z0-9_À-ɏ]*")


def deaccent(name):
    return "".join(REPLACEMENTS.get(c, c) for c in name)


def collect():
    """Every non-ASCII identifier that appears in code position, with its file."""
    found = {}
    for root, dirs, files in os.walk(REPO):
        dirs[:] = [d for d in dirs if d not in SKIP_DIRS]
        for f in files:
            if os.path.splitext(f)[1].lower() != ".rs":
                continue
            path = os.path.join(root, f)
            text = open(path, encoding="utf-8", newline="").read()
            for kind, a, b in scan(text, "rust"):
                if kind == "comment":
                    continue
                seg = text[a:b]
                # A string literal is output: skip it whole.
                if seg[:1] in ('"', "'") or seg[:2] in ('r"', 'b"'):
                    continue
                for m in IDENT.finditer(seg):
                    name = m.group(0)
                    if any(ord(c) > 127 for c in name):
                        found.setdefault(name, set()).add(path)
    return found


def main():
    apply = "--apply" in sys.argv
    found = collect()
    if not found:
        print("no non-ASCII identifiers left")
        return 0

    print(f"{len(found)} non-ASCII identifiers, in "
          f"{len({p for ps in found.values() for p in ps})} files:")
    for name in sorted(found):
        print(f"  {name}  ->  {deaccent(name)}   ({len(found[name])} files)")

    if not apply:
        print("\n(dry run -- pass --apply to rewrite, then cargo check)")
        return 0

    # Rewrite. Comments were already swept to the de-accented spelling, so only
    # code positions can still hold the accented name -- but the replacement is
    # done per span anyway, so a string keeps whatever it says.
    files = {p for ps in found.values() for p in ps}
    pattern = re.compile("|".join(sorted((re.escape(n) for n in found), key=len,
                                         reverse=True)))
    for path in sorted(files):
        text = open(path, encoding="utf-8", newline="").read()
        out = []
        for kind, a, b in scan(text, "rust"):
            seg = text[a:b]
            if kind != "comment" and not (seg[:1] in ('"', "'")
                                          or seg[:2] in ('r"', 'b"')):
                seg = pattern.sub(lambda m: deaccent(m.group(0)), seg)
            out.append(seg)
        with open(path, "w", encoding="utf-8", newline="") as fh:
            fh.write("".join(out))
    print(f"\n{len(files)} files rewritten. Now run: cargo check --workspace")
    return 0


if __name__ == "__main__":
    sys.exit(main())
