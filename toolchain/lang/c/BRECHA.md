# BRECHA -- lo que le falta a BMO C, medido y no opinado

> **AUTO-GENERADO** por `toolchain/tools/c-gen/generate.py`. No editar a mano:
> se regenera con `py toolchain/tools/c-gen/generate.py`.

Cada fila de este documento sale de una **sonda**: un programa de C minimo que
se le da a BMO C. Si compila, la fila dice `si`. Si no, dice el error EXACTO que
devolvio el compilador.

Eso importa mas de lo que parece. Leer el lexer diria que *palabras* reconoce
BMO C -- y `static` esta en el lexer de cualquier compilador de juguete que
luego no sabe que hacer con ella. Aqui se pregunta lo unico que decide:
**compila?**

Es el mismo criterio que el banco de pruebas de BMO C (que EJECUTA los
programas en vez de mirar volcados de bytes) y el mismo que `VERDAD.md` aplica
al hardware. Un informe deducido de las fuentes envejece el dia que alguien
toca las fuentes; uno que compila se actualiza solo.


Medido el **2026-08-02**.

## El numero

**32 de 32** sondas del lenguaje compilan.

## El lenguaje, sonda a sonda

| Caracteristica | Era | Compila? | Para que |
|---|---|---|---|
| #define con argumentos | C89 | **si** | DOOM: FixedMul, MAXPLAYERS... por todas partes |
| #if aritmetico | C89 | **si** | DOOM: #if defined(NORMALUNIX) |
| #include propio | C89 | **si** | DOOM son ~50 ficheros con sus cabeceras |
| aritmetica de punteros | C89 | **si** | DOOM: recorre el framebuffer con punteros |
| array de char inicializado | C89 | **si** | DOOM: tablas de nombres de sprite |
| array de struct | C89 | **si** | DOOM: tablas de estados, sprites, sectores |
| array dentro de struct | C89 | **si** | DOOM: `char nombre[8]` en cada lump del WAD |
| array dentro de union | C89 | **si** | DOOM: la union de datos del WAD |
| auto | C89 | **si** | redundante desde 1978; en C23 cambio de significado |
| bitfields | C89 | **si** | poco usado en DOOM; caro de emitir |
| declaradores multiples | C89 | **si** | `int a, b;` -- DOOM lo usa en cada fichero |
| enum | C89 | **si** | esencial |
| extern | C89 | **si** | declarar sin definir; obligatorio si hay varios ficheros |
| for con declaracion | C99 | **si** | comodidad; DOOM es C89 y no lo necesita |
| goto | C89 | **si** | DOOM lo usa poco pero lo usa |
| literal de cadena | C89 | **si** | esencial |
| long long | C99 | **si** | no lo pide DOOM; util para contadores |
| operador ternario | C89 | **si** | esencial |
| prototipo (param con nombre) | C89 | **si** | obligatorio para llamar antes de definir |
| prototipo (param sin nombre) | C89 | **si** | asi los escribe DOOM en sus cabeceras |
| puntero a funcion | C89 | **si** | DOOM: think_t, actionf_t -- el corazon de sus actores |
| recursion | C89 | **si** | esencial |
| register | C89 | **si** | hoy no aporta nada: todos los compiladores lo ignoran |
| static (global) | C89 | **si** | DOOM: una global por fichero, ocultada al enlazador |
| static (local) | C89 | **si** | DOOM lo usa en casi cada funcion |
| struct | C89 | **si** | esencial |
| switch con fallthrough | C89 | **si** | esencial |
| typedef | C89 | **si** | DOOM define fixed_t, mobj_t... todo pasa por aqui |
| union | C89 | **si** | DOOM la usa en sus thinkers |
| unsigned char | C89 | **si** | DOOM: el framebuffer es byte[] |
| varargs: declarar (...) | C89 | **si** | DOOM: I_Error(fmt, ...) |
| varargs: leerlos | C89 | **si** | sin esto, `...` compila y no sirve para nada |

