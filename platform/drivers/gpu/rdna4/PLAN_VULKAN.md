# PLAN VULKAN — el camino largo, escrito para no reconstruirlo

> Escrito el **2026-08-04**. Objetivo declarado del dueño:
> **RX 9060 XT 16 GB (RDNA 4) + Vulkan, para correr juegos en BMO-X.**
>
> Este documento **no reemplaza** el plan de la cabecera de `src/lib.rs`. Lo
> complementa, y lo primero que hace es separarlos — porque son dos metas
> distintas y confundirlas es la forma clásica de no terminar ninguna.

---

# ★ LAS DOS METAS, que no son la misma

| | **Meta A — acelerar el compositor** | **Meta B — correr juegos de Vulkan** |
|---|---|---|
| Dónde está escrita | `src/lib.rs`, cabecera | **este documento** |
| Qué hace falta de la GPU | **un motor**: SDMA (copia de rectángulos) | **todo**: 3D, sombreadores, memoria, sincronización |
| ¿Toca el display (DCN)? | **no** — el firmware UEFI ya lo dejó programado | no, si se sigue usando el framebuffer del GOP |
| ¿Compilador de sombreadores? | **no** | ★ **sí, y es un proyecto propio** |
| Tamaño | como el driver de AHCI | como el propio BMO-X |
| ¿Sirve a la banca? | ◐ un poco: el escritorio va más suelto | ✗ nada |

**La meta A es alcanzable y está bien planificada. La meta B es este documento
y es un proyecto de años — pero con piezas contables, que es distinto de
imposible.**

---

# Lo que Vulkan abarata, y lo que no

El dueño trajo el argumento y es correcto: **Vulkan está más abajo que
OpenGL**, así que el *driver* hace menos trabajo. No rastrea estado, no adivina,
no compila sombreadores al dibujar. Eso es real y juega a favor.

Pero el ahorro cae en **una sola** de las tres partes:

| Coste | ¿Lo abarata Vulkan? |
|---|---|
| Inicializar el hardware, memoria de vídeo, anillos de comandos | **no** — idéntico |
| **SPIR-V → instrucciones de RDNA** | **no** — es otro compilador entero |
| La API encima (1.0 → 1.1 → 1.2 → 1.3) | ★ **sí, y mucho.** Y aquí la estrategia incremental del dueño es la correcta |

## Y dos cosas que un juego hace y que no son Vulkan

1. **Un juego no pide "Vulkan 1.3": pide una LISTA DE CARACTERÍSTICAS.**
   Consulta `VkPhysicalDeviceFeatures` y **si le falta una, no arranca**. Los
   juegos actuales piden `descriptorIndexing`, `timelineSemaphore`,
   `dynamicRendering` — cosas de 1.2 y 1.3. Con un 1.0 honesto arrancan los de
   hace unos años, no los de ahora.

2. **Vulkan es ~30 % de lo que toca un juego.** Lo demás: **hilos** (Vulkan
   está diseñado para construir command buffers en varios hilos), sistema de
   ficheros para los assets, audio, entrada y a veces red. Con un Vulkan
   perfecto y sin hilos, el juego no arranca igual.

---

# ★★ RUTA B1 — VULKAN POR SOFTWARE (la que casi nadie considera primero)

Es lo que hacen **SwiftShader** (Google) y **lavapipe** (Mesa): implementar
Vulkan **sin GPU**, rasterizando en el CPU.

Y borra de un golpe **las dos partes caras**:

| | Con GPU | Por software |
|---|---|---|
| Inicializar el hardware | ★ el muro | **no existe** |
| SPIR-V → ISA de la GPU | ★ otro compilador | SPIR-V → **x86-64**, que es tu propia arquitectura |
| La API | igual | igual |
| Rasterizador | lo pone el silicio | ← **hay que escribirlo** |

## Lo que hay a favor, medido

- **6 núcleos físicos / 12 hilos a 3.70 GHz**, comprobado con `info` en metal
- El framebuffer con **doble búfer y write-combining** ya funciona
- El compositor ya tiene la costura (`Volcador`) para meter otro backend

## ★ Y la conexión que puso el dueño solo

SPIR-V → x86-64 **es un JIT**. Y el JIT necesita páginas ejecutables, que es
exactamente la pieza que él mismo diseñó:

```
KIND_CODIGO   nace escribible y NO ejecutable
SELLAR        la vuelve ejecutable Y revoca la escritura, en el mismo acto
garantía      nunca los dos derechos sobre la misma página a la vez
```

**Su idea de W^X no era un tema aparte: es un requisito de esta ruta.**

## Las piezas de B1

| # | Pieza | Tamaño |
|---|---|---|
| 1 | Cargador de SPIR-V (parsear el bytecode) | semanas — es un formato documentado y sencillo |
| 2 | SPIR-V → x86-64 (intérprete primero, JIT después) | ★ meses |
| 3 | `KIND_CODIGO` + `SELLAR` en el kernel | **3 piezas pequeñas**, ya diseñadas |
| 4 | Rasterizador: triángulos, z-buffer, texturas, recorte | ★ meses |
| 5 | La API de Vulkan 1.0: instance, device, queue, command buffer, pipeline, swapchain | meses de fontanería |
| 6 | Hilos (`clone` no; los de BMO) | 3 piezas, ver `QUE_DESBLOQUEA` |

**Veredicto de B1**: seis piezas, dos de ellas grandes, **ninguna es un muro**.
Todo está documentado y no depende de que AMD publique nada. Y da algo que se
ve moverse en pantalla mucho antes que B2.

> **Un driver de RDNA4 es para que los juegos vayan RÁPIDO. Un Vulkan por
> software es para que los juegos VAYAN.** Y "que vayan" es lo primero.

---

# RUTA B2 — VULKAN SOBRE LA RX 9060 XT

La de verdad. Se anota entera para que el día que se tome, se tome con los
motivos delante.

## Las piezas

| # | Pieza | Estado del conocimiento |
|---|---|---|
| 1 | Enumerar PCIe y mapear los BAR | ★ **ya se hace** en BMO (xHCI, AHCI) |
| 2 | **Cargar el firmware por el PSP** | ⚠️ el muro — ver abajo |
| 3 | Anillos de comandos (GFX, compute, SDMA) + timbres | ★ **la forma es la de xHCI**, ya peleada en metal |
| 4 | Gestor de memoria de vídeo: VRAM, GTT, tablas de página de la GPU | proyecto propio, documentado en `amdgpu` |
| 5 | SPIR-V → ISA de RDNA | ★ proyecto propio. **La ISA de RDNA sí está publicada** por AMD |
| 6 | La API de Vulkan encima | se reutiliza entera de B1 |

## ⚠️ El muro real: el PSP y el firmware

En las GPU de AMD modernas hay un **Platform Security Processor** que
**autentica el microcódigo antes de que la GPU funcione**. No es un `memcpy` a
un registro: hay una secuencia de arranque, y es la parte peor documentada
públicamente.

**Lo que se sabe con certeza:**
- Los blobs de firmware de AMD **están publicados en `linux-firmware` y son
  redistribuibles**. Ésa es la razón de elegir AMD y no Nvidia, y sigue en pie.
- `amdgpu` (el driver abierto de Linux) **hace todo esto y se puede leer**.

**Lo que hay que averiguar antes de prometer nada** (ley: se pregunta, no se
supone):
- La secuencia exacta del PSP para **Navi 4x concretamente**
- Si la RX 9060 XT tiene sus cabeceras de registros publicadas
- Cuánto de `amdgpu` hay que replicar para llegar al primer anillo vivo

**No escribas un plan de fechas sobre esto hasta haberlo mirado.** Es
exactamente el tipo de cosa que parece de dos semanas y son seis meses.

## Y la nota que ya estaba escrita, que sigue valiendo

De `src/lib.rs`: *si el firmware o los registros de la SKU concreta no
estuvieran publicados, las alternativas son RDNA 3 (Navi 33, RX 7600) o RDNA 2
(Navi 23, RX 6600), con más años de rodaje.* Para B2 eso importa **más** que
para la meta A, porque aquí sí se toca el 3D.

---

# ★ EL ORDEN, y el disparador

Esto **no es lo siguiente** y no debe serlo. El orden con motivo:

1. **La meta A primero** (SDMA para el compositor) — pequeña, y enseña el
   camino de anillos y firmware sin jugarse el 3D
