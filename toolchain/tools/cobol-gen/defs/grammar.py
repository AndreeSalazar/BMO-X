"""(FUTURO) Formatos de sentencia de COBOL — la gramática por verbo, como
DATOS, para que `generate.py` produzca el dispatch del parser.

La idea: los manuales ISO/IBM describen cada verbo con un formato formal
regular; encodearlo como datos (igual que sem-asm encodea instrucciones)
deja que Python genere el andamiaje del parser. La SEMÁNTICA (qué hace cada
verbo, qué bytes emite) sigue siendo lógica Rust.

Ejemplo de la forma que tendría (aún NO consumido por generate.py):

    STATEMENT_FORMATS = {
        "MOVE": ["<operand>", "TO", "<ident>+"],
        "ADD":  ["<operand>+", "TO", "<ident>", "[GIVING <ident>]",
                 "[ROUNDED]", "[ON SIZE ERROR <stmt>]", "[END-ADD]"],
        "IF":   ["<condition>", "[THEN]", "<stmt>*",
                 "[ELSE <stmt>*]", "[END-IF]"],
        # … EVALUATE, PERFORM VARYING, STRING, INSPECT, SEARCH …
    }

Cuando se llene, `generate.py` emitirá tablas/plantillas de parseo; el
`tparser.rs` las consumirá para reconocer cada forma, y el `codegen.rs`
implementará (a mano, en Rust) qué produce cada una.
"""

STATEMENT_FORMATS: dict[str, list[str]] = {}