## libc -- y el destinatario de cada funcion

★ La columna que decide no es *existe en el estandar*, es **para**
**que**. Una lista de libc sin motivo al lado es una invitacion a
implementarla entera, que es exactamente el fallo que la hoja de ruta
descarta con nombre propio.

### Lo que BMO necesita para lo suyo

| Funcion | Cabecera | Compila? | Para que |
|---|---|---|---|
| printf | `stdio.h` | **si** | ya esta: es lo primero que hizo BMO C |
| getchar | `stdio.h` | **si** | ya esta: verificado en el Ryzen |
| scanf | `stdio.h` | **si** | ya esta: pregc.bex pregunta la edad |

### Lo que pide el objetivo de prueba

| Funcion | Cabecera | Compila? | Para que |
|---|---|---|---|
| puts | `stdio.h` | **NO** | una linea y salto; trivial encima de printf |
| sprintf | `stdio.h` | **NO** | DOOM formatea en buffers, no solo en pantalla |
| malloc | `stdlib.h` | **si** | ★ DOOM pide UN bloque grande (Z_Zone) y se lo administra el |
| free | `stdlib.h` | **si** | pareja de malloc; con Z_Zone se llama poquisimo |
| memset | `string.h` | **si** | limpiar el framebuffer y las estructuras |
| memcpy | `string.h` | **si** | ★ el blit de cada fotograma pasa por aqui |
| strlen | `string.h` | **si** | esencial en cuanto hay texto |
| strcmp | `string.h` | **si** | DOOM busca lumps del WAD por nombre |
| strcpy | `string.h` | **si** | pareja obligada de strcmp |
| abs | `stdlib.h` | **si** | el render lo usa a manos llenas |
| atoi | `stdlib.h` | **NO** | parametros de linea de ordenes |
| fopen/fread | `stdio.h` | -- | ★ el WAD son 4 MB. BMO ya tiene KIND_ARCHIVO |
| exit | `stdlib.h` | **NO** | I_Quit. BMO ya sale por la puerta normal |

### Lo que NO entra, y por que

| Funcion | Cabecera | Motivo del rechazo |
|---|---|---|
| pow/sin/cos | `math.h` | DOOM NO usa coma flotante en el render: son tablas de punto fijo |
| pthread_* | `pthread.h` | no hay hilos de usuario y no los pide el objetivo |
| setlocale | `locale.h` | una libc de verdad empieza aqui y no acaba nunca |
| wchar_t / wcs* | `wchar.h` | la consola de BMO es de un byte por caracter a proposito |
| signal | `signal.h` | no hay senales que mandar: aqui un fallo mata la tarea y lo dice |
| setjmp/longjmp | `setjmp.h` | DOOM no lo necesita y emitirlo pide guardar el marco entero |

## Lo que traen GCC, LLVM y MSVC encima del estandar

Esta lista no esta para copiarla: esta para **reconocerla y**
**rechazarla**. Es el mismo reparto que ya hace el COBOL de BMO
--esencia contra `VENDOR:`-- y por la misma razon: un compilador que
persigue las extensiones de otros tres no termina nunca.

★ Con una excepcion honesta: **DOOM se escribio para GCC en 1993**.
Si su codigo usa una extension, el rechazo no puede ser *no*: tiene
que ser *no, y esto es lo que se hace en su lugar*.

