# QUE DESBLOQUEA QUE -- el censo de lo que BMO-X puede correr

> Escrito el **2026-08-02**, a partir de la superficie del sistema **medida**,
> no supuesta.
>
> Vive en `docs/` y no en `toolchain/lang/cpp/` **a proposito**: la tesis del
> documento es que esto no es una pregunta sobre C++. Ponerlo dentro de C++ lo
> contradiria.

## La frase que reordena todo

> **C++ no desbloquea aplicaciones. Lo que desbloquea aplicaciones es la
> SUPERFICIE DEL SISTEMA.**
>
> C++ desbloquea *escribir cosas grandes sin que se hagan ingobernables*, que
> es otra cosa y tambien vale.

La prueba esta en la propia lista: casi todo lo valioso que se podria portar
esta escrito en **C**, y BMO C ya pasa 32 de 32 sondas.

---

## Lo que BMO-X tiene HOY

Medido sobre `platform/abi/bmo-abi/src/syscalls/surface/` y los drivers del
arbol.

> ★★ **AL 2026-08-18: 2 syscalls y 88 OPERACIONES.** El mismo `grep` sobre el
> mismo directorio, en tres fechas del arbol: **69** el 11-08 (`cf878698`),
> **73** el 14-08 (`324f7654`), **88** hoy. O sea +27% en una semana, y son 43
> `TASK`, 9 `ARCH`, 6 `INPUT`, 4 `AUDIO`, 4 `FB`, 4 `PRESTADO`, 4 `CONSOLA`...
>
> Dos cosas de esa cifra, y ninguna es cosmetica. La primera: **la fuente de la
> medicion se movio** -- `surface.rs` es hoy un directorio de siete ficheros, y
> por eso el numero envejecio sin que nadie lo notara. La segunda: **dos puertas
> es la FORMA, 88 es el TAMANO**, y prometer que la superficie cabe en la cabeza
> hay que hacerlo con el numero de hoy. Donde va lo que crece sin tocar la
> puerta esta en [`META-SDK_HARD.md`](../../META-SDK_HARD.md) 1.1: *comodidad es
> cabecera, autoridad es operacion*.

> **Al 2026-08-11: 2 syscalls y 39 operaciones.** Cuando este documento se
> escribio eran 3 y 22, y las dos mitades de ese cambio dicen lo mismo:
> `CHANNEL_KICK` se retiro el 10-08 --era una operacion sobre un handle, o sea
> la definicion de `INVOKE`-- y las operaciones casi se doblaron. **La puerta se
> hizo mas estrecha mientras el sistema crecia**, que es exactamente lo que el
> diseno prometia y ahora tiene numeros.

| Pieza | Estado | Que habilita |
|---|---|---|
| Framebuffer + doble bufer | ✅ corre en el Ryzen | todo lo 2D y todo el *software rendering* |
| Teclado + raton USB | ✅ en metal | entrada real |
| Tiempo (TSC) + espera | ✅ | bucles de juego, temporizacion |
| **Memoria (`KIND_MEMORIA`)** | ✅ `TASK_OP_MEMORIA_PEDIR`, `MEM_OP_BASE`, `MEM_OP_BYTES` | ★ `malloc`/`new` **ya no esta bloqueado**; falta el asignador encima, que es codigo de usuario |
| Ficheros | ✅ abrir / crear / leer / leer-linea / escribir / tamano / cerrar | E/S de datos, FAT32 lectura + ESTRATOS |
| Consola | ✅ escribir y leer | stdin/stdout |
| Lanzar programas, rutas, info | ✅ | un shell de verdad |
| **Red** | ⏳ **la NIC se RECONOCE** (11-08): `find_net` + MAC + enlace, cero escrituras. Sin `KIND_RED`, sin anillos y **sin pila TCP/IP** | nada conectado todavia |
| **Hilos** | ❌ **cero syscalls de crear hilo** | una tarea = un hilo |
| **Compilacion separada** | ❌ **una sola unidad de traduccion** | obligatorio *unity build* |
| Enlazado dinamico | ❌ y no hace falta | todo estatico |
| GPU | ❌ (el perfil RDNA4 esta reservado, sin hardware) | nada de OpenGL/Vulkan |
| Fuentes vectoriales | ❌ solo mapa de bits (`fontgen`) | texto de bitmap |

