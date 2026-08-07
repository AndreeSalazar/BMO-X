# QUÉ DESBLOQUEA QUÉ — el censo de lo que BMO-X puede correr

> Escrito el **2026-08-02**, a partir de la superficie del sistema **medida**,
> no supuesta.
>
> Vive en `docs/` y no en `toolchain/lang/cpp/` **a propósito**: la tesis del
> documento es que esto no es una pregunta sobre C++. Ponerlo dentro de C++ lo
> contradiría.

## La frase que reordena todo

> **C++ no desbloquea aplicaciones. Lo que desbloquea aplicaciones es la
> SUPERFICIE DEL SISTEMA.**
>
> C++ desbloquea *escribir cosas grandes sin que se hagan ingobernables*, que
> es otra cosa y también vale.

La prueba está en la propia lista: casi todo lo valioso que se podría portar
está escrito en **C**, y BMO C ya pasa 32 de 32 sondas.

---

## Lo que BMO-X tiene HOY

Medido sobre `platform/abi/bmo-abi/src/syscalls/surface.rs` (3 syscalls, **22
operaciones**) y los drivers del árbol.

| Pieza | Estado | Qué habilita |
|---|---|---|
| Framebuffer + doble búfer | ✅ corre en el Ryzen | todo lo 2D y todo el *software rendering* |
| Teclado + ratón USB | ✅ en metal | entrada real |
| Tiempo (TSC) + espera | ✅ | bucles de juego, temporización |
| **Memoria (`KIND_MEMORIA`)** | ✅ `TASK_OP_MEMORIA_PEDIR`, `MEM_OP_BASE`, `MEM_OP_BYTES` | ★ `malloc`/`new` **ya no está bloqueado**; falta el asignador encima, que es código de usuario |
| Ficheros | ✅ abrir / crear / leer / leer-línea / escribir / tamaño / cerrar | E/S de datos, FAT32 lectura + ESTRATOS |
| Consola | ✅ escribir y leer | stdin/stdout |
| Lanzar programas, rutas, info | ✅ | un shell de verdad |
| **Red** | ❌ **cero syscalls de red**. El e1000 son 287 líneas de esqueleto y **no hay pila TCP/IP** | nada conectado |
| **Hilos** | ❌ **cero syscalls de crear hilo** | una tarea = un hilo |
| **Compilación separada** | ❌ **una sola unidad de traducción** | obligatorio *unity build* |
| Enlazado dinámico | ❌ y no hace falta | todo estático |
| GPU | ❌ (el perfil RDNA4 está reservado, sin hardware) | nada de OpenGL/Vulkan |
| Fuentes vectoriales | ❌ sólo mapa de bits (`fontgen`) | texto de bitmap |

### ★ Por qué el driver de red "no resuelve nada"

Es la observación correcta y conviene tenerla escrita, porque se repite con
cada driver:

**Un driver de NIC te da TRAMAS, no conexiones.** El e1000 pone bytes en el
cable y los saca. Entre eso y `descargar una página` hay, en orden: ARP, IPv4,
ICMP, UDP, **TCP** (ventanas, retransmisión, control de congestión), DNS,
HTTP, y **TLS** (que por sí solo es criptografía, certificados y una máquina de
estados grande).

**El driver es el 5% del problema. La pila es el 95%.** Y la pila es
independiente del hardware — la misma vale para cualquier NIC.

Atajo real y anotado: **smoltcp** (Rust, `no_std`) ya está mencionado en la
cabecera del driver. Es una pila TCP/IP pensada exactamente para esto. TLS
sigue siendo aparte.

---

## Nivel 0 — no necesita C++ en absoluto

C ya está en 32/32. Todo esto se porta con el frontend que ya existe.

