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
