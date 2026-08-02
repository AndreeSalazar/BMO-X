"""**El censo de C, entero** — para poder decidir qué NO entra.

Un inventario de todo lo que el estándar llama "C", por categorías, con un
veredicto por fila. No está para implementarlo: está para poder **mirar el
tamaño real del idioma y elegir**.

Veredictos:

    ESENCIA     sin esto no es C. Entra, tarde o temprano.
    UTIL        aporta de verdad a lo que BMO hace. Entra cuando toque.
    DESCARTAR   existe en C y NO entra, con el motivo escrito.

★ La columna que decide es la tercera. Una lista de características sin
veredicto es una lista de deberes; con veredicto es un **alcance**. Y un
alcance es lo que hace que un compilador se pueda terminar — la diferencia
entre "le falta la mitad de C" y "esta mitad de C está fuera a propósito".

Nota sobre `DESCARTAR`: no significa "nunca". Significa "no en este alcance, y
éste es el motivo". El día que el motivo caduque, la fila cambia.
"""

# (categoria, elemento, era, veredicto, motivo)
CENSO = [
    # ── Tipos base ──────────────────────────────────────────────────
    ("tipos", "void", "C89", "ESENCIA", "el tipo de lo que no devuelve nada"),
    ("tipos", "char / signed / unsigned char", "C89", "ESENCIA", "el byte"),
    ("tipos", "short / unsigned short", "C89", "ESENCIA", "16 bits"),
    ("tipos", "int / unsigned int", "C89", "ESENCIA", "el entero por defecto"),
    ("tipos", "long / unsigned long", "C89", "ESENCIA", "64 bits en este ABI"),
    ("tipos", "long long", "C99", "UTIL", "ya está; en x86-64 coincide con long"),
    ("tipos", "float", "C89", "UTIL", "está; la banca NO lo usa (decimal exacto)"),
    ("tipos", "double", "C89", "UTIL", "está; ídem"),
    ("tipos", "long double (80 bits)", "C89", "DESCARTAR",
     "el x87 de 80 bits es una rareza de Intel; el decimal exacto ya lo dan COBOL y Ada"),
    ("tipos", "_Bool", "C99", "UTIL", "un int de 0/1; barato"),
    ("tipos", "_Complex / _Imaginary", "C99", "DESCARTAR",
     "números complejos en un SO de banca: nadie los ha pedido nunca"),

    # ── Calificadores ───────────────────────────────────────────────
    ("calificadores", "const", "C89", "ESENCIA", "está"),
    ("calificadores", "volatile", "C89", "ESENCIA", "está; obligatorio para MMIO"),
    ("calificadores", "restrict", "C99", "DESCARTAR",
     "es una promesa al OPTIMIZADOR. No cambia lo que el programa hace"),
    ("calificadores", "_Atomic", "C11", "DESCARTAR",
     "no hay hilos de usuario. Cuando haya SMP se vuelve a mirar"),

    # ── Almacenamiento ──────────────────────────────────────────────
    ("almacenamiento", "auto", "C89", "UTIL", "aceptado y tirado: redundante desde 1978"),
    ("almacenamiento", "register", "C89", "UTIL", "aceptado y tirado: todos lo ignoran"),
    ("almacenamiento", "static", "C89", "ESENCIA", "HECHO 2026-08-02"),
    ("almacenamiento", "extern", "C89", "ESENCIA", "está"),
    ("almacenamiento", "_Thread_local", "C11", "DESCARTAR", "no hay hilos de usuario"),

    # ── Tipos derivados ─────────────────────────────────────────────
    ("derivados", "punteros (multinivel)", "C89", "ESENCIA", "está"),
    ("derivados", "arrays", "C89", "ESENCIA", "está, también dentro de agregados"),
    ("derivados", "punteros a función", "C89", "ESENCIA", "está; DOOM vive de ellos"),
    ("derivados", "struct / union / enum", "C89", "ESENCIA", "están"),
    ("derivados", "campos de bits", "C89", "UTIL",
     "se aceptan SIN empaquetar; empaquetar es máscara y RMW en cada acceso"),
    ("derivados", "miembro de array flexible", "C99", "UTIL", "el `t x[]` final de un struct"),
    ("derivados", "VLA (array de longitud variable)", "C99", "DESCARTAR",
     "pide reservar en la pila en ejecución; C11 ya lo hizo opcional y casi nadie lo usa"),

    # ── Funciones ───────────────────────────────────────────────────
    ("funciones", "prototipos", "C89", "ESENCIA", "HECHO 2026-08-02: sin esto no hay recursión mutua"),
    ("funciones", "varargs (...)", "C89", "ESENCIA", "HECHO 2026-08-02, con `__va_arg(i)`"),
    ("funciones", "inline", "C99", "DESCARTAR", "sugerencia al optimizador"),
    ("funciones", "_Noreturn", "C11", "DESCARTAR", "ídem"),
    ("funciones", "K&R (parámetros sin tipo)", "C89", "DESCARTAR",
     "sintaxis obsoleta desde 1989; ni DOOM la usa"),

    # ── Operadores ──────────────────────────────────────────────────
    ("operadores", "aritméticos + - * / %", "C89", "ESENCIA", "están"),
    ("operadores", "incremento/decremento ++ --", "C89", "ESENCIA", "están (pre y post)"),
    ("operadores", "relacionales == != < > <= >=", "C89", "ESENCIA", "están"),
    ("operadores", "lógicos && || !", "C89", "ESENCIA", "están, con cortocircuito"),
    ("operadores", "de bits & | ^ ~ << >>", "C89", "ESENCIA", "están"),
    ("operadores", "asignación compuesta (11)", "C89", "ESENCIA", "están"),
    ("operadores", "acceso . -> [] ()", "C89", "ESENCIA", "están"),
    ("operadores", "&direccion / *indireccion", "C89", "ESENCIA", "están"),
    ("operadores", "ternario ?:", "C89", "ESENCIA", "está"),
    ("operadores", "coma", "C89", "ESENCIA", "está"),
    ("operadores", "sizeof", "C89", "ESENCIA", "está"),
    ("operadores", "cast", "C89", "ESENCIA", "está, y trunca de verdad"),
    ("operadores", "_Alignof / _Alignas", "C11", "DESCARTAR", "el alineado lo decide el layout"),
    ("operadores", "_Generic", "C11", "DESCARTAR",
     "selección por tipo en macros. Es lo que C++ resuelve con sobrecarga"),
    ("operadores", "literales compuestos", "C99", "UTIL", "`(struct P){1,2}` — azúcar útil"),

    # ── Sentencias ──────────────────────────────────────────────────
    ("sentencias", "expresión y bloque", "C89", "ESENCIA", "están"),
    ("sentencias", "if / else", "C89", "ESENCIA", "están"),
    ("sentencias", "switch / case / default", "C89", "ESENCIA", "están, con fallthrough"),
    ("sentencias", "while / do-while / for", "C89", "ESENCIA", "están"),
    ("sentencias", "break / continue", "C89", "ESENCIA", "están"),
    ("sentencias", "return", "C89", "ESENCIA", "está"),
    ("sentencias", "goto y etiquetas", "C89", "ESENCIA", "están"),
    ("sentencias", "sentencia vacía", "C89", "ESENCIA", "está"),
    ("sentencias", "declaración mezclada con código", "C99", "UTIL", "está"),

    # ── Preprocesador ───────────────────────────────────────────────
    ("preprocesador", "#define objeto", "C89", "ESENCIA", "está"),
    ("preprocesador", "#define función", "C89", "ESENCIA", "está"),
    ("preprocesador", "#define variádica", "C99", "UTIL", "está"),
    ("preprocesador", "#include", "C89", "ESENCIA", "está"),
    ("preprocesador", "#if / #ifdef / #ifndef / #elif / #else / #endif", "C89", "ESENCIA", "están"),
    ("preprocesador", "#undef", "C89", "ESENCIA", "está"),
    ("preprocesador", "#error", "C89", "ESENCIA", "está"),
    ("preprocesador", "#pragma", "C89", "UTIL", "se ignora; `#pragma once` sí conviene"),
    ("preprocesador", "# (stringize) y ## (pegado)", "C89", "UTIL",
     "los usa cualquier cabecera con macros serias"),
    ("preprocesador", "#line", "C89", "DESCARTAR", "sólo cambia los números de error"),
    ("preprocesador", "__FILE__ / __LINE__", "C89", "UTIL", "un `assert` de verdad los pide"),

    # ── Biblioteca: las 29 cabeceras de C11 ─────────────────────────
    ("biblioteca", "<stdio.h>", "C89", "ESENCIA", "printf/getchar/scanf están; faltan puts/sprintf/ficheros"),
    ("biblioteca", "<string.h>", "C89", "ESENCIA", "memcpy/memset/strlen/strcmp/strcpy HECHOS"),
    ("biblioteca", "<stdlib.h>", "C89", "ESENCIA", "abs HECHO; malloc/free piden la capability de memoria"),
    ("biblioteca", "<stddef.h>", "C89", "ESENCIA", "size_t, NULL, offsetof — tipos, no código"),
    ("biblioteca", "<stdint.h>", "C99", "ESENCIA", "int32_t y compañía: puro typedef"),
    ("biblioteca", "<limits.h> / <float.h>", "C89", "ESENCIA", "constantes"),
    ("biblioteca", "<stdbool.h>", "C99", "UTIL", "tres macros"),
    ("biblioteca", "<stdarg.h>", "C89", "UTIL", "va_list sobre `__va_arg`"),
    ("biblioteca", "<ctype.h>", "C89", "UTIL", "isdigit y compañía: una tabla de 256"),
    ("biblioteca", "<assert.h>", "C89", "UTIL", "con __FILE__/__LINE__"),
    ("biblioteca", "<time.h>", "C89", "UTIL", "hay TSC; falta calendario"),
    ("biblioteca", "<math.h>", "C89", "DESCARTAR",
     "DOOM no usa coma flotante en el render; el decimal exacto ya está en COBOL y Ada"),
    ("biblioteca", "<errno.h>", "C89", "DESCARTAR",
     "un global de error es justo lo contrario de devolver el fallo"),
    ("biblioteca", "<signal.h>", "C89", "DESCARTAR",
     "no hay señales: aquí un fallo mata la tarea y lo DICE"),
    ("biblioteca", "<setjmp.h>", "C89", "DESCARTAR", "pide guardar el marco entero; nadie lo pide"),
    ("biblioteca", "<locale.h>", "C89", "DESCARTAR", "una libc de verdad empieza aquí y no acaba"),
    ("biblioteca", "<wchar.h> / <wctype.h> / <uchar.h>", "C89", "DESCARTAR",
     "la consola de BMO es de un byte por carácter a propósito"),
    ("biblioteca", "<threads.h>", "C11", "DESCARTAR", "no hay hilos de usuario"),
    ("biblioteca", "<stdatomic.h>", "C11", "DESCARTAR", "ídem"),
    ("biblioteca", "<complex.h> / <tgmath.h>", "C99", "DESCARTAR", "números complejos"),
    ("biblioteca", "<fenv.h>", "C99", "DESCARTAR", "modos de redondeo del x87"),
    ("biblioteca", "<inttypes.h>", "C99", "DESCARTAR", "sólo macros de formato para printf"),
    ("biblioteca", "<iso646.h>", "C89", "DESCARTAR", "alias de `&&` para teclados sin `&`. 1995"),
    ("biblioteca", "<stdalign.h> / <stdnoreturn.h>", "C11", "DESCARTAR", "envoltorio de lo ya descartado"),
]


def por_veredicto(v):
    return [f for f in CENSO if f[3] == v]


def categorias():
    vistas = []
    for c, *_ in CENSO:
        if c not in vistas:
            vistas.append(c)
    return vistas
