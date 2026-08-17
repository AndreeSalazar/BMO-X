#!/usr/bin/env python3
"""enlaces -- every reference to a document must resolve to a real file.

Why this exists
===============

The tree does not only cite documents from other documents: it cites them from
kernel source, from `Cargo.toml`, from `build.ps1` and from C examples. There
are around a hundred and thirty such citations. Nothing checked them.

The first sweep that looked found one that had never resolved: a chapter cited
AVANCES.md inside docs/, while that file has always lived at the root of the
repo. It was not a typo made once and noticed -- it was a pointer that had been wrong
for as long as it had existed, in a file whose whole job is to send the reader
somewhere else.

That is the failure this tool exists for, and the reason it is a guardian and
not a one-off cleanup. Documents get renamed and moved; the citations do not
move with them, and a citation does not fail loudly. It sends a reader to
nothing and the reader assumes the document was never written.

    L4 of META-KERNEL_HARD: a rule is proven by saying NO. This tool was
    proven by pointing a link at a file that does not exist and checking that
    it named the file and the line. See the commit that introduced it.

What counts as a citation
=========================

Three shapes, and each resolves differently. The difference is not cosmetic --
resolving them all the same way is what would produce false alarms:

  1. A markdown link -- bracketed text, then the path in parentheses ending
     in .md -- **inside a `.md` file**. Resolved relative to the file that
     contains it, which is what a markdown renderer does. This is the only shape a reader can click, so a broken one
     is the most expensive.

     The "inside a `.md` file" half of that is not a detail, and it is not an
     exception list -- it is the difference between a guardian and a nuisance.
     `toolchain/tools/c-gen/generate_cpp.py` *emits* markdown that lands in
     `toolchain/lang/cpp/`; its `](MAESTROS.md)` resolves **at the
     destination**, and resolving it next to the generator reports four breaks
     that are not breaks. So outside `.md`, a link is treated as shape 3: the
     name has to exist, but not at a computed path. A guardian that cries wolf
     gets switched off, and then it protects nothing.

  2. A backticked path that contains a slash, `` `docs/identidad/LA_RAM.md` ``.
     Tried against the repo root first, then relative to the citing file.
     Either is legitimate in this tree and both are used.

  3. A bare backticked name, `` `PLAN_VULKAN.md` ``. This one cannot be
     resolved by path, because the convention of this tree is that a document
     about a piece of code **lives next to that code** -- `PLAN_VULKAN.md` is
     in `platform/drivers/gpu/rdna4/`, and it is cited from four places that
     are nowhere near it. So the check is weaker on purpose: the basename must
     exist somewhere in the repository. That still catches a rename, which is
     the thing that actually happens.

What it deliberately does NOT check
===================================

Anchors (`#section`). A link to a heading that no longer exists is a real
defect, but checking it means parsing every heading and slugifying it the way
the renderer does; getting that subtly wrong produces false alarms, and a
guardian that cries wolf gets disabled. Named and not done, which is different
from forgotten.

Usage
-----

    python enlaces.py --check     # report and exit 1 if anything is broken
    python enlaces.py             # same, but always exit 0 (survey mode)
"""

import argparse
import os
import re
import subprocess
import sys

# Un enlace markdown: corchetes, parentesis y una ruta que acaba en .md, con o
# sin ancla detras. El ancla se descarta al resolver.
LINK_MD = re.compile(r"\]\(\s*([^)\s#]+\.md)(?:#[^)\s]*)?\s*\)")

# Una ruta entre backticks que acaba en .md, con o sin barras. El backtick es lo
# que la distingue de una frase que casualmente acabe en esas tres letras.
#
# ** Y por eso los ejemplos de este fichero van SIN backticks: este guardian no
# sabe distinguir una cita de la CITA DE UNA CITA ROTA, y tiene razon. Se cazo a
# si mismo aqui el dia que se escribio.
TICK_MD = re.compile(r"`([A-Za-z0-9_./\\-]+\.md)`")

# Se barre lo que git tiene registrado: los artefactos generados y `target/` no
# son fuentes y sus citas no las mantiene nadie.
EXTS = (".md", ".rs", ".toml", ".ps1", ".py", ".c", ".h", ".txt")


def tracked_files(root):
    out = subprocess.run(
        ["git", "-C", root, "ls-files"],
        capture_output=True, text=True, check=True,
    ).stdout.splitlines()
    return [f for f in out if f.endswith(EXTS)]


def index_basenames(root):
    """basename -> cuantas veces aparece en el repo."""
    seen = {}
    out = subprocess.run(
        ["git", "-C", root, "ls-files"],
        capture_output=True, text=True, check=True,
    ).stdout.splitlines()
    for f in out:
        if f.endswith(".md"):
            b = os.path.basename(f)
            seen[b] = seen.get(b, 0) + 1
    return seen


def resolve(root, citing, target, kind, basenames):
    """Devuelve None si resuelve, o el motivo por el que no."""
    target = target.replace("\\", "/")
    here = os.path.dirname(citing)

    # Un enlace solo se resuelve por ruta si esta EN un `.md`: alli el sitio del
    # fichero es lo que usa el que hace clic. En una fuente puede ser texto que
    # se genera para aterrizar en otra carpeta, y entonces la ruta de aqui no
    # dice nada. Ver la cabecera.
    if kind == "link" and citing.endswith(".md"):
        p = os.path.normpath(os.path.join(root, here, target))
        return None if os.path.isfile(p) else "no existe desde " + (here or ".")

    if "/" in target:
        # Ruta con barras: vale desde la raiz o desde el fichero que la cita.
        desde_raiz = os.path.normpath(os.path.join(root, target))
        desde_aqui = os.path.normpath(os.path.join(root, here, target))
        if os.path.isfile(desde_raiz) or os.path.isfile(desde_aqui):
            return None
        return "no existe ni desde la raiz ni desde " + (here or ".")

    # Nombre pelado: la convencion permite que viva junto a su codigo, asi que
    # solo se exige que exista en alguna parte.
    return None if basenames.get(target) else "ese nombre no existe en el repo"


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--check", action="store_true",
                    help="salir con 1 si hay alguna cita rota")
    args = ap.parse_args()

    root = subprocess.run(
        ["git", "rev-parse", "--show-toplevel"],
        capture_output=True, text=True, check=True,
    ).stdout.strip()

    basenames = index_basenames(root)
    roto = []
    total = 0

    for rel in tracked_files(root):
        full = os.path.join(root, rel)
        try:
            with open(full, "r", encoding="utf-8", errors="replace") as fh:
                lineas = fh.readlines()
        except OSError:
            continue

        for n, linea in enumerate(lineas, 1):
            for m in LINK_MD.finditer(linea):
                total += 1
                porque = resolve(root, rel, m.group(1), "link", basenames)
                if porque:
                    roto.append((rel, n, m.group(1), porque))
            for m in TICK_MD.finditer(linea):
                total += 1
                porque = resolve(root, rel, m.group(1), "tick", basenames)
                if porque:
                    roto.append((rel, n, m.group(1), porque))

    if roto:
        print("citas a documentos que NO resuelven:")
        for rel, n, target, porque in roto:
            print("  %s:%d  ->  %s   (%s)" % (rel, n, target, porque))
        print("")
        print("%d citas rotas de %d" % (len(roto), total))
        return 1 if args.check else 0

    print("clean: las %d citas a documentos resuelven" % total)
    return 0


if __name__ == "__main__":
    sys.exit(main())
