# BRECHA — lo que le falta a BMO C, medido y no opinado

> **AUTO-GENERADO** por `toolchain/tools/c-gen/generate.py`. No editar a mano:
> se regenera con `py toolchain/tools/c-gen/generate.py`.

Cada fila de este documento sale de una **sonda**: un programa de C minimo que
se le da a BMO C. Si compila, la fila dice `si`. Si no, dice el error EXACTO que
devolvio el compilador.

Eso importa mas de lo que parece. Leer el lexer diria que *palabras* reconoce
BMO C — y `static` esta en el lexer de cualquier compilador de juguete que
luego no sabe que hacer con ella. Aqui se pregunta lo unico que decide:
**¿compila?**

Es el mismo criterio que el banco de pruebas de BMO C (que EJECUTA los
programas en vez de mirar volcados de bytes) y el mismo que `VERDAD.md` aplica
al hardware. Un informe deducido de las fuentes envejece el dia que alguien
toca las fuentes; uno que compila se actualiza solo.


Medido el **2026-08-02**.

## El numero

**31 de 31** sondas del lenguaje compilan.

## El lenguaje, sonda a sonda

| Caracteristica | Era | ¿Compila? | Para que |
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
| enum | C89 | **si** | esencial |
| extern | C89 | **si** | declarar sin definir; obligatorio si hay varios ficheros |
| for con declaracion | C99 | **si** | comodidad; DOOM es C89 y no lo necesita |
| goto | C89 | **si** | DOOM lo usa poco pero lo usa |
| literal de cadena | C89 | **si** | esencial |
| long long | C99 | **si** | no lo pide DOOM; util para contadores |
| operador ternario | C89 | **si** | esencial |
| prototipo (param con nombre) | C89 | **si** | obligatorio para llamar antes de definir |
| prototipo (param sin nombre) | C89 | **si** | asi los escribe DOOM en sus cabeceras |
| puntero a funcion | C89 | **si** | DOOM: think_t, actionf_t — el corazon de sus actores |
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

## libc — y el destinatario de cada funcion

★ La columna que decide no es *existe en el estandar*, es **para**
**que**. Una lista de libc sin motivo al lado es una invitacion a
implementarla entera, que es exactamente el fallo que la hoja de ruta
descarta con nombre propio.

### Lo que BMO necesita para lo suyo

| Funcion | Cabecera | ¿Compila? | Para que |
|---|---|---|---|
| printf | `stdio.h` | **si** | ya esta: es lo primero que hizo BMO C |
| getchar | `stdio.h` | **si** | ya esta: verificado en el Ryzen |
| scanf | `stdio.h` | **si** | ya esta: pregc.bex pregunta la edad |

### Lo que pide el objetivo de prueba

| Funcion | Cabecera | ¿Compila? | Para que |
|---|---|---|---|
| puts | `stdio.h` | **NO** | una linea y salto; trivial encima de printf |
| sprintf | `stdio.h` | **NO** | DOOM formatea en buffers, no solo en pantalla |
| malloc | `stdlib.h` | **NO** | ★ DOOM pide UN bloque grande (Z_Zone) y se lo administra el |
| free | `stdlib.h` | **NO** | pareja de malloc; con Z_Zone se llama poquisimo |
| memset | `string.h` | **NO** | limpiar el framebuffer y las estructuras |
| memcpy | `string.h` | **NO** | ★ el blit de cada fotograma pasa por aqui |
| strlen | `string.h` | **NO** | esencial en cuanto hay texto |
| strcmp | `string.h` | **NO** | DOOM busca lumps del WAD por nombre |
| strcpy | `string.h` | **NO** | pareja obligada de strcmp |
| abs | `stdlib.h` | **NO** | el render lo usa a manos llenas |
| atoi | `stdlib.h` | **NO** | parametros de linea de ordenes |
| fopen/fread | `stdio.h` | — | ★ el WAD son 4 MB. BMO ya tiene KIND_ARCHIVO |
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
—esencia contra `VENDOR:`— y por la misma razon: un compilador que
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

## Lo que este documento NO dice

Que una sonda compile **no** significa que el programa haga lo
correcto: eso lo prueba el banco de Rust, que ejecuta. Aqui se
pregunta si el compilador ACEPTA la construccion, que es la primera
de las dos preguntas y la que decide si 35.000 lineas ajenas tienen
alguna posibilidad de entrar.

