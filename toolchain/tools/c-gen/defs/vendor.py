"""Lo que traen GCC, LLVM y MSVC **encima** del estandar.

Esta lista no existe para copiarla: existe para **reconocerla y rechazarla**.
Es el mismo reparto que ya hace el COBOL de BMO —esencia por era contra
`VENDOR:categoria`— y por la misma razon: un compilador que persigue las
extensiones de otros tres nunca termina.

★ Y hay una excepcion honesta: DOOM se escribio para GCC en 1993. Si su codigo
usa una extension, el rechazo no puede ser "no", tiene que ser "no, y esto es
lo que se hace en su lugar". Por eso cada fila lleva SALIDA.
"""

# (extension, de quien, veredicto, salida)
EXTENSIONES = [
    ("__attribute__((packed))", "GCC/Clang", "RECHAZAR",
     "DOOM lo usa en las estructuras del WAD. Salida: leer los campos byte a byte"
     " al cargar, que ademas arregla el endianness de paso"),
    ("__attribute__((noreturn))", "GCC/Clang", "RECHAZAR",
     "es una pista para el optimizador, no cambia lo que el programa hace: se ignora"),
    ("__declspec(dllimport)", "MSVC", "RECHAZAR",
     "no hay DLLs en BMO: un .bex es una imagen entera"),
    ("asm inline", "los tres", "RECHAZAR",
     "BMO ya tiene sem-asm, que es asm con nombres y tabla. No hacen falta dos"),
    ("typeof", "GCC/Clang", "MIRAR",
     "en C23 es estandar. Si entra, que entre como C23 y no como extension"),
    ("expresiones de sentencia ({...})", "GCC/Clang", "RECHAZAR",
     "DOOM no las usa; complican el parser para nada"),
    ("arrays de longitud cero", "GCC", "RECHAZAR",
     "C99 tiene miembros de array flexible: eso si es estandar"),
    ("#pragma once", "los tres (de facto)", "ACEPTAR",
     "no es del estandar y lo implementa todo el mundo. Cuesta cuatro lineas"
     " y evita el guardas de cabecera en 50 ficheros"),
    ("__builtin_expect", "GCC/Clang", "RECHAZAR",
     "optimizacion. Se ignora sin cambiar el resultado"),
    ("long double de 80 bits", "GCC", "RECHAZAR",
     "DOOM no usa coma flotante; el decimal exacto ya lo da COBOL/Ada"),
]


# ── Lo que se le PREGUNTA a un compilador de verdad, si esta instalado ──
#
# Extraer esto de GCC/Clang/MSVC no es para copiarlo: es para **contrastar**.
# Si BMO C dice que `int` mide 4 y los tres dicen 4, no hay discusion. Si dijera
# otra cosa, el que se ha equivocado es BMO.
PREGUNTAS_DE_TAMANO = [
    ("char", 1), ("short", 2), ("int", 4), ("long", 8),
    ("long long", 8), ("void*", 8), ("float", 4), ("double", 8),
]

# Macros que los tres definen y que un codigo ajeno consulta para saber donde
# esta. DOOM mira `__GNUC__` y `NORMALUNIX`.
MACROS_QUE_MIRA_EL_CODIGO_AJENO = [
    "__STDC__", "__STDC_VERSION__", "__x86_64__", "__LP64__",
    "__GNUC__", "__clang__", "_MSC_VER", "__SIZEOF_INT__", "__CHAR_BIT__",
]