| Extension | De quien | Veredicto | La salida |
|---|---|---|---|
| `__attribute__((packed))` | GCC/Clang | **RECHAZAR** | DOOM lo usa en las estructuras del WAD. Salida: leer los campos byte a byte al cargar, que ademas arregla el endianness de paso |
| `__attribute__((noreturn))` | GCC/Clang | **RECHAZAR** | es una pista para el optimizador, no cambia lo que el programa hace: se ignora |
| `__declspec(dllimport)` | MSVC | **RECHAZAR** | no hay DLLs en BMO: un .bex es una imagen entera |
| `asm inline` | los tres | **RECHAZAR** | BMO ya tiene sem-asm, que es asm con nombres y tabla. No hacen falta dos |
| `typeof` | GCC/Clang | **MIRAR** | en C23 es estandar. Si entra, que entre como C23 y no como extension |
| `expresiones de sentencia ({...})` | GCC/Clang | **RECHAZAR** | DOOM no las usa; complican el parser para nada |
| `arrays de longitud cero` | GCC | **RECHAZAR** | C99 tiene miembros de array flexible: eso si es estandar |
| `#pragma once` | los tres (de facto) | **ACEPTAR** | no es del estandar y lo implementa todo el mundo. Cuesta cuatro lineas y evita el guardas de cabecera en 50 ficheros |
| `__builtin_expect` | GCC/Clang | **RECHAZAR** | optimizacion. Se ignora sin cambiar el resultado |
| `long double de 80 bits` | GCC | **RECHAZAR** | DOOM no usa coma flotante; el decimal exacto ya lo da COBOL/Ada |

## Los testigos de esta maquina

| Compilador | Estado |
|---|---|
| GCC | no esta instalado |
| LLVM/Clang | no esta instalado |
| MSVC | no esta instalado |

**Ninguno de los tres esta instalado**, y el informe lo dice en vez
de inventarse sus datos. Un extractor que rellena huecos cuando no
encuentra la fuente es peor que uno que no encuentra nada: el
segundo te manda a instalar un compilador, el primero te manda a
depurar una mentira. Es la regla que `VERDAD.md` ya le aplica al
emulador.

Para que este apartado se llene:
`winget install LLVM.LLVM` (Clang trae `-dM -E`, que es lo que se usa).

## ★ El censo de C, entero -- y que se DESCARTA

Un compilador acotado no se define por lo que tiene: se define por
**lo que deja fuera a proposito**. Una lista de caracteristicas sin
veredicto es una lista de deberes; con veredicto es un *alcance* -- y
un alcance es lo que hace que esto se pueda terminar.

**91 elementos** en el censo:

| Veredicto | Cuantos | Que significa |
|---|---|---|
| **ESENCIA** | 47 | sin esto no es C. Entra, tarde o temprano |
| **UTIL** | 19 | aporta a lo que BMO hace. Entra cuando toque |
| **DESCARTAR** | 25 | existe en C y **no entra**, con su motivo |

O sea: **27 de cada 100 elementos de C se quedan fuera**, y cada
uno con un motivo que se puede discutir. `DESCARTAR` no es *nunca*: es
*no en este alcance*. El dia que el motivo caduque, la fila cambia.

### tipos

| Elemento | Era | Veredicto | Motivo |
|---|---|---|---|
| void | C89 | **ESENCIA** | el tipo de lo que no devuelve nada |
| char / signed / unsigned char | C89 | **ESENCIA** | el byte |
| short / unsigned short | C89 | **ESENCIA** | 16 bits |
| int / unsigned int | C89 | **ESENCIA** | el entero por defecto |
| long / unsigned long | C89 | **ESENCIA** | 64 bits en este ABI |
| long long | C99 | UTIL | ya esta; en x86-64 coincide con long |
| float | C89 | UTIL | esta; la banca NO lo usa (decimal exacto) |
| double | C89 | UTIL | esta; idem |
| long double (80 bits) | C89 | ~~FUERA~~ | el x87 de 80 bits es una rareza de Intel; el decimal exacto ya lo dan COBOL y Ada |
| _Bool | C99 | UTIL | un int de 0/1; barato |
| _Complex / _Imaginary | C99 | ~~FUERA~~ | numeros complejos en un SO de banca: nadie los ha pedido nunca |

### calificadores

| Elemento | Era | Veredicto | Motivo |
|---|---|---|---|
| const | C89 | **ESENCIA** | esta |
| volatile | C89 | **ESENCIA** | esta; obligatorio para MMIO |
| restrict | C99 | ~~FUERA~~ | es una promesa al OPTIMIZADOR. No cambia lo que el programa hace |
| _Atomic | C11 | ~~FUERA~~ | no hay hilos de usuario. Cuando haya SMP se vuelve a mirar |

