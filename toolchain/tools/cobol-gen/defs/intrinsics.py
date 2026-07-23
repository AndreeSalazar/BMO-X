"""Funciones intrínsecas de COBOL (ISO-2002+). Solo la LISTA (reconocimiento);
la implementación de cada una es lógica Rust + runtime (bmo-rt), no generable.
Ampliar hacia el catálogo completo del estándar 2023.
"""

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
