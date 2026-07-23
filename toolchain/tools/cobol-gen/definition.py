"""Agregador de la definición COBOL — importa de `defs/` y re-exporta.

Estructura organizada por concern (crecer cada archivo = crecer BMO COBOL):

    defs/
      words.py       palabras reservadas (esencia por era + STANDARD + VENDOR)
      verbs.py       verbos con codegen (palabra → CobolStatement)
      intrinsics.py  funciones intrínsecas (lista de reconocimiento)
      grammar.py     (futuro) formatos de sentencia por verbo → dispatch
      editmasks.py   (futuro) máscaras de edición PIC ($$,$$9.99, Z, CR/DB…)

`generate.py` lee de aquí. Python genera lo TABULAR (tablas, dispatch,
esqueletos); la SEMÁNTICA/codegen de cada verbo es lógica Rust (no generable).
"""

from defs.words import (  # noqa: F401
    RESERVED_BY_STANDARD,
    RESERVED_STANDARD,
    RESERVED_VENDOR,
)
from defs.verbs import VERBS  # noqa: F401
from defs.intrinsics import INTRINSIC_FUNCTIONS  # noqa: F401
