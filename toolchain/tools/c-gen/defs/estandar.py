"""La ESENCIA de C: lo que dice ISO, por era.

Curado a mano y a proposito. No se extrae de ningun compilador porque los tres
—GCC, LLVM y MSVC— traen su propio equipaje encima del estandar, y mezclarlos
seria empezar por donde no se debe: el estandar es la referencia, los
compiladores son testigos.

Cada entrada lleva SONDA: un programa de C minimo que usa esa caracteristica y
nada mas. Se compila con BMO C y su veredicto es el dato. Leer el lexer diria
que palabras reconoce; una sonda dice que COMPILA, que es lo unico que importa.
"""

# ── Palabras clave por era ────────────────────────────────────────────

# C89/C90: las 32 originales.
C89 = [
    "auto", "break", "case", "char", "const", "continue", "default", "do",
    "double", "else", "enum", "extern", "float", "for", "goto", "if",
    "int", "long", "register", "return", "short", "signed", "sizeof",
    "static", "struct", "switch", "typedef", "union", "unsigned", "void",
    "volatile", "while",
]

# C99 anade cinco.
C99 = ["inline", "restrict", "_Bool", "_Complex", "_Imaginary"]

# C11 anade siete.
C11 = [
    "_Alignas", "_Alignof", "_Atomic", "_Generic", "_Noreturn",
    "_Static_assert", "_Thread_local",
]

# C23 anade estas (las que importan; el resto son alias de <stdbool.h> etc).
C23 = ["bool", "true", "false", "nullptr", "typeof", "constexpr", "static_assert"]


def todas():
    """{palabra: era}. La era mas temprana gana: `int` es C89, no C23."""
    fuera = {}
    for era, lista in (("C89", C89), ("C99", C99), ("C11", C11), ("C23", C23)):
        for w in lista:
            fuera.setdefault(w, era)
    return fuera


# ── Sondas: un programa por caracteristica ────────────────────────────
#
# El nombre es la clave, el valor es (era, para_que_sirve, fuente).
# `para_que_sirve` no es decoracion: es lo que decide si algo entra en un
# compilador ACOTADO. "Lo pide DOOM" es un motivo; "existe en el estandar" no
# lo es por si solo.

