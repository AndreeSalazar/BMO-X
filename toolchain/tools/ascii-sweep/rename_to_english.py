#!/usr/bin/env python3
"""rename_to_english -- rename Spanish Rust identifiers to English, by hand.

This is the third and last of the language tools, and it is deliberately the
dumbest of the three: it applies a dictionary that a human wrote, one entry at
a time. There is no morphology, no stemming and no guessing, because the whole
value of a rename is choosing the RIGHT English word and only a reader knows
which one that is. `soltar` is `release`, not `drop`; `ceder` is `yield`;
`reclamar` is `claim`. A stemmer would have got all three wrong.

Three rules, and the third is the interesting one
=================================================

1. **Code positions only.** Strings are output and are never touched.

2. **Whole identifiers.** The dictionary maps a complete name, not a token, so
   `leer` -> `read` never turns `leer_linea` into `read_linea` by accident:
   `leer_linea` has its own entry, `read_line`. Renaming half a name is worse
   than not renaming it.

3. **Inside a comment, only what is inside backticks.** A comment that says
   "`reclamar` le da la pantalla al proceso" has a code reference and prose in
   the same sentence. The reference has to follow the rename or it stops
   pointing at anything; the prose must NOT, or the comments turn into
   Spanglish. So the backtick is the signal, and it is already the convention
   this codebase uses everywhere.

What verifies it
================

`cargo check --workspace --all-targets`. A rename that misses an occurrence does
not build, and one that collides with an existing name does not build either.
That is the entire safety argument, and it is why this tool only touches Rust.

**`--all-targets` is not optional.** Plain `cargo check` does not compile
`#[cfg(test)]` code, and the one real collision this batch produced was in
there: a test helper called `leer_linea` taking three arguments became
`read_line`, which is also the name of the emitter it was testing. Renamed to
`run_read_line`, which says what it actually does -- it RUNS the emitter.

A module rename is also a FILE rename: `prestamo` -> `loan` needs
`git mv prestamo.rs loan.rs`, or the build stops at "file not found for
module".

Contracts are NOT in here
=========================

`INPUT_OP_TECLA`, `KIND_ARCHIVO`, `ARCH_OP_LEER` and the rest of the opcode
names have a twin in `toolchain/forge/sem-asm/tables/bmo/bmo.h`
(`BMO_ENTRADA_TECLA`, ...). Renaming one side and not the other would leave two
names for one number with nothing to catch the drift, so those move in their own
coordinated batch, both sides at once.

    python rename_to_english.py --dry-run
    python rename_to_english.py --apply && cargo check --workspace
"""

import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from ascii_sweep import REPO, SKIP_DIRS, scan

# ---------------------------------------------------------------------------
# Batch 1: the kernel's capability objects, and the verbs used everywhere.
#
# Chosen because they are the vocabulary the rest of the kernel spells out
# loudest -- `fb::reclamar`, `input::soltar`, `prestamo::ofrecer` -- and because
# every one of them is internal Rust, so the compiler checks the whole job.
# ---------------------------------------------------------------------------