2. **`KIND_CODIGO` + `SELLAR`** — 3 piezas, sirven al JIT y son de diseño puro
3. **Hilos** — B1 y B2 los necesitan los dos
4. **B1: Vulkan por software** — algo que se ve moverse
5. **B2** — sólo si B1 demostró que la API y el rasterizador funcionan

## El disparador honesto

> **Nada de esto empieza hasta que BMO-X tenga enlazador, libc e índice.**

Porque un Vulkan sin `malloc` de verdad no se puede ni escribir, y porque un
sistema que todavía no sabe correr un banco no debería estar escribiendo un
compilador de sombreadores.

## Y la medida que decide si vale la pena

`perf` en la caja de Ejecutar dice **KiB por fotograma y peor caso**. La caja
de sucio ya recorta casi todo el volcado.

**La respuesta puede perfectamente ser que la GPU no compre nada** para lo que
BMO-X hace hoy. Ese número se mira antes de gastar un euro y se vuelve a mirar
después para saber si sirvió. Es la regla 4 del plan original y aquí vale
igual.

---

# ★★ POR QUÉ AMD NO TE LIMITA — y qué sí lo hace

> Ampliación del 2026-08-04. El dueño lo señaló y tiene razón: *"AMD no
> limita"*. Conviene escribir POR QUÉ, porque el motivo cambia el plan.

## Lo que AMD publica, y no es poco

| Qué | Dónde | Para qué existe |
|---|---|---|
| **El juego de instrucciones (ISA)** de cada arquitectura RDNA | PDF público de AMD | para que cualquiera escriba un compilador de sombreadores |
| **Las cabeceras de registros** del chip | dentro del código de `amdgpu`, miles de ficheros | para programar el silicio |
| **El firmware** (microcódigo) | `linux-firmware`, con licencia de **redistribución** | para que una distro pueda incluirlo |
| **Un driver de kernel entero y abierto** | `amdgpu` | referencia funcionando |
| **Un Vulkan entero y abierto** | Mesa **RADV** | referencia funcionando de la capa de arriba |

## Por qué lo hacen — los motivos reales

No es filantropía, y conviene entenderlo porque explica **qué seguirá abierto**:

1. **Venden silicio, no drivers.** Nvidia monetiza CUDA y su pila cerrada; AMD
   compite por precio y volumen. Un driver abierto no les quita ingresos.
2. **Las consolas ya obligan a documentar.** PlayStation y Xbox llevan
   arquitectura AMD, y esos fabricantes reciben documentación completa. Lo que
   ya está escrito para un tercero cuesta poco publicar.
3. **HPC y empresa exigen auditar.** Un laboratorio que compra mil tarjetas
   quiere poder leer lo que corre en ellas. Ahí lo cerrado es una desventaja
   comercial.
4. **Valve y Steam Deck.** RADV mejoró enormemente porque Valve pagó ingenieros
   para ello. AMD se benefició sin invertir.

★ **Consecuencia práctica**: lo que está abierto lo está por motivos
estructurales, no por una campaña que pueda revertirse el año que viene. **Es
apostable.**

## Y entonces, ¿qué limita de verdad?

> **No es el permiso. Es el VOLUMEN.**

`amdgpu` son cientos de miles de líneas. Las cabeceras de registros son miles
de ficheros. RADV es otro proyecto grande. **Todo está ahí y nadie te lo
impide** — pero leerlo y destilar lo que hace falta es el trabajo, y es un
trabajo de **lectura**, no de ingeniería inversa.

Ésa es una diferencia enorme respecto a Nvidia, donde el trabajo **sí** era
ingeniería inversa a ciegas. Y es la razón, ya escrita en `lib.rs`, para elegir
AMD — que sigue en pie y ahora con sus motivos.

**La única parte que sigue siendo opaca** es la secuencia del **PSP** —el
procesador que autentica el microcódigo—. No porque AMD la prohíba, sino porque
está descrita en código y no en prosa. Se puede leer en `amdgpu`; lo que no hay
es un documento que la explique.

---

# ★ QUÉ JUEGOS ABRE CADA NIVEL DE VULKAN

⚠️ **Con una advertencia primero, que es la que de verdad manda**: un juego
**no comprueba el número de versión**. Comprueba una **lista de extensiones y
características**, y si le falta una se niega a arrancar. La versión es un
resumen, no el contrato.