### ★ Por que el driver de red "no resuelve nada"

Es la observacion correcta y conviene tenerla escrita, porque se repite con
cada driver:

**Un driver de NIC te da TRAMAS, no conexiones.** El e1000 pone bytes en el
cable y los saca. Entre eso y `descargar una pagina` hay, en orden: ARP, IPv4,
ICMP, UDP, **TCP** (ventanas, retransmision, control de congestion), DNS,
HTTP, y **TLS** (que por si solo es criptografia, certificados y una maquina de
estados grande).

**El driver es el 5% del problema. La pila es el 95%.** Y la pila es
independiente del hardware -- la misma vale para cualquier NIC.

Atajo real y anotado: **smoltcp** (Rust, `no_std`) es una pila TCP/IP pensada
exactamente para esto. TLS sigue siendo aparte.

> ⚠ **2026-08-11**: `smoltcp` era dependencia del crate `bmo-net` y se quito con
> el resto. No era una decision contra ella -- es que **la pila no va en Ring
> 0**, y aquel crate estaba en el kernel. Cuando vuelva sera dentro de un
> programa de Ring 3, que es su sitio. Ver [`RED_MAESTRO.md`](../maestro/RED_MAESTRO.md).

---

## Nivel 0 -- no necesita C++ en absoluto

C ya esta en 32/32. Todo esto se porta con el frontend que ya existe.

| App | Lengua | Tamano | Que falta | Tiempo |
|---|---|---|---|---|
| **DOOM** | C | ~35k | libc (`malloc`, `sprintf`, `atoi`, `exit`), unity build | ★ objetivo ya declarado -- **semanas** |
| **Lua** | C | ~30k | libc + `setjmp`/`longjmp` | semanas |
| **SQLite** | C | ~150k, **un solo fichero** | libc + VFS sobre `ARCH_OP_*` | ★ el *amalgamation* ya es unity build **por diseno** -- semanas |
| zlib, stb_*, libpng, libjpeg | C | 5-30k c/u | libc | dias cada una |
| **Quake 1** (renderer software) | C | ~100k | libc; unity build a 100k empieza a doler | 1-2 meses |
| **NetSurf** (navegador propio) | C | ~200k | **red + TLS + FreeType** | bloqueado por RED, no por lenguaje |
| Git, Vim, CPython | C | 250k-400k | POSIX grande: `fork`, senales, `mmap`, permisos | **no con 22 operaciones** |

### ★★ DOOM, ya no estimado: MEDIDO (2026-08-08)

La fila de arriba decia *"libc (`malloc`, `sprintf`, `atoi`, `exit`), unity
build"*. Era una suposicion razonable y **tres de sus cuatro partes no eran el
problema**. Se bajo el codigo (`ozkl/doomgeneric`, GPL-2.0) y se conto.

**El tamano real**: 56.465 lineas de C en **81 ficheros** de nucleo, mas 10.637
de cabeceras. 49 funciones distintas de libc.

★ **Tres hallazgos que cambian la estimacion, y los tres van en contra de lo
que se suponia:**

1. **DOOM no necesita coma flotante.** El unico `atan()` del renderer esta
   dentro de un `#if 0` con el comentario *"UNUSED - now getting from
   tables.c"*. Quitando `fabs` (aceleracion del raton) y `atof` (parsear el
   config), el motor entero es punto fijo -- que es exactamente lo que
   `c/ray.bex` ya demostro en el Ryzen.
