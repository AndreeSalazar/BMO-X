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

## Nivel 3 — subsistemas enteros (años, y no por C++)

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