### almacenamiento

| Elemento | Era | Veredicto | Motivo |
|---|---|---|---|
| auto | C89 | UTIL | aceptado y tirado: redundante desde 1978 |
| register | C89 | UTIL | aceptado y tirado: todos lo ignoran |
| static | C89 | **ESENCIA** | HECHO 2026-08-02 |
| extern | C89 | **ESENCIA** | esta |
| _Thread_local | C11 | ~~FUERA~~ | no hay hilos de usuario |

### derivados

| Elemento | Era | Veredicto | Motivo |
|---|---|---|---|
| punteros (multinivel) | C89 | **ESENCIA** | esta |
| arrays | C89 | **ESENCIA** | esta, tambien dentro de agregados |
| punteros a funcion | C89 | **ESENCIA** | esta; DOOM vive de ellos |
| struct / union / enum | C89 | **ESENCIA** | estan |
| campos de bits | C89 | UTIL | se aceptan SIN empaquetar; empaquetar es mascara y RMW en cada acceso |
| miembro de array flexible | C99 | UTIL | el `t x[]` final de un struct |
| VLA (array de longitud variable) | C99 | ~~FUERA~~ | pide reservar en la pila en ejecucion; C11 ya lo hizo opcional y casi nadie lo usa |

### funciones

| Elemento | Era | Veredicto | Motivo |
|---|---|---|---|
| prototipos | C89 | **ESENCIA** | HECHO 2026-08-02: sin esto no hay recursion mutua |
| varargs (...) | C89 | **ESENCIA** | HECHO 2026-08-02, con `__va_arg(i)` |
| inline | C99 | ~~FUERA~~ | sugerencia al optimizador |
| _Noreturn | C11 | ~~FUERA~~ | idem |
| K&R (parametros sin tipo) | C89 | ~~FUERA~~ | sintaxis obsoleta desde 1989; ni DOOM la usa |

### operadores

| Elemento | Era | Veredicto | Motivo |
|---|---|---|---|
| aritmeticos + - * / % | C89 | **ESENCIA** | estan |
| incremento/decremento ++ -- | C89 | **ESENCIA** | estan (pre y post) |
| relacionales == != < > <= >= | C89 | **ESENCIA** | estan |
| logicos && || ! | C89 | **ESENCIA** | estan, con cortocircuito |
| de bits & | ^ ~ << >> | C89 | **ESENCIA** | estan |
| asignacion compuesta (11) | C89 | **ESENCIA** | estan |
| acceso . -> [] () | C89 | **ESENCIA** | estan |
| &direccion / *indireccion | C89 | **ESENCIA** | estan |
| ternario ?: | C89 | **ESENCIA** | esta |
| coma | C89 | **ESENCIA** | esta |
| sizeof | C89 | **ESENCIA** | esta |
| cast | C89 | **ESENCIA** | esta, y trunca de verdad |
| _Alignof / _Alignas | C11 | ~~FUERA~~ | el alineado lo decide el layout |
| _Generic | C11 | ~~FUERA~~ | seleccion por tipo en macros. Es lo que C++ resuelve con sobrecarga |
| literales compuestos | C99 | UTIL | `(struct P){1,2}` -- azucar util |

### sentencias

| Elemento | Era | Veredicto | Motivo |
|---|---|---|---|
| expresion y bloque | C89 | **ESENCIA** | estan |
| if / else | C89 | **ESENCIA** | estan |
| switch / case / default | C89 | **ESENCIA** | estan, con fallthrough |
| while / do-while / for | C89 | **ESENCIA** | estan |
| break / continue | C89 | **ESENCIA** | estan |
| return | C89 | **ESENCIA** | esta |
| goto y etiquetas | C89 | **ESENCIA** | estan |
| sentencia vacia | C89 | **ESENCIA** | esta |
| declaracion mezclada con codigo | C99 | UTIL | esta |