2. **El tope de 4 `malloc` por proceso NO bloquea DOOM.** `I_ZoneBase` pide **un
   solo bloque** (`DEFAULT_RAM 6` MiB) y `Z_Malloc` reparte todo desde dentro.
   Es el caso que `KIND_MEMORIA` ya sirve: el compositor tiene 8,4 MiB por esa
   via.
3. **Lo que si es trabajo es `fprintf`: 64 llamadas**, mas `vsnprintf` y
   `vfprintf`. No es una funcion suelta, es la familia de `printf` con destino.

Y la capa de plataforma son **seis funciones** (`DG_Init`, `DG_DrawFrame`,
`DG_SleepMs`, `DG_GetTicksMs`, `DG_GetKey`, `DG_SetWindowTitle`). Las seis
existen ya en BMO.

#### El metodo, que vale mas que el resultado

Los 81 ficheros se compilan **UNO A UNO** y se guarda el primer error de cada
uno. Un unity build se para en el primero y te cuenta UNA cosa; 81 primeros
errores en una pasada son una **distribucion**: que rompe muchas veces y que
rompe una sola.

La primera pasada dio **0 de 81, y los 81 fallos eran CINCO causas** -- todas
del front (preprocesador y declaraciones), ninguna del generador de codigo.

Siete tandas despues (`ee090428` .. `9d661bdc`), el 2026-08-08:

```
   ficheros sueltos:  0 -> 7 -> 27 -> 35 -> 41 -> 47 -> 55 -> 61 -> 67 -> 69
   unity build:       PARSEA LAS 56.465 LINEAS y esta dentro del generador
```

★ **Y a partir de los 67 lo que falla ya no es el lenguaje**: nueve de los que
quedan son un simbolo definido en OTRO fichero, y eso una unidad de traduccion
sola no lo resuelve por definicion. Por eso el numero que importa dejo de ser el
de ficheros sueltos y paso a ser el **unity build** -- los 81 concatenados, que
es como se compila DOOM aqui porque BMO C no enlaza.

★★ **Donde se para el unity, y tiene nombre**: `printf` con el formato calculado
en tiempo de ejecucion. Es la pieza **"libc para DOOM"** de esta misma hoja: un
formateador que recorra la cadena al vuelo, que es lo que piden `I_Error(fmt,
...)` y `M_snprintf`. Delante ya no hay nada desconocido.

★★ Y la segunda tanda destapo un fallo que no era de DOOM: **`grid[1][0]` leia
`grid[0][2]`**. El paso de un indice contestaba 8 para cualquier array de
arrays, cuando un paso del indice de fuera es una FILA entera. Compilaba, corria
e imprimia un numero plausible. Lo cazo la fila nueva del banco **porque ejecuta
el programa**; y el emulador, que no tenia `imul reg, r/m, imm`, dio panic con
el opcode en la mano en vez de inventarse el valor.

⚠ Y dos de esos "fallos del compilador" eran de las cabeceras de sonda: un
`<stdbool.h>` sin `__bool_true_false_are_defined` se llevo 52 ficheros por
delante, y un `<inttypes.h>` vacio otros 77. **Un stub equivocado no informa de
una verdad mas pequena: informa de otra.**

#### Lo que falta para DOOM, en orden