Dicho eso, cada nivel corresponde grosso modo a una época:

| Nivel | Año | Qué trajo | Qué época abre |
|---|---|---|---|
| **1.0** | 2016 | lo básico: pipelines, render passes, descriptor sets | los primeros títulos con Vulkan nativo — la generación de **DOOM 2016**, *The Talos Principle*, *Dota 2* |
| **1.1** | 2018 | subgroups, memoria protegida, multiview | motores de 2018-2020 |
| **1.2** | 2020 | ★ **timeline semaphores**, **descriptor indexing**, buffer device address | **aquí empieza lo moderno de verdad**, y es donde las capas de traducción actuales ponen su mínimo |
| **1.3** | 2022 | ★ **dynamic rendering**, synchronization2 | motores actuales |

★ **La conclusión útil**: un **1.0 honesto y completo** ya da juegos de verdad,
de una generación entera. No es un ejercicio: es DOOM.

Y **1.2 es el escalón que más abre**.

---

# ⚠️ DXVK — la trampa que hay que ver antes de contar con él

**DXVK** traduce Direct3D 9/10/11 → Vulkan. Es la pieza que hace que Proton
funcione, y la idea de usarlo para juegos antiguos es buena… hasta que se mira
**qué** traduce.

> **DXVK traduce la API DE GRÁFICOS. No traduce el sistema operativo.**

Un juego de D3D9 es un `.exe` de Windows. Además de dibujar, ese ejecutable:

- abre ficheros con `CreateFileW` de `kernel32.dll`
- crea ventanas con `user32.dll`
- lee el registro, saca sonido por `dsound`, lee el ratón por `dinput`
- y arranca con el cargador de PE de Windows

**DXVK no hace nada de eso.** DXVK **presupone que hay un Windows debajo** —
real, o Wine.

La cadena completa es:

```
juego .exe  →  Wine (TODO el sistema)  →  DXVK (sólo gráficos)  →  Vulkan
                ↑
                aquí están los 25 años y los millones de líneas
```

**DXVK sin Wine no arranca ni un juego.** Y Wine es exactamente la frontera que
este proyecto decidió no cruzar — ver `docs/ENTRAR_EN_SU_ECOSISTEMA.md`.

## ★ Y el camino que SÍ lleva a los juegos antiguos

No es traducir Windows: es que **muchos clásicos tienen motor abierto y
reescrito**, en C o C++ portable, y casi todos sobre **SDL**:

| Motor abierto | Juego original |
|---|---|
| **GZDoom**, Chocolate Doom | Doom, Heretic, Hexen |
| **ioquake3** | Quake III |
| **OpenMW** | Morrowind |
| **devilutionX** | Diablo |
| **OpenRCT2** | RollerCoaster Tycoon 2 |
| **ScummVM** | cientos de aventuras gráficas |
| **DOSBox** | el catálogo entero de MS-DOS |

Todos son **código fuente que se compila**, no binarios de Windows que
traducir. Y todos hablan por SDL — la palanca nº 1 de
`docs/QUE_DESBLOQUEA.md`, cuya capa de plataforma son **cuatro funciones** y de
las que BMO ya tiene tres.

> **Para juegos antiguos el camino no es DXVK: es SDL + motores abiertos.**
> Y ése no necesita Vulkan, ni GPU, ni Wine — necesita el enlazador y la libc.

## La comparación que ordena las cuatro ideas

| Camino | Qué hace falta | Qué da |
|---|---|---|
| **SDL + motores abiertos** | enlazador, libc, SDL | ★ Doom, Quake, Morrowind, ScummVM, DOSBox — **sin GPU** |
| **Vulkan por software** | + JIT, rasterizador, hilos | juegos con Vulkan nativo, lentos pero corriendo |
| **Vulkan sobre RDNA4** | + el driver entero | los mismos, rápidos |
| **DXVK** | + **Wine entero** | ✗ descartado: cuesta el proyecto |

★★ **Y fíjate en la primera fila: la que más juegos da es la que menos cuesta,
y no necesita nada de esta carpeta.**

Eso no invalida el plan de Vulkan — **lo coloca**. Vulkan es para juegos que
**sólo** existen en Vulkan. Para todo lo demás, el camino corto pasa por el
enlazador, que es el mismo que pide el banco.
