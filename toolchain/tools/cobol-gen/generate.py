"""Generador Python -> Rust para COBOL.

Lee `definition.py` (definicion compacta, por estandar) y ESCRIBE Rust
verboso y rapido en `lang/cobol/src/generated/`. Python es una herramienta de
tu PC; el Rust generado se commitea. **Python nunca entra a BMO** -- no es
dependencia de runtime, no ships, no corre en el kernel.

Uso:
    py toolchain/tools/cobol-gen/generate.py
"""

import pathlib
import definition

HERE = pathlib.Path(__file__).resolve().parent
OUT_DIR = (HERE / ".." / ".." / "lang" / "cobol" / "src" / "generated").resolve()
OUT_DIR.mkdir(parents=True, exist_ok=True)

HEADER = (
    "// AUTO-GENERADO por toolchain/tools/cobol-gen/generate.py -- NO editar a mano.\n"
    "// Fuente: toolchain/tools/cobol-gen/definition.py\n"
    "// Regenerar: py toolchain/tools/cobol-gen/generate.py\n"
    "// Python es la fabrica; este Rust se commitea. Python jamas entra a BMO.\n\n"
)


def all_reserved_with_standard():
    """Devuelve {palabra: etiqueta}. La ESENCIA (era/STANDARD) se distingue de
    las extensiones de VENDOR (VENDOR:categoria).

    Orden de prioridad de etiqueta:
      1. era estandar (COBOL74/85/2002/2023) — esencia curada, gana.
      2. VENDOR:<categoria> — extensiones no estandar.
      3. STANDARD — resto del corpus estandar (Gordon) no clasificado.
    """
    seen = {}
    # 1. Esencia por era (gana sobre todo).
    for std in ["COBOL74", "COBOL85", "COBOL2002", "COBOL2023"]:
        for w in definition.RESERVED_BY_STANDARD.get(std, []):
            wu = w.upper()
            seen.setdefault(wu, std)
    # 2. Extensiones de vendor (marcadas aparte).
    for cat, words in getattr(definition, "RESERVED_VENDOR", {}).items():
        for w in words:
            wu = w.upper()
            seen.setdefault(wu, f"VENDOR:{cat}")
    # 3. Resto del corpus estandar.
    for w in getattr(definition, "RESERVED_STANDARD", []):
        wu = w.upper()
        seen.setdefault(wu, "STANDARD")
    return seen


def is_essence(tag: str) -> bool:
    """¿La etiqueta es esencia estándar (no vendor)?"""
    return not tag.startswith("VENDOR:")


def gen_words() -> str:
    reserved = all_reserved_with_standard()
    words = sorted(reserved)
    out = [HEADER]
    out.append("/// Palabras reservadas de COBOL (Grace Hopper 1959 -> ISO 2023),")
    out.append("/// ordenadas para busqueda binaria.")
    out.append(f"pub static RESERVED: [&str; {len(words)}] = [")
    for w in words:
        out.append(f'    "{w}",')
    out.append("];\n")

    out.append("/// Estandar canonico donde cada palabra se hizo reservada.")
    out.append(f"pub static RESERVED_STD: [(&str, &str); {len(words)}] = [")
    for w in words:
        out.append(f'    ("{w}", "{reserved[w]}"),')
    out.append("];\n")

    out.append("/// Es `w` (en MAYUSCULAS) una palabra reservada de COBOL?")
    out.append("pub fn is_reserved(w: &str) -> bool {")
    out.append("    RESERVED.binary_search(&w).is_ok()")
    out.append("}\n")

    out.append("/// Etiqueta de `w`: era estandar (COBOL74..2023), \"STANDARD\", o")
    out.append("/// \"VENDOR:<cat>\". None si no es reservada.")
    out.append("pub fn reserved_since(w: &str) -> Option<&'static str> {")
    out.append("    RESERVED_STD.binary_search_by(|(k, _)| k.cmp(&w))")
    out.append("        .ok().map(|i| RESERVED_STD[i].1)")
    out.append("}\n")

    out.append("/// Es `w` ESENCIA COBOL estandar (no extension de vendor)?")
    out.append("pub fn is_essence(w: &str) -> bool {")
    out.append("    reserved_since(w).map_or(false, |s| !s.starts_with(\"VENDOR:\"))")
    out.append("}\n")

    out.append("/// Es `w` una extension de VENDOR (IBM/VAX/pantalla, NO estandar)?")
    out.append("pub fn is_vendor(w: &str) -> bool {")
    out.append("    reserved_since(w).map_or(false, |s| s.starts_with(\"VENDOR:\"))")
    out.append("}\n")

    out.append("/// Verbo COBOL -> nombre de la variante CobolStatement que lo emite.")
    out.append("/// `None` = reconocido pero aun sin codegen.")
    out.append("pub fn verb_kind(w: &str) -> Option<&'static str> {")
    out.append("    match w {")
    for word, kind in sorted(definition.VERBS.items()):
        out.append(f'        "{word}" => Some("{kind}"),')
    out.append("        _ => None,")
    out.append("    }")
    out.append("}\n")

    funcs = sorted({f.upper() for f in definition.INTRINSIC_FUNCTIONS})
    out.append("/// Funciones intrinsecas de COBOL (ISO-2002+), ordenadas.")
    out.append(f"pub static INTRINSIC: [&str; {len(funcs)}] = [")
    for f in funcs:
        out.append(f'    "{f}",')
    out.append("];\n")
    out.append("/// Es `f` (MAYUSCULAS) una funcion intrinseca de COBOL?")
    out.append("pub fn is_intrinsic(f: &str) -> bool {")
    out.append("    INTRINSIC.binary_search(&f).is_ok()")
    out.append("}")
    return "\n".join(out) + "\n"


def main() -> None:
    (OUT_DIR / "words.rs").write_text(gen_words(), encoding="utf-8")
    reserved = all_reserved_with_standard()
    print(f"generado {OUT_DIR / 'words.rs'}")
    print(f"  {len(reserved)} palabras reservadas (4 estandares)")
    print(f"  {len(definition.VERBS)} verbos, "
          f"{len(set(definition.INTRINSIC_FUNCTIONS))} intrinsecas")


if __name__ == "__main__":
    main()