| # | Pieza | Tamano |
|---|---|---|
| 1 | ~~Declaradores con coma, `[]` sin medida, `inline`, `#define` con `\`, `[a][b]`, punteros a funcion como parametro~~ -- ✅ **hechas el 08-08**. Lo que queda del front: una invocacion de macro-funcion repartida en varias lineas, `%p` en `printf`, y unos pocos casos sueltos que el guion de la sonda lista uno a uno | dias |
| 2 | **Compilacion separada, o un unity build de 56k lineas** | ★ el techo de verdad |
| 3 | La familia `fprintf`/`vsnprintf` | mediana |
| 4 | ~20 funciones triviales (`toupper`, `isspace`, `atoi`, `strncpy`, `strrchr`, `strstr`, `strdup`, `memmove`, `strcasecmp`, `ftell`, `feof`, `fwrite`) | una tarde |
| 5 | `system`, `mkdir`, `getenv`, `remove`, `rename` -- todas en buscar el WAD y guardar partida | se apuntalan |
| 6 | Que el `.bex` quepa en **1 MiB** (`MAX_BEX`) | por medir |

El banco de sonda (cabeceras de sistema minimas y el guion) vive **fuera del
repo**, en `BMO-externo/doom-port/`: codigo GPL y un WAD no entran en un arbol
con licencia Techne.

## Nivel 1 -- C++ acotado + lo que ya hay

Asume los 6 pasos del frontend hechos (ver `toolchain/lang/cpp/BRECHA.md`).

| App | Tamano | Que usa de C++ | Tiempo tras el frontend |
|---|---|---|---|
| ★ **Dear ImGui** | ~40k, nucleo en pocos ficheros | clases, sobrecarga de operadores, contenedores propios (no usa la STL) | ★ **el mejor retorno de la lista**: una GUI completa de herramientas sobre el framebuffer crudo. **1-2 meses** |
| **Box2D** (fisica 2D) | ~15k | clases, herencia simple, virtuales | ~1 mes |
| Juegos 2D propios | -- | RAII, plantillas basicas | inmediato |
| **Dune Legacy / OpenRA-tipo** | 50-150k | C++ acotado | 2-4 meses c/u |
| Editor / herramientas de BMO | -- | ★ donde C++ paga de verdad | -- |

## Nivel 2 -- pide **una** pieza de sistema que no existe

| App | Bloqueante REAL | Que cuesta el bloqueante |
|---|---|---|
| Cualquier navegador | **pila TCP/IP + TLS** | meses. smoltcp acorta la mitad; TLS es proyecto propio |
| Texto de calidad | **FreeType + shaping** | FreeType es C, ~150k -- semanas si hay libc |
| Servidores, bases de datos serias | red + hilos | -- |
| **OpenTTD, ScummVM, DOSBox** | ~600k C++ -- **unity build a 600k no es viable** | ★ **compilacion separada** |

## Nivel 3 -- subsistemas enteros (y no por C++)

> ⚠ Este apartado decia **"anos"** a secas. Se corrigio el 2026-08-04: un
> "anos" sin desglosar no es una estimacion, es una forma educada de no
> pensar. El desglose real esta en [LAS PIEZAS, CONTADAS](#-las-piezas-contadas)
> -- y de las tres filas de abajo, **dos estaban mal contadas**.

| App | Por que |
|---|---|
| Doom 3, cualquier motor 3D moderno | GPU + driver + OpenGL/Vulkan |
| **V8 / cualquier JIT de JS** | paginas W+X y mapear codigo en ejecucion -- no existe en el modelo de capabilities |
| LibreOffice, Blender, Qt | millones de lineas + POSIX completo + hilos + GPU |

## ★ Nivel 4 -- la pregunta de Google, contestada sin adornos

**Chromium/Blink son del orden de 30 millones de lineas** con ~200 dependencias
de terceros. Necesita JIT (V8), multiproceso con IPC y sandbox, pila de red
completa con TLS y HTTP/2, HarfBuzz + FreeType + ICU, compositor por GPU,
audio, video y un POSIX de verdad.

**El frontend de C++ seria del orden del 2% del problema.** La respuesta honesta
es **no** -- y no por C++: Chromium tampoco corre sobre un Linux al que le
falten esas piezas.

**Pero un navegador si es alcanzable**: NetSurf, motor propio, ~200k lineas, en
**C**. Su bloqueante es la **red**.

---

## ★ Las palancas, ordenadas por lo que desbloquean

Si el criterio es *que cambia mas cosas por unidad de esfuerzo*, el orden **no**
es el que uno espera:

| # | Palanca | Que desbloquea | Necesita C++? |
|---|---|---|---|
| 1 | **Compilacion separada** | todo lo que pase de ~100k lineas. Hoy es el techo duro del sistema, **y es lo que bloquea a SDL** | **NO** |
| 2 | **Portar SDL 1.2** -- es C, y su capa de plataforma es pequena y esta bien delimitada | ★ **cientos** de juegos y aplicaciones de golpe, sin tocarlos uno a uno | **NO** |
| 3 | **Pila de red + TLS** | navegador, servidores, actualizaciones, todo lo conectado | **NO** |
| 4 | El **C++ acotado** | ImGui, Box2D, y escribir lo grande de BMO sin ahogarse | -- |
| ~~x~~ | ~~**libc: el asignador sobre `KIND_MEMORIA`**~~ | **HECHO** el 2026-08-09: `<bmo/monton.h>` | -- |

Las tres primeras son C y sistema. Ninguna pide clases.

### ⚠ La fila 1 y la 2 estaban al reves, y por que

Hasta el 2026-08-18 esta tabla ponia SDL en el puesto 1 con esta frase: *"su
capa de plataforma son ~4 funciones (video, entrada, audio, tiempo). **BMO ya
tiene las cuatro**"*. Medida contra el arbol, se cae por tres sitios:

1. **De las cuatro, hay dos y media.** Tiempo, entero. Video, entero --y encaja
   mejor de lo que decia: `SDL_Flip` **es** `R-APP4`. Entrada, solo por relevo
   de pantalla entera. **Audio, no**: `ring0/obj/audio.rs` abre diciendo *"esto
   no es un driver de audio"*, y debajo solo hay el altavoz del PC.
2. **Faltaba nombrar los HILOS.** SDL 1.2 trae subsistema de hilos y su audio
   arranca uno propio; aqui no hay hilos de Ring 3 y
   `toolchain/lang/c/BRECHA.md` lo repite cuatro veces. Se puede construir sin
   ellos, pero es una decision de diseno que hay que tomar antes.
3. ★★ **SDL son del orden de cien ficheros `.c`, y hoy solo hay unity build.**
   Dos `static` con el mismo nombre en ficheros distintos dejan de ocultarse y
   pasan a ser una redefinicion. O sea: **SDL no adelanta a la compilacion
   separada -- es el mejor argumento a favor de ella.**

★ Y la consecuencia que separa dos objetivos que estaban mezclados: **SDL no es
el camino para abrir DOOM.** DOOM ya corre sin SDL, con su propia capa de
plataforma; lo que le falta es sitio compartido y un fallo de codegen. SDL es el
camino para que **otro** traiga su juego.

## Y sobre la GPU

**No tener RDNA4 no es un problema, y el motivo es concreto**: todo el Nivel 0
y el Nivel 1 es *software rendering* al framebuffer. DOOM, Quake, ImGui y todo
lo 2D pintan pixeles a memoria -- que es exactamente lo que BMO ya hace, con
doble bufer y write-combining.

La GPU solo aparece en el **Nivel 3**, que esta fuera de alcance por otras diez
razones antes que por ella. Comprar una tarjeta hoy no adelantaria nada:
adelantaria el dia en que haga falta escribir un driver de RDNA4, que es el
proyecto que la usaria.

---

## Como se lee esta tabla dentro de un ano

La misma regla que el censo de C++: **un "no" con motivo escrito se puede
discutir; uno sin motivo es un agujero.** Cuando una fila de "que falta" se
cumpla, la app cambia de nivel. Ese es el uso del documento: no es una lista de
deseos, es un **mapa de dependencias**.

---

# ★★ LAS PIEZAS, CONTADAS

> Anadido el **2026-08-04**, a peticion del dueno y **con dos correcciones
> suyas incorporadas**. La version anterior de este documento despachaba la GPU
> con *"anos"* y el JIT con *"no encaja"*. Las dos eran pereza: un "anos" sin
> desglosar no es una estimacion, es una forma educada de no pensar.
>
> Aqui cada hueco se parte en **piezas contables**. Una pieza es algo que se
> puede empezar el lunes y terminar sabiendo si funciona.

## Por que contar piezas y no meses

Un mes es una sensacion; una pieza se termina o no se termina. Y contar obliga
a lo que de verdad importa: **descubrir que dentro de un hueco enorme hay tres
piezas faciles y una imposible** -- que es exactamente el caso de la GPU, y no
se veia diciendo "anos".

La columna que decide el orden no es el numero de piezas: es **cuantas de ellas
ya estan escritas**.

---

## 1 - EL ENLAZADOR -- ★★ 5/5, CERRADO EL 2026-08-07

**Camino B** (funciones sintetizadas), el decidido en `toolchain/forge/README.md`.

| # | Pieza | Estado |
|---|---|---|
| 1 | Tabla de funciones sintetizables: nombre -> bytes | ✅ el mecanismo **ya corria en metal** con `__bmo_syscall_stub`, cableado para un unico nombre |
| 2 | El codegen inyecta la funcion una vez y relocaliza las llamadas | ✅ `ea3429f4` -- `codegen/sintetizadas.rs`. El stub es la primera entrada de la tabla a proposito: si no reprodujera el caso que ya funcionaba, no serviria |
| 3 | `malloc`/`free` de verdad sobre `KIND_MEMORIA` | ✅ `bmo-rt::heap::freelist`, 247 lineas -- **"probada" es cierto desde el 2026-08-07 y no antes** (`7b2a3a73`): los 6 tests no enlazaban en el host por el `_start` de `crt0`, asi que `cargo test -p bmo-rt` no ejecutaba ninguno. Ahora 6/6 |
| 4 | `printf` minimo (`%d %s %x %c`) | ✅ `5f644aae` -- las cinco conversiones a la tabla. **-8,2% de codigo en los seis ejemplos** (-25,2% en `holac`) |
| 5 | Cadenas: `strlen` `strcpy` `memcpy` `memset` | ✅ `bfcaf45b` -- mas `strcmp`, `strchr`, `strncmp`, `memcmp`. **-32,2%** en un programa que las usa |

★★ **Era el hueco mas barato de todo el sistema y el que mas desbloquea**, y ya
no esta. Con esto **DOOM, Lua y SQLite dejan de estar bloqueados por el
lenguaje** -- que no es lo mismo que estar listos: siguen bloqueados por la
compilacion separada a partir de ~100k lineas (section  *Que desbloquea que*, fila 2).

### ⚠ Tres cosas que este 5/5 **no** significa, y conviene leerlas

**1. El ahorro no se ve en el `.bex`.** Los seis ejemplos miden exactamente lo
mismo que antes, byte por byte. La seccion de codigo se rellena a 4096
(`pad_to_page`, con `0xCC`) y 2 KB de ahorro caben dentro del relleno. Ese
relleno existe porque **BEF no tiene relocations** -- lo dice la cabecera de
`patch_all_fixups`--, asi que hasta que las tenga, todo ahorro de codigo por
debajo de una pagina es invisible en el fichero. Es la deuda que hay que pagar
para que esto se note.

**2. Lo que quedo fuera, y con criterio medido.** Enlazar cuesta ~10 bytes por
llamada; en linea, ~3 mas el cuerpo. Asi que **`abs` (13 bytes) se queda en
linea**: apenas pasa del coste de llamarlo. Y `malloc`/`free` no pueden entrar
todavia porque usan `fresh_label()`, que es estado del `Codegen`, y un
`Sintetizador` solo recibe `&mut Vec<u8>`. La regla no es "todo a la tabla".

**3. `memmove` sigue roto.** Comparte el `copiar` de `memcpy`, que avanza de
principio a fin, asi que con solapamiento y `dst > src` corrompe -- que es
exactamente lo que `memmove` promete y `memcpy` no. **Hoy es un `memcpy` con
otro nombre.**

---

## 2 - LAS TRES OPS DE `KIND_ARCHIVO` -- 3 piezas pequenas

Baratas desde que `3.0` (reemplazar en FAT32) esta hecho.

| # | Pieza | Nota |
|---|---|---|
| 1 | ~~`ARCH_OP_POSICIONAR`~~ | ★ **HECHO** (`b791ce4b`, 2026-08-07) con otro nombre: `ARCH_OP_SALTAR` = `0x07`, mas `ARCH_OP_LEER_EN` = `0x06` para leer un bloque de golpe. Lo despacha `obj/archivo.rs`, y `fseek`/`fread` de `bmo/archivo.h` lo llaman. **Esta fila siguio diciendo "pendiente" con el nombre viejo**, que es la forma mas cara de equivocarse en un mapa de dependencias: hace parecer bloqueado lo que ya corre |
| 2 | Modo I-O | el fichero entero ya vive en RAM: es el modo, no el modelo |
| 3 | Modo EXTEND | `CURSOR = LARGO` al abrir |

Detras caen RRDS, ESDS y **KSDS** -- o sea el indice, o sea la banca.

---

## 3 - ESTRATOS ESCRIBIR (`3.4`) -- 5 piezas, y una ya esta

| # | Pieza | Estado |
|---|---|---|
| 1 | Reservar bloques libres | -- |
| 2 | Escribir un nodo nuevo (`encode` + `escribir_bloque`) | `escribir_bloque` ya existe |
| 3 | **Escribir un flujo**: el arbol de datos, el inverso de `descender` | la pieza gorda |
| 4 | Crear/actualizar el `:entradas` del padre | -- |
| 5 | Encadenar el estrato y sellar | ★ **`sellar()` ya commitea en el Ryzen** |

---

## 4 - HILOS -- 4 piezas

| # | Pieza |
|---|---|
| 1 | `TASK_OP_HILO_CREAR` en la superficie |
| 2 | Pila por hilo |
| 3 | Planificar: **el round-robin ya existe**, hay que dejarle meter hilos de un mismo proceso |
| 4 | Esperar/unir (`join`) |

**No lo pide la banca** -- un batch es E/S secuencial. Lo piden Chrome y Steam.

---

## 5 - RED -- 5 piezas, cuatro razonables y una enorme

| # | Pieza | Tamano |
|---|---|---|
| 1 | [!] **CORREGIDO 2026-08-24.** Decia *"cablear el e1000"*, y el e1000 **ya no existe**: era la NIC de QEMU y esta maquina lleva una **Realtek RTL8111/8168**. Las 287 lineas se borraron. Lo que hay es un perfil del aparato de verdad, con el paso 0 **verificado en el Ryzen** y el anillo RX escrito. Falta TX | ~300 lineas |
| 2 | **smoltcp**: ARP, IPv4, ICMP, UDP, TCP | crate `no_std` **ya hecha** -- integrarla. ** Y traerla es CORRECTO por la ley 24: una pila TCP es **software**, no nombra ningun aparato. Traer un driver de NIC generico era lo incorrecto, y es la misma ley contestando distinto a los dos lados |
| 3 | `KIND_SOCKET` y sus operaciones | dias |
| 4 | DNS | dias |
| 5 | **TLS** | ★ proyecto propio: criptografia, certificados, maquina de estados |

Sin la 5 no hay nada conectado que sirva hoy. Con las cuatro primeras hay red
de area local, que ya es mucho para un banco con terminales.

---

## 6 - GPU + VULKAN -- 6 piezas, y **la correccion del dueno era justa**

> *"No subestimes, no es anos; no es meter Vulkan entero, es meter Vulkan 1.0
> hasta 1.3 con proceso -- por algo se llama estrategia."*

Tiene razon y el "anos" de este documento estaba mal escrito. Desglosado:

| # | Pieza | Realidad |
|---|---|---|
| 1 | Enumerar PCIe y mapear los BARs | **dias** -- es leer una tabla |
| 2 | ★ **Inicializar el GPU**: power, clocks, cargar el microcodigo | **AQUI ESTA EL MURO.** AMD lo documenta a medias y el firmware es un blob binario |
| 3 | Anillos de comandos (GFX ring, DMA) | semanas, y esta documentado |
| 4 | Gestor de memoria de video: VRAM, GTT, page tables de la GPU | ★ meses, y es un asignador de verdad |
| 5 | Compilador de shaders: SPIR-V -> ISA de RDNA | ★ **es otro compilador entero** |
| 6 | La API Vulkan 1.0 encima: instance, device, queue, command buffer, pipeline, swapchain | meses, pero es fontaneria sobre 1-5 |
| + | 1.1 - 1.2 - 1.3 | **incrementos**, no reescrituras -- y ahi la estrategia del dueno es la correcta |

**La estimacion honesta corregida**: no son "anos de imposible". Son **seis
piezas de las que dos son proyectos propios** (la 4 y la 5) y **una es un muro
de documentacion** (la 2). Las otras tres son trabajo normal.

Y sigue sin servir a la banca. Pero ya no esta mal contado.

---

## 7 - EL JIT SIN ROMPER LAS CAPABILITIES -- 3 piezas

> *"Podria crear un intermedio para que no choque con capabilities."*

★★ **La idea del dueno es correcta y mejor que la objecion original de este
documento.** Decir "W+X contradice el modelo" era mirar solo el caso malo. Lo
que hace falta no es W+X -- es **W^X con transicion explicita**, y eso no
contradice las capabilities: **es exactamente como se expresan.**

| # | Pieza |
|---|---|
| 1 | `KIND_CODIGO`: memoria que nace **escribible y NO ejecutable** |
| 2 | `SELLAR`: la vuelve ejecutable **y revoca el derecho de escritura en el mismo acto** |
| 3 | La garantia: un proceso **nunca** tiene los dos derechos sobre la misma pagina a la vez |

Con eso un JIT funciona --escribe, sella, ejecuta, y para regenerar pide otra--
y el sistema **nunca** ha entregado una pagina W+X. Es lo que hacen macOS
(`MAP_JIT`) y OpenBSD (`W^X`), y encaja aqui mejor que en ellos porque aqui el
derecho es un objeto y no un bit de una `mmap`.

**No sirve a la banca ni desbloquea Chrome por si solo.** Pero es una pieza de
diseno que estaba mal descartada, y descartarla mal habria cerrado una puerta
por un motivo falso.

---

## ★ EL ORDEN, por lo que cuesta terminarlo

Ordenado por **piezas que faltan de verdad**, no por piezas totales:

| Hueco | Piezas | Ya escritas | **Faltan** | Sirve al banco? |
|---|---|---|---|---|
| **Las 3 ops de `KIND_ARCHIVO`** | 3 | 0 | **3 pequenas** | ★★★ |
| **El enlazador** | 5 | 1 | **4** | ★★★ |
| **ESTRATOS escribir** | 5 | 1 | **4**, una gorda | ★★★ |
| El JIT sin romper capabilities | 3 | 0 | 3 | ✗ |
| Hilos | 4 | 1 | 3 | ✗ |
| Red sin TLS | 4 | 1 | 3 | ◐ |
| Red con TLS | 5 | 1 | 4, **una enorme** | ◐ |
| GPU + Vulkan 1.0 | 6 | 0 | **6, dos son proyectos** | ✗ |

**Las tres primeras filas son las mismas tres que pide la banca.** No hace
falta elegir entre "madurar el sistema" y "llegar al banco": durante los tres
primeros huecos **son el mismo trabajo**.

A partir del cuarto se separan, y entonces si hay que elegir -- pero para
entonces el sistema tendra enlazador, libc e indice, que es otro sistema.
