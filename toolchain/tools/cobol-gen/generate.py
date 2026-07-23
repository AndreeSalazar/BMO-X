"""Generador Python → Rust para COBOL.

Lee `definition.py` (definición compacta) y ESCRIBE Rust verboso y rápido en
`lang/cobol/src/generated/`. Python es una herramienta de tu PC; el Rust
generado se commitea. **Python nunca entra a BMO** — soberanía intacta.

Uso (una terminal nueva ya tiene `py`; si no, usa la ruta completa a python):
    py toolchain/tools/cobol-gen/generate.py
"""

import pathlib
import definition

HERE = pathlib.Path(__file__).resolve().parent
OUT_DIR = HERE / ".." / ".." / "lang" / "cobol" / "src" / "generated"
OUT_DIR = OUT_DIR.resolve()
OUT_DIR.mkdir(parents=True, exist_ok=True)

HEADER = (
    "// AUTO-GENERADO por toolchain/tools/cobol-gen/generate.py — NO editar a mano.\n"
    "// Fuente: toolchain/tools/cobol-gen/definition.py\n"
    "// Regenerar: py toolchain/tools/cobol-gen/generate.py\n"
    "// Python es la fabrica; este Rust se commitea. Python jamas entra a BMO.\n\n"
)


def gen_reserved() -> str:
    words = sorted({w.upper() for w in definition.RESERVED_WORDS})
    out = [HEADER]
    out.append("/// Palabras reservadas de COBOL, ordenadas (busqueda binaria).")
    out.append(f"pub static RESERVED: [&str; {len(words)}] = [")
    for w in words:
        out.append(f'    "{w}",')
    out.append("];\n")
    out.append("/// Es `w` (en MAYUSCULAS) una palabra reservada de COBOL?")
    out.append("pub fn is_reserved(w: &str) -> bool {")
    out.append("    RESERVED.binary_search(&w).is_ok()")
    out.append("}\n")
    out.append("/// Verbo COBOL -> nombre de la variante CobolStatement que lo emite.")
    out.append("/// `None` = reconocido pero aun sin codegen.")
    out.append("pub fn verb_kind(w: &str) -> Option<&'static str> {")
    out.append("    match w {")
    for word, kind in sorted(definition.VERBS.items()):
        out.append(f'        "{word}" => Some("{kind}"),')
    out.append("        _ => None,")
    out.append("    }")
    out.append("}")
    return "\n".join(out) + "\n"


def main() -> None:
    text = gen_reserved()
    dst = OUT_DIR / "words.rs"
    dst.write_text(text, encoding="utf-8")
    n_words = len({w.upper() for w in definition.RESERVED_WORDS})
    n_verbs = len(definition.VERBS)
    print(f"generado {dst}")
    print(f"  {n_words} palabras reservadas, {n_verbs} verbos")


if __name__ == "__main__":
    main()