RENAMES = {
    # -- Owning a device: claim / release / rescue -------------------------
    "reclamar": "claim",
    "soltar": "release",
    "soltar_buffer": "release_buffer",
    "rescatar": "rescue",
    "ceder": "yield_screen",
    "cedido": "yielded",
    "dueno": "owner",
    "DUENO": "OWNER",
    "SIN_DUENO": "NO_OWNER",
    "DUENO_PRIMERO": "FIRST_OWNER",
    "proceso_muerto": "process_died",

    # -- Lending memory between processes ---------------------------------
    "prestamo": "loan",
    "ofrecer": "offer",
    "tomar": "take",
    "Oferta": "Offer",
    "deshacer": "undo",
    "bytes_mapeados": "mapped_bytes",
    "entregado_por": "handed_over_by",
    "total_entregado": "total_handed_over",
    "procesos_con_memoria": "processes_with_memory",

    # -- The basic verbs, spelled the same way everywhere ------------------
    "abrir": "open",
    "cerrar": "close",
    "crear": "create",
    "leer": "read",
    "leer_en": "read_into",
    "leer_linea": "read_line",
    "leer_entrada": "read_entry",
    "escribir": "write",
    "escribir_en_frame": "write_into_frame",
    "escribir_entrada": "write_entry",
    "guardar": "save",
    "cargar": "load",
    "borrar": "erase",
    "buscar": "find_by",
    "pedir": "request",
    "reservar": "reserve",
    "crecer": "grow",
    "esperar": "wait_for",
    "llamar": "call",
    "responder": "reply_to",
    "publicar": "publish",
    "completar": "complete",
    "conceder_cliente": "grant_client",
    "siguiente": "next",
    "asignar_salida": "assign_output",
    "salida_de": "output_of",
    "hay_hijo": "has_child",

    # -- Nouns ------------------------------------------------------------
    "nombre": "name",
    "operacion": "operation",
    "ranura": "slot",
    "ranura_o_nueva": "slot_or_new",
    "hueco": "free_slot",
    "capacidad": "capacity",
    "perdidos": "dropped",
    "encoladas": "queued",
    "vivos": "alive",
    "clave_endpoint": "endpoint_key",
    "clave_respuesta": "reply_key",
    "ultima_escritura": "last_write",
    "Respuesta": "Reply",
    "Resultado": "Outcome",
    "Llamada": "Call",
    "Cuenta": "Count",

    # -- Constants --------------------------------------------------------
    "LIBRE": "FREE_SLOT",
    "VACIO": "EMPTY",
    "VACIA": "EMPTY_ONE",
    "NADA": "NOTHING",
    "INICIAL": "INITIAL",
    "PAGINA": "PAGE",
    "COLA": "QUEUE",
    "ANILLO": "RING",
    "ERROR_OCUPADO": "ERROR_BUSY",
    "ERROR_NO_ESTA": "ERROR_NOT_THERE",
    "ERROR_SIN_HUECO": "ERROR_NO_FREE_SLOT",
    "ERROR_SIN_RANURA": "ERROR_NO_SLOT",
    "ERROR_SIN_PANTALLA": "ERROR_NO_SCREEN",
    "ERROR_SIN_RAM": "ERROR_NO_RAM",
    "ERROR_SOLO_LECTURA": "ERROR_READ_ONLY",
    "ERROR_ES_CARPETA": "ERROR_IS_DIRECTORY",
    "ERROR_CARPETA": "ERROR_DIRECTORY",
    "ERROR_NOMBRE": "ERROR_NAME",
    "ERROR_DEMASIADAS": "ERROR_TOO_MANY",
    "ERROR_DEMASIADO": "ERROR_TOO_BIG",
    "ERROR_DEMASIADO_GRANDE": "ERROR_TOO_LARGE",

    # -- Focus policy, from `platform/drivers/usb/input/foco.rs` -----------
    "senalando": "pointing_at",
    "senalada": "pointed_at",
    "indice_senalado": "pointed_index",
}

# Longest first, so `leer_linea` is matched before `leer`.
PATTERN = re.compile(
    r"\b(" + "|".join(sorted(map(re.escape, RENAMES), key=len, reverse=True)) + r")\b"
)
# Inside a comment, only a backticked reference is a code reference.
IN_TICKS = re.compile(r"`([^`\n]+)`")


CODE, COMMENT, STRING = 0, 1, 2
STARTS_STRING = re.compile(r'^(?:[bcr]{0,2}#*"|\')')