SONDAS = {
    # ── Almacenamiento y ambito ──
    "static (local)": ("C89", "DOOM lo usa en casi cada funcion",
                       "int main(){static int n=5;return n;}"),
    "static (global)": ("C89", "DOOM: una global por fichero, ocultada al enlazador",
                        "static int g=7;\nint main(){return g;}"),
    "extern": ("C89", "declarar sin definir; obligatorio si hay varios ficheros",
               "extern int g;\nint main(){return 0;}"),
    "register": ("C89", "hoy no aporta nada: todos los compiladores lo ignoran",
                 "int main(){register int n=1;return n;}"),
    "auto": ("C89", "redundante desde 1978; en C23 cambio de significado",
             "int main(){auto int n=1;return n;}"),

    # ── Tipos ──
    "struct": ("C89", "esencial", "struct P{int x;int y;};\nint main(){struct P p;p.x=1;return p.x;}"),
    # ★ MINIMA a proposito: sin array dentro. La primera version llevaba
    # `char c[4]` y fallaba — pero entonces no se sabia si lo que falta es la
    # union o los arrays dentro de ella. Una sonda que mezcla dos cosas no
    # contesta ninguna.
    "union": ("C89", "DOOM la usa en sus thinkers",
              "union U{int i;char c;};\nint main(){union U u;u.i=0;return u.c;}"),
    "array dentro de union": ("C89", "DOOM: la union de datos del WAD",
                              "union U{int i;char c[4];};\nint main(){union U u;u.i=0;return u.c[0];}"),
    "enum": ("C89", "esencial", "enum E{A,B};\nint main(){return B;}"),
    "typedef": ("C89", "DOOM define fixed_t, mobj_t... todo pasa por aqui",
                "typedef int fixed_t;\nint main(){fixed_t f=1;return f;}"),
    "puntero a funcion": ("C89", "DOOM: think_t, actionf_t — el corazon de sus actores",
                          "int suma(int a){return a;}\nint main(){int (*f)(int)=suma;return f(1);}"),
    "array de struct": ("C89", "DOOM: tablas de estados, sprites, sectores",
                        "struct P{int x;};\nint main(){struct P v[4];v[0].x=3;return v[0].x;}"),
    "bitfields": ("C89", "poco usado en DOOM; caro de emitir",
                  "struct F{unsigned a:3;unsigned b:5;};\nint main(){struct F f;f.a=1;return f.a;}"),
    "long long": ("C99", "no lo pide DOOM; util para contadores",
                  "int main(){long long n=1;return (int)n;}"),
    "unsigned char": ("C89", "DOOM: el framebuffer es byte[]",
                      "int main(){unsigned char b=200;return b;}"),

    # ── Funciones ──
    # ★ SIN `#include <stdarg.h>`. La primera version lo llevaba y fallaba con
    # "file not found: stdarg.h" — o sea que no probaba los varargs, probaba si
    # existe la cabecera. Aqui se pregunta solo por la SINTAXIS del `...`, que
    # es lo que el compilador tiene que aceptar antes de que la cabecera tenga
    # sentido siquiera.
    "varargs: declarar (...)": ("C89", "DOOM: I_Error(fmt, ...)",
                                "int suma(int n,...){return n;}\nint main(){return suma(1,2);}"),
    # ★ Declararlos y LEERLOS son dos sondas, y hacen falta las dos. Aceptar
    # `...` sin poder leer los argumentos compilaria media libc y no haria
    # ninguna: la sonda de arriba se pondria verde y el informe estaria
    # exagerando lo que hay.
    "varargs: leerlos": ("C89", "sin esto, `...` compila y no sirve para nada",
                         "int suma(int n,...){int t;int i;t=0;"
                         "for(i=0;i<n;i=i+1){t=t+__va_arg(i);}return t;}"
                         "\nint main(){return suma(2,3,4);}"),
    "array dentro de struct": ("C89", "DOOM: `char nombre[8]` en cada lump del WAD",
                               "struct S{int i;char c[4];};"
                               "\nint main(){struct S s;s.c[0]=7;return s.c[0];}"),
    # Y estas dos separadas: un prototipo con el parametro SIN nombre es legal
    # en C y es como los escribe DOOM, pero si falla hay que saber si lo que
    # falta es el prototipo o el nombre que falta.
    "prototipo (param con nombre)": ("C89", "obligatorio para llamar antes de definir",
                                     "int f(int a);\nint main(){return f(1);}\nint f(int a){return a;}"),
    "prototipo (param sin nombre)": ("C89", "asi los escribe DOOM en sus cabeceras",
                                     "int f(int);\nint main(){return f(1);}\nint f(int a){return a;}"),
    "recursion": ("C89", "esencial", "int f(int n){return n<=1?1:n*f(n-1);}\nint main(){return f(3);}"),

    # ── Control ──
    "goto": ("C89", "DOOM lo usa poco pero lo usa", "int main(){int i=0;otra:i++;if(i<3)goto otra;return i;}"),
    "switch con fallthrough": ("C89", "esencial",
                               "int main(){int n=1;switch(n){case 0:case 1:return 7;default:return 0;}}"),
    "operador ternario": ("C89", "esencial", "int main(){int n=1;return n?2:3;}"),
    "for con declaracion": ("C99", "comodidad; DOOM es C89 y no lo necesita",
                            "int main(){int s=0;for(int i=0;i<3;i++)s+=i;return s;}"),

    # ── Preprocesador ──
    "#define con argumentos": ("C89", "DOOM: FixedMul, MAXPLAYERS... por todas partes",
                               "#define DOBLE(x) ((x)*2)\nint main(){return DOBLE(2);}"),
    "#include propio": ("C89", "DOOM son ~50 ficheros con sus cabeceras",
                        None),  # necesita dos ficheros: la sonda lo monta aparte
    "#if aritmetico": ("C89", "DOOM: #if defined(NORMALUNIX)",
                       "#define N 2\n#if N > 1\nint main(){return 1;}\n#else\nint main(){return 0;}\n#endif"),

    # ── Cadenas y memoria ──
    "literal de cadena": ("C89", "esencial", "int main(){char *s=\"hola\";return s[0];}"),
    "array de char inicializado": ("C89", "DOOM: tablas de nombres de sprite",
                                   "int main(){char s[4]=\"abc\";return s[0];}"),
    "aritmetica de punteros": ("C89", "DOOM: recorre el framebuffer con punteros",
                               "int main(){int v[4];int *p=v;p++;*p=3;return v[1];}"),
}
