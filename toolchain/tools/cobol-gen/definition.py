"""Definición COMPACTA de COBOL — de Grace Hopper (1959) al estándar 2023.

Organizada POR ERA/ESTÁNDAR: cada palabra sabe cuándo entró. Python expande
esto a tablas Rust verbosas. Crecer estas listas = crecer el parser Rust
SOLO. Python es la fábrica; NUNCA entra a BMO (solo se commitea el Rust).

Fuentes (conocimiento público de los estándares; ampliar hacia el texto
completo ISO 1989:2023): COBOL-60/68/74 (Hopper/CODASYL), ANSI-85,
ISO 2002 (OO), ISO 2014, ISO 2023.
"""

# ── Palabras reservadas por estándar que las introdujo/consolidó ──────────
# El valor es el "primer estándar" canónico donde la palabra es reservada.
RESERVED_BY_STANDARD = {
    # COBOL-60/74: el núcleo de Grace Hopper y CODASYL.
    "COBOL74": [
        "IDENTIFICATION", "ENVIRONMENT", "DATA", "PROCEDURE", "DIVISION",
        "SECTION", "PROGRAM-ID", "AUTHOR", "WORKING-STORAGE", "FILE",
        "LINKAGE", "CONFIGURATION", "INPUT-OUTPUT", "FILE-CONTROL",
        "ACCEPT", "ADD", "ALTER", "CALL", "CLOSE", "DISPLAY", "DIVIDE",
        "EXIT", "GO", "IF", "MOVE", "MULTIPLY", "OPEN", "PERFORM", "READ",
        "WRITE", "REWRITE", "STOP", "SUBTRACT", "SORT", "MERGE", "SET",
        "PIC", "PICTURE", "VALUE", "OCCURS", "REDEFINES", "USAGE", "COMP",
        "COMPUTATIONAL", "DISPLAY", "BINARY", "SIGN", "LEADING", "TRAILING",
        "SEPARATE", "FILLER", "BLANK", "JUSTIFIED", "SYNCHRONIZED",
        "ELSE", "THEN", "UNTIL", "VARYING", "THROUGH", "THRU", "TIMES",
        "GIVING", "REMAINDER", "ROUNDED", "ON", "SIZE", "ERROR",
        "OVERFLOW", "NOT", "AND", "OR", "TO", "FROM", "BY", "INTO",
        "EQUAL", "GREATER", "LESS", "THAN", "ZERO", "ZEROS", "ZEROES",
        "SPACE", "SPACES", "HIGH-VALUE", "HIGH-VALUES", "LOW-VALUE",
        "LOW-VALUES", "QUOTE", "QUOTES", "ALL", "OF", "IN", "IS", "ARE",
        "USING", "RUN", "STANDARD", "LABEL", "RECORD", "RECORDS", "BLOCK",
        "SELECT", "ASSIGN", "ORGANIZATION", "ACCESS", "SEQUENTIAL",
        "INDEXED", "RELATIVE", "KEY", "STATUS", "AT", "END", "INVALID",
        "DEPENDING", "ASCENDING", "DESCENDING", "COUNT", "TALLYING",
        "REPLACING", "INSPECT", "STRING", "UNSTRING", "DELIMITED",
        "DELIMITER", "POINTER", "ADVANCING", "PAGE", "LINE", "LINES",
    ],
    # ANSI-85: estructura moderna (scope terminators, EVALUATE, INITIALIZE).
    "COBOL85": [
        "EVALUATE", "WHEN", "CONTINUE", "INITIALIZE", "CANCEL", "GOBACK",
        "END-IF", "END-PERFORM", "END-EVALUATE", "END-READ", "END-WRITE",
        "END-ADD", "END-SUBTRACT", "END-MULTIPLY", "END-DIVIDE",
        "END-COMPUTE", "END-CALL", "END-STRING", "END-UNSTRING",
        "END-SEARCH", "END-START", "END-DELETE", "END-REWRITE",
        "END-RETURN", "END-ACCEPT", "END-DISPLAY", "COMPUTE", "SEARCH",
        "RELEASE", "RETURN", "START", "DELETE", "REFERENCE", "CONTENT",
        "GLOBAL", "EXTERNAL", "COMMON", "TRUE", "FALSE", "ANY", "OTHER",
        "PACKED-DECIMAL",
    ],
    # ISO-2002: orientación a objetos + intrínsecas.
    "COBOL2002": [
        "CLASS", "CLASS-ID", "OBJECT", "METHOD", "METHOD-ID", "FACTORY",
        "INHERITS", "INVOKE", "SELF", "SUPER", "OVERRIDE", "FUNCTION",
        "RAISING", "EXCEPTION", "RESUME", "LOCAL-STORAGE", "TYPEDEF",
        "PROPERTY", "GET", "REPOSITORY", "INTERFACE", "INTERFACE-ID",
        "BASED", "BIT", "BOOLEAN", "VAL-STATUS", "VALIDATE", "FORMAT",
    ],
    # ISO-2014 / 2023: refinamientos modernos.
    "COBOL2023": [
        "ALLOCATE", "FREE", "JSON", "XML", "GENERATE", "PARSE",
        "ACTIVE-CLASS", "ALIGNED", "AWAY-FROM-ZERO", "NEAREST-EVEN",
        "PROHIBITED", "TRUNCATION", "STANDARD-BINARY", "STANDARD-DECIMAL",
        "FLOAT-SHORT", "FLOAT-LONG", "FLOAT-EXTENDED", "USER-DEFAULT",
    ],
}

# ── Verbos con codegen (palabra COBOL -> variante CobolStatement) ─────────
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

# ── Funciones intrínsecas (ISO-2002+) — se generan a su propia tabla ──────
INTRINSIC_FUNCTIONS = [
    "ABS", "ACOS", "ANNUITY", "ASIN", "ATAN", "BYTE-LENGTH", "CHAR",
    "COS", "CURRENT-DATE", "DATE-OF-INTEGER", "DAY-OF-INTEGER", "E",
    "EXP", "FACTORIAL", "INTEGER", "INTEGER-OF-DATE", "INTEGER-PART",
    "LENGTH", "LOG", "LOG10", "LOWER-CASE", "MAX", "MEAN", "MEDIAN",
    "MIDRANGE", "MIN", "MOD", "NUMVAL", "NUMVAL-C", "ORD", "ORD-MAX",
    "ORD-MIN", "PI", "PRESENT-VALUE", "RANDOM", "RANGE", "REM",
    "REVERSE", "SIN", "SQRT", "STANDARD-DEVIATION", "SUM", "TAN",
    "TRIM", "UPPER-CASE", "VARIANCE", "WHEN-COMPILED",
    # 2023
    "FORMATTED-CURRENT-DATE", "FORMATTED-DATE", "FORMATTED-TIME",
    "CONCATENATE", "SUBSTITUTE", "BIT-OF", "BIT-TO-CHAR", "HEX-OF",
]