def classify(text):
    r"""A mask over the text: is each character code, comment or string?

    ** Why a mask and not the spans themselves. **

    The Rust scanner in `ascii_sweep` cuts a run of code at every `/`, `"`, `'`,
    `r` and `b`, because those are the characters that can OPEN something. That
    is correct for deciding what is a comment -- which is all that tool needed
    -- but it means an identifier is routinely split across spans: `nombre`
    arrives as `nom` + `b` + `re`.

    Running the rename per span therefore matched almost nothing that contained
    a lowercase r or b, and the first attempt renamed the backticked references
    inside the comments while leaving the functions themselves alone. It
    compiled -- nothing renamed is still consistent -- and the docs pointed at
    names that did not exist.

    A per-character mask has no such seam: the spans decide the LABEL, and the
    replacement then runs over the whole text.
    """
    mask = bytearray(len(text))
    for kind, a, b in scan(text, "rust"):
        seg = text[a:b]
        if kind == "comment":
            v = COMMENT
        elif STARTS_STRING.match(seg) and len(seg) > 1:
            v = STRING
        else:
            v = CODE
        if v:
            mask[a:b] = bytes([v]) * (b - a)
    return mask


def rewrite_file(text):
    mask = classify(text)
    # Inside a comment only a backticked reference is a code reference, so
    # collect where the backticks are before deciding anything.
    ticked = bytearray(len(text))
    for m in IN_TICKS.finditer(text):
        if mask[m.start()] == COMMENT:
            ticked[m.start(1):m.end(1)] = b"\x01" * (m.end(1) - m.start(1))

    # ** The exception to "never touch a string". **
    #
    # Since Rust 2021, `format!("{name}")` captures the variable `name` from the
    # surrounding scope -- so that identifier lives INSIDE a string literal and
    # is code in every sense that matters. The compiler found twelve of them:
    # rename the variable and not the interpolation, and the build breaks with
    # "cannot find value".
    #
    # So the rule is narrow and exact: inside a string, a name is renamed only
    # when it appears as `{name}` or `{name:spec}`. `{{name}}` is an escaped
    # brace and stays literal, which is what the lookbehind is for.
    interp = bytearray(len(text))
    for m in re.finditer(r"(?<!\{)\{(" + PATTERN.pattern[2:-2] + r")(:[^}\n]*)?\}", text):
        if mask[m.start()] == STRING:
            interp[m.start(1):m.end(1)] = b"\x01" * (m.end(1) - m.start(1))

    def one(m):
        kind = mask[m.start()]
        if kind == STRING:
            return RENAMES[m.group(1)] if interp[m.start()] else m.group(0)
        if kind == COMMENT and not ticked[m.start()]:
            return m.group(0)                     # prose stays Spanish
        return RENAMES[m.group(1)]

    return PATTERN.sub(one, text)


def main():
    apply = "--apply" in sys.argv
    changed = hits = 0
    per_name = {}
    for root, dirs, files in os.walk(REPO):
        dirs[:] = [d for d in dirs if d not in SKIP_DIRS]
        for f in files:
            if not f.endswith(".rs"):
                continue
            path = os.path.join(root, f)
            before = open(path, encoding="utf-8", newline="").read()
            if not PATTERN.search(before):
                continue
            after = rewrite_file(before)
            if after == before:
                continue
            changed += 1
            for m in PATTERN.finditer(before):
                per_name[m.group(1)] = per_name.get(m.group(1), 0) + 1
                hits += 1
            if apply:
                with open(path, "w", encoding="utf-8", newline="") as fh:
                    fh.write(after)

    verb = "rewritten" if apply else "would be rewritten"
    print(f"{len(RENAMES)} names in the dictionary")
    print(f"{changed} files {verb}, {hits} occurrences")
    print("\nmost frequent:")
    for n, c in sorted(per_name.items(), key=lambda kv: -kv[1])[:15]:
        print(f"  {c:>5}  {n}  ->  {RENAMES[n]}")
    missing = sorted(set(RENAMES) - set(per_name))
    if missing:
        print(f"\n{len(missing)} entries matched nothing (already renamed, or a typo):")
        print("  " + ", ".join(missing))
    if not apply:
        print("\n(dry run -- pass --apply, then cargo check --workspace)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
