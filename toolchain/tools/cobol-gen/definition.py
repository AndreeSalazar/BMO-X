"""Definición COMPACTA de COBOL — la fuente de la que Python genera el Rust
gigante. Crecer estas listas hace crecer el parser Rust SOLO, sin escribir
Rust a mano. Python es la fábrica; nunca entra a BMO (solo se commitea el
Rust generado).

Regla: aquí describes COBOL de forma corta y legible; `generate.py` la
expande a tablas Rust verbosas y rápidas (búsqueda binaria).
"""

# Palabras reservadas de COBOL (subconjunto inicial de ANSI-85 + comunes;
# ampliar hacia el estándar 2023 completo — son cientos). Mayúsculas.
RESERVED_WORDS = [
    # Divisiones y secciones
    "IDENTIFICATION", "ENVIRONMENT", "DATA", "PROCEDURE", "DIVISION",
    "SECTION", "PROGRAM-ID", "AUTHOR", "WORKING-STORAGE", "FILE",
    "LINKAGE", "CONFIGURATION", "INPUT-OUTPUT", "FILE-CONTROL",
    # Verbos
    "ACCEPT", "ADD", "ALTER", "CALL", "CANCEL", "CLOSE", "COMPUTE",
    "CONTINUE", "DELETE", "DISPLAY", "DIVIDE", "EVALUATE", "EXIT",
    "GO", "GOBACK", "IF", "INITIALIZE", "INSPECT", "MERGE", "MOVE",
    "MULTIPLY", "OPEN", "PERFORM", "READ", "RELEASE", "RETURN",
    "REWRITE", "SEARCH", "SET", "SORT", "START", "STOP", "STRING",
    "SUBTRACT", "UNSTRING", "WRITE",
    # Cláusulas / palabras de estructura y datos
    "PIC", "PICTURE", "VALUE", "OCCURS", "REDEFINES", "USAGE", "COMP",
    "COMP-3", "BINARY", "PACKED-DECIMAL", "DISPLAY", "SIGN", "LEADING",
    "TRAILING", "SEPARATE", "FILLER", "BLANK", "JUSTIFIED", "SYNCHRONIZED",
    # Condicionales / control
    "ELSE", "END-IF", "THEN", "WHEN", "UNTIL", "VARYING", "THROUGH",
    "THRU", "TIMES", "GIVING", "REMAINDER", "ROUNDED", "ON", "SIZE",
    "ERROR", "OVERFLOW", "NOT", "AND", "OR", "TO", "FROM", "BY", "INTO",
    "EQUAL", "GREATER", "LESS", "THAN", "ZERO", "ZEROS", "SPACE",
    "SPACES", "HIGH-VALUE", "LOW-VALUE", "QUOTE", "ALL", "OF", "IN",
    "IS", "ARE", "USING", "RETURNING", "END", "RUN",
]

# Verbos que ya sabe emitir el codegen → el CobolStatement correspondiente.
# (palabra COBOL -> variante del enum). Ampliar a medida que el codegen crezca.
VERBS = {
    "DISPLAY": "Display",
    "ACCEPT": "Accept",
    "MOVE": "Move",
    "ADD": "Add",
    "SUBTRACT": "Subtract",
    "MULTIPLY": "Multiply",
    "DIVIDE": "Divide",
    "COMPUTE": "Compute",
    "IF": "If",
    "PERFORM": "Perform",
    "OPEN": "Open",
    "CLOSE": "Close",
    "READ": "Read",
    "WRITE": "Write",
    "STOP": "StopRun",
}