| App | Lengua | Tamaño | Qué falta | Tiempo |
|---|---|---|---|---|
| **DOOM** | C | ~35k | libc (`malloc`, `sprintf`, `atoi`, `exit`), unity build | ★ objetivo ya declarado — **semanas** |
| **Lua** | C | ~30k | libc + `setjmp`/`longjmp` | semanas |
| **SQLite** | C | ~150k, **un solo fichero** | libc + VFS sobre `ARCH_OP_*` | ★ el *amalgamation* ya es unity build **por diseño** — semanas |
| zlib, stb_*, libpng, libjpeg | C | 5–30k c/u | libc | días cada una |
| **Quake 1** (renderer software) | C | ~100k | libc; unity build a 100k empieza a doler | 1–2 meses |
| **NetSurf** (navegador propio) | C | ~200k | **red + TLS + FreeType** | bloqueado por RED, no por lenguaje |
| Git, Vim, CPython | C | 250k–400k | POSIX grande: `fork`, señales, `mmap`, permisos | **no con 22 operaciones** |

## Nivel 1 — C++ acotado + lo que ya hay

Asume los 6 pasos del frontend hechos (ver `toolchain/lang/cpp/BRECHA.md`).

| App | Tamaño | Qué usa de C++ | Tiempo tras el frontend |
|---|---|---|---|
| ★ **Dear ImGui** | ~40k, núcleo en pocos ficheros | clases, sobrecarga de operadores, contenedores propios (no usa la STL) | ★ **el mejor retorno de la lista**: una GUI completa de herramientas sobre el framebuffer crudo. **1–2 meses** |
| **Box2D** (física 2D) | ~15k | clases, herencia simple, virtuales | ~1 mes |
| Juegos 2D propios | — | RAII, plantillas básicas | inmediato |
| **Dune Legacy / OpenRA-tipo** | 50–150k | C++ acotado | 2–4 meses c/u |
| Editor / herramientas de BMO | — | ★ donde C++ paga de verdad | — |

## Nivel 2 — pide **una** pieza de sistema que no existe

| App | Bloqueante REAL | Qué cuesta el bloqueante |
|---|---|---|
| Cualquier navegador | **pila TCP/IP + TLS** | meses. smoltcp acorta la mitad; TLS es proyecto propio |
| Texto de calidad | **FreeType + shaping** | FreeType es C, ~150k — semanas si hay libc |
| Servidores, bases de datos serias | red + hilos | — |
| **OpenTTD, ScummVM, DOSBox** | ~600k C++ — **unity build a 600k no es viable** | ★ **compilación separada** |

## Nivel 3 — subsistemas enteros (y no por C++)

