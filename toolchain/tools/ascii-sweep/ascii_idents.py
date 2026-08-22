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

# Any identifier at all. Whether it is ASCII is decided AFTERWARDS.
#
# ** This used to be `[A-Za-z_À-ɏ]...`, a range that covers accented
# Latin and NOTHING ELSE. It was built for the real risk of the day --Spanish
# accents-- and it was honest about that; the consequence was not. An
# identifier in Cyrillic, Greek or CJK **did not even match**, so the sweep
# walked past it and printed `clean`.
#
# That happened on 2026-08-21: a Cyrillic letter slipped into a test file and
# this tool reported no findings. A guard that answers `clean` for a case it
# cannot see is worse than no guard, because someone believed it.
IDENT = re.compile(r"[^\W\d]\w*", re.UNICODE)


def deaccent(name):
    return "".join(REPLACEMENTS.get(c, c) for c in name)


def se_puede_traducir(name):
    """Does de-accenting actually produce an ASCII name?

    ** `deaccent` only knows the Latin table, so for anything else it hands
    back the same non-ASCII name. Rewriting with it would be a no-op that
    reports success -- which is the failure mode this whole file exists to
    prevent.
    """
    return all(ord(c) < 128 for c in deaccent(name))


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
    # ** Two piles, and they are not the same problem. One this tool can fix;
    # the other it can only report -- and saying so is the point.
    traducibles = {n: p for n, p in found.items() if se_puede_traducir(n)}
    a_mano = {n: p for n, p in found.items() if not se_puede_traducir(n)}

    for name in sorted(traducibles):
        print(f"  {name}  ->  {deaccent(name)}   ({len(found[name])} files)")
    for name in sorted(a_mano):
        # ** El nombre se pinta ESCAPADO, y no es una concesion a la consola.
        # Una `a` cirilica es identica a una latina: pintarla tal cual no
        # ayudaria a encontrarla. `c\\u0430ja` dice exactamente cual es la
        # impostora y en que posicion.
        visible = name.encode("unicode_escape").decode("ascii")
        print(f"  {visible}  ->  NOT LATIN: rename by hand   ({len(found[name])} files)")
        for path in sorted(found[name]):
            print(f"      {path}")

    if a_mano and not traducibles:
        print(chr(10) + "Nothing to rewrite automatically: these are not accented Latin.")
        return 1
    if not apply:
        print(chr(10) + "(dry run -- pass --apply to rewrite, then cargo check)")
        return 0
    found = traducibles

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