### preprocesador

| Elemento | Era | Veredicto | Motivo |
|---|---|---|---|
| #define objeto | C89 | **ESENCIA** | esta |
| #define funcion | C89 | **ESENCIA** | esta |
| #define variadica | C99 | UTIL | esta |
| #include | C89 | **ESENCIA** | esta |
| #if / #ifdef / #ifndef / #elif / #else / #endif | C89 | **ESENCIA** | estan |
| #undef | C89 | **ESENCIA** | esta |
| #error | C89 | **ESENCIA** | esta |
| #pragma | C89 | UTIL | se ignora; `#pragma once` si conviene |
| # (stringize) y ## (pegado) | C89 | UTIL | los usa cualquier cabecera con macros serias |
| #line | C89 | ~~FUERA~~ | solo cambia los numeros de error |
| __FILE__ / __LINE__ | C89 | UTIL | un `assert` de verdad los pide |

### biblioteca

| Elemento | Era | Veredicto | Motivo |
|---|---|---|---|
| <stdio.h> | C89 | **ESENCIA** | printf/getchar/scanf estan; faltan puts/sprintf/ficheros |
| <string.h> | C89 | **ESENCIA** | memcpy/memset/strlen/strcmp/strcpy HECHOS |
| <stdlib.h> | C89 | **ESENCIA** | abs HECHO; malloc/free piden la capability de memoria |
| <stddef.h> | C89 | **ESENCIA** | size_t, NULL, offsetof -- tipos, no codigo |
| <stdint.h> | C99 | **ESENCIA** | int32_t y compania: puro typedef |
| <limits.h> / <float.h> | C89 | **ESENCIA** | constantes |
| <stdbool.h> | C99 | UTIL | tres macros |
| <stdarg.h> | C89 | UTIL | va_list sobre `__va_arg` |
| <ctype.h> | C89 | UTIL | isdigit y compania: una tabla de 256 |
| <assert.h> | C89 | UTIL | con __FILE__/__LINE__ |
| <time.h> | C89 | UTIL | hay TSC; falta calendario |
| <math.h> | C89 | ~~FUERA~~ | DOOM no usa coma flotante en el render; el decimal exacto ya esta en COBOL y Ada |
| <errno.h> | C89 | ~~FUERA~~ | un global de error es justo lo contrario de devolver el fallo |
| <signal.h> | C89 | ~~FUERA~~ | no hay senales: aqui un fallo mata la tarea y lo DICE |
| <setjmp.h> | C89 | ~~FUERA~~ | pide guardar el marco entero; nadie lo pide |
| <locale.h> | C89 | ~~FUERA~~ | una libc de verdad empieza aqui y no acaba |
| <wchar.h> / <wctype.h> / <uchar.h> | C89 | ~~FUERA~~ | la consola de BMO es de un byte por caracter a proposito |
| <threads.h> | C11 | ~~FUERA~~ | no hay hilos de usuario |
| <stdatomic.h> | C11 | ~~FUERA~~ | idem |
| <complex.h> / <tgmath.h> | C99 | ~~FUERA~~ | numeros complejos |
| <fenv.h> | C99 | ~~FUERA~~ | modos de redondeo del x87 |
| <inttypes.h> | C99 | ~~FUERA~~ | solo macros de formato para printf |
| <iso646.h> | C89 | ~~FUERA~~ | alias de `&&` para teclados sin `&`. 1995 |
| <stdalign.h> / <stdnoreturn.h> | C11 | ~~FUERA~~ | envoltorio de lo ya descartado |

## Lo que este documento NO dice

Que una sonda compile **no** significa que el programa haga lo
correcto: eso lo prueba el banco de Rust, que ejecuta. Aqui se
pregunta si el compilador ACEPTA la construccion, que es la primera
de las dos preguntas y la que decide si 35.000 lineas ajenas tienen
alguna posibilidad de entrar.

