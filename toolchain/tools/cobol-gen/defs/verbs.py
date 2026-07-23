"""Verbos COBOL con codegen: palabra → variante `CobolStatement` que la emite.

Crecer esto (y su gramática/codegen en Rust) es como BMO COBOL gana verbos.
`None`/ausente = reconocido como palabra pero aún sin compilar.
"""

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