> ⚠ Este apartado decía **"años"** a secas. Se corrigió el 2026-08-04: un
> "años" sin desglosar no es una estimación, es una forma educada de no
> pensar. El desglose real está en [LAS PIEZAS, CONTADAS](#-las-piezas-contadas)
> — y de las tres filas de abajo, **dos estaban mal contadas**.

| App | Por qué |
|---|---|
| Doom 3, cualquier motor 3D moderno | GPU + driver + OpenGL/Vulkan |
| **V8 / cualquier JIT de JS** | páginas W+X y mapear código en ejecución — no existe en el modelo de capabilities |
| LibreOffice, Blender, Qt | millones de líneas + POSIX completo + hilos + GPU |

## ★ Nivel 4 — la pregunta de Google, contestada sin adornos

**Chromium/Blink son del orden de 30 millones de líneas** con ~200 dependencias
de terceros. Necesita JIT (V8), multiproceso con IPC y sandbox, pila de red
completa con TLS y HTTP/2, HarfBuzz + FreeType + ICU, compositor por GPU,
audio, vídeo y un POSIX de verdad.

**El frontend de C++ sería del orden del 2% del problema.** La respuesta honesta
es **no** — y no por C++: Chromium tampoco corre sobre un Linux al que le
falten esas piezas.

**Pero un navegador sí es alcanzable**: NetSurf, motor propio, ~200k líneas, en
**C**. Su bloqueante es la **red**.

---

## ★ Las palancas, ordenadas por lo que desbloquean

Si el criterio es *qué cambia más cosas por unidad de esfuerzo*, el orden **no**
es el que uno espera:

| # | Palanca | Qué desbloquea | ¿Necesita C++? |
|---|---|---|---|
| 1 | **Portar SDL** — SDL 1.2 es C, y su capa de plataforma son ~4 funciones (vídeo, entrada, audio, tiempo). **BMO ya tiene las cuatro** | ★ **cientos** de juegos y aplicaciones de golpe, sin tocarlos uno a uno | **NO** |
| 2 | **Compilación separada** | todo lo que pase de ~100k líneas. Hoy es el techo duro del sistema | **NO** |
| 3 | **libc: el asignador sobre `KIND_MEMORIA`** | DOOM, Lua, SQLite y todo lo demás | **NO** |
| 4 | **Pila de red + TLS** | navegador, servidores, actualizaciones, todo lo conectado | **NO** |
| 5 | El **C++ acotado** | ImGui, Box2D, y escribir lo grande de BMO sin ahogarse | — |

Las cuatro primeras son C y sistema. Ninguna pide clases.

## Y sobre la GPU

**No tener RDNA4 no es un problema, y el motivo es concreto**: todo el Nivel 0
y el Nivel 1 es *software rendering* al framebuffer. DOOM, Quake, ImGui y todo
lo 2D pintan píxeles a memoria — que es exactamente lo que BMO ya hace, con
doble búfer y write-combining.

La GPU sólo aparece en el **Nivel 3**, que está fuera de alcance por otras diez
razones antes que por ella. Comprar una tarjeta hoy no adelantaría nada:
adelantaría el día en que haga falta escribir un driver de RDNA4, que es el
proyecto que la usaría.

---

## Cómo se lee esta tabla dentro de un año

La misma regla que el censo de C++: **un "no" con motivo escrito se puede
discutir; uno sin motivo es un agujero.** Cuando una fila de "qué falta" se
cumpla, la app cambia de nivel. Ese es el uso del documento: no es una lista de
deseos, es un **mapa de dependencias**.

---

# ★★ LAS PIEZAS, CONTADAS

> Añadido el **2026-08-04**, a petición del dueño y **con dos correcciones
> suyas incorporadas**. La versión anterior de este documento despachaba la GPU
> con *"años"* y el JIT con *"no encaja"*. Las dos eran pereza: un "años" sin
> desglosar no es una estimación, es una forma educada de no pensar.
>
> Aquí cada hueco se parte en **piezas contables**. Una pieza es algo que se
> puede empezar el lunes y terminar sabiendo si funciona.

## Por qué contar piezas y no meses

Un mes es una sensación; una pieza se termina o no se termina. Y contar obliga
a lo que de verdad importa: **descubrir que dentro de un hueco enorme hay tres
piezas fáciles y una imposible** — que es exactamente el caso de la GPU, y no
se veía diciendo "años".

La columna que decide el orden no es el número de piezas: es **cuántas de ellas
ya están escritas**.

---

## 1 · EL ENLAZADOR — ★★ 5/5, CERRADO EL 2026-08-07

**Camino B** (funciones sintetizadas), el decidido en `forge/README.md`.

| # | Pieza | Estado |
|---|---|---|
| 1 | Tabla de funciones sintetizables: nombre → bytes | ✅ el mecanismo **ya corría en metal** con `__bmo_syscall_stub`, cableado para un único nombre |
| 2 | El codegen inyecta la función una vez y relocaliza las llamadas | ✅ `ea3429f4` — `codegen/sintetizadas.rs`. El stub es la primera entrada de la tabla a propósito: si no reprodujera el caso que ya funcionaba, no serviría |
| 3 | `malloc`/`free` de verdad sobre `KIND_MEMORIA` | ✅ `bmo-rt::heap::freelist`, 247 líneas — **"probada" es cierto desde el 2026-08-07 y no antes** (`7b2a3a73`): los 6 tests no enlazaban en el host por el `_start` de `crt0`, así que `cargo test -p bmo-rt` no ejecutaba ninguno. Ahora 6/6 |
| 4 | `printf` mínimo (`%d %s %x %c`) | ✅ `5f644aae` — las cinco conversiones a la tabla. **−8,2% de código en los seis ejemplos** (−25,2% en `holac`) |
| 5 | Cadenas: `strlen` `strcpy` `memcpy` `memset` | ✅ `bfcaf45b` — más `strcmp`, `strchr`, `strncmp`, `memcmp`. **−32,2%** en un programa que las usa |

★★ **Era el hueco más barato de todo el sistema y el que más desbloquea**, y ya
no está. Con esto **DOOM, Lua y SQLite dejan de estar bloqueados por el
lenguaje** — que no es lo mismo que estar listos: siguen bloqueados por la
compilación separada a partir de ~100k líneas (§ *Qué desbloquea qué*, fila 2).

### ⚠️ Tres cosas que este 5/5 **no** significa, y conviene leerlas

**1. El ahorro no se ve en el `.bex`.** Los seis ejemplos miden exactamente lo
mismo que antes, byte por byte. La sección de código se rellena a 4096
(`pad_to_page`, con `0xCC`) y 2 KB de ahorro caben dentro del relleno. Ese
relleno existe porque **BEF no tiene relocations** — lo dice la cabecera de
`patch_all_fixups`—, así que hasta que las tenga, todo ahorro de código por
debajo de una página es invisible en el fichero. Es la deuda que hay que pagar
para que esto se note.

**2. Lo que quedó fuera, y con criterio medido.** Enlazar cuesta ~10 bytes por
llamada; en línea, ~3 más el cuerpo. Así que **`abs` (13 bytes) se queda en
línea**: apenas pasa del coste de llamarlo. Y `malloc`/`free` no pueden entrar
todavía porque usan `fresh_label()`, que es estado del `Codegen`, y un
`Sintetizador` sólo recibe `&mut Vec<u8>`. La regla no es "todo a la tabla".

**3. `memmove` sigue roto.** Comparte el `copiar` de `memcpy`, que avanza de
principio a fin, así que con solapamiento y `dst > src` corrompe — que es
exactamente lo que `memmove` promete y `memcpy` no. **Hoy es un `memcpy` con
otro nombre.**

---

## 2 · LAS TRES OPS DE `KIND_ARCHIVO` — 3 piezas pequeñas

Baratas desde que `3.0` (reemplazar en FAT32) está hecho.

| # | Pieza | Nota |
|---|---|---|
| 1 | ~~`ARCH_OP_POSICIONAR`~~ | ★ **HECHO** (`b791ce4b`, 2026-08-07) con otro nombre: `ARCH_OP_SALTAR` = `0x07`, más `ARCH_OP_LEER_EN` = `0x06` para leer un bloque de golpe. Lo despacha `obj/archivo.rs`, y `fseek`/`fread` de `bmo/archivo.h` lo llaman. **Esta fila siguió diciendo "pendiente" con el nombre viejo**, que es la forma más cara de equivocarse en un mapa de dependencias: hace parecer bloqueado lo que ya corre |
| 2 | Modo I-O | el fichero entero ya vive en RAM: es el modo, no el modelo |
| 3 | Modo EXTEND | `CURSOR = LARGO` al abrir |

Detrás caen RRDS, ESDS y **KSDS** — o sea el índice, o sea la banca.

---

## 3 · ESTRATOS ESCRIBIR (`3.4`) — 5 piezas, y una ya está

| # | Pieza | Estado |
|---|---|---|
| 1 | Reservar bloques libres | — |
| 2 | Escribir un nodo nuevo (`encode` + `escribir_bloque`) | `escribir_bloque` ya existe |
| 3 | **Escribir un flujo**: el árbol de datos, el inverso de `descender` | la pieza gorda |
| 4 | Crear/actualizar el `:entradas` del padre | — |
| 5 | Encadenar el estrato y sellar | ★ **`sellar()` ya commitea en el Ryzen** |

---

## 4 · HILOS — 4 piezas

| # | Pieza |
|---|---|
| 1 | `TASK_OP_HILO_CREAR` en la superficie |
| 2 | Pila por hilo |
| 3 | Planificar: **el round-robin ya existe**, hay que dejarle meter hilos de un mismo proceso |
| 4 | Esperar/unir (`join`) |

**No lo pide la banca** — un batch es E/S secuencial. Lo piden Chrome y Steam.

---

## 5 · RED — 5 piezas, cuatro razonables y una enorme

| # | Pieza | Tamaño |
|---|---|---|
| 1 | Cablear el e1000: anillos TX/RX sobre las 287 líneas de esqueleto | semanas |
| 2 | **smoltcp**: ARP, IPv4, ICMP, UDP, TCP | crate `no_std` **ya hecha** — integrarla |
| 3 | `KIND_SOCKET` y sus operaciones | días |
| 4 | DNS | días |
| 5 | **TLS** | ★ proyecto propio: criptografía, certificados, máquina de estados |

Sin la 5 no hay nada conectado que sirva hoy. Con las cuatro primeras hay red
de área local, que ya es mucho para un banco con terminales.

---

## 6 · GPU + VULKAN — 6 piezas, y **la corrección del dueño era justa**

> *"No subestimes, no es años; no es meter Vulkan entero, es meter Vulkan 1.0
> hasta 1.3 con proceso — por algo se llama estrategia."*

Tiene razón y el "años" de este documento estaba mal escrito. Desglosado:

| # | Pieza | Realidad |
|---|---|---|
| 1 | Enumerar PCIe y mapear los BARs | **días** — es leer una tabla |
| 2 | ★ **Inicializar el GPU**: power, clocks, cargar el microcódigo | **AQUÍ ESTÁ EL MURO.** AMD lo documenta a medias y el firmware es un blob binario |
| 3 | Anillos de comandos (GFX ring, DMA) | semanas, y está documentado |
| 4 | Gestor de memoria de vídeo: VRAM, GTT, page tables de la GPU | ★ meses, y es un asignador de verdad |
| 5 | Compilador de shaders: SPIR-V → ISA de RDNA | ★ **es otro compilador entero** |
| 6 | La API Vulkan 1.0 encima: instance, device, queue, command buffer, pipeline, swapchain | meses, pero es fontanería sobre 1-5 |
| + | 1.1 · 1.2 · 1.3 | **incrementos**, no reescrituras — y ahí la estrategia del dueño es la correcta |

**La estimación honesta corregida**: no son "años de imposible". Son **seis
piezas de las que dos son proyectos propios** (la 4 y la 5) y **una es un muro
de documentación** (la 2). Las otras tres son trabajo normal.

Y sigue sin servir a la banca. Pero ya no está mal contado.

---

## 7 · EL JIT SIN ROMPER LAS CAPABILITIES — 3 piezas

> *"Podría crear un intermedio para que no choque con capabilities."*

★★ **La idea del dueño es correcta y mejor que la objeción original de este
documento.** Decir "W+X contradice el modelo" era mirar sólo el caso malo. Lo
que hace falta no es W+X — es **W^X con transición explícita**, y eso no
contradice las capabilities: **es exactamente cómo se expresan.**

| # | Pieza |
|---|---|
| 1 | `KIND_CODIGO`: memoria que nace **escribible y NO ejecutable** |
| 2 | `SELLAR`: la vuelve ejecutable **y revoca el derecho de escritura en el mismo acto** |
| 3 | La garantía: un proceso **nunca** tiene los dos derechos sobre la misma página a la vez |

Con eso un JIT funciona —escribe, sella, ejecuta, y para regenerar pide otra—
y el sistema **nunca** ha entregado una página W+X. Es lo que hacen macOS
(`MAP_JIT`) y OpenBSD (`W^X`), y encaja aquí mejor que en ellos porque aquí el
derecho es un objeto y no un bit de una `mmap`.

**No sirve a la banca ni desbloquea Chrome por sí solo.** Pero es una pieza de
diseño que estaba mal descartada, y descartarla mal habría cerrado una puerta
por un motivo falso.

---

## ★ EL ORDEN, por lo que cuesta terminarlo

Ordenado por **piezas que faltan de verdad**, no por piezas totales:

| Hueco | Piezas | Ya escritas | **Faltan** | ¿Sirve al banco? |
|---|---|---|---|---|
| **Las 3 ops de `KIND_ARCHIVO`** | 3 | 0 | **3 pequeñas** | ★★★ |
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

A partir del cuarto se separan, y entonces sí hay que elegir — pero para
entonces el sistema tendrá enlazador, libc e índice, que es otro sistema.
