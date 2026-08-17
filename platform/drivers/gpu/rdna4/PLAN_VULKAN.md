# PLAN VULKAN -- el camino largo, escrito para no reconstruirlo

> Escrito el **2026-08-04**. Objetivo declarado del dueno:
> **RX 9060 XT 16 GB (RDNA 4) + Vulkan, para correr juegos en BMO-X.**
>
> Este documento **no reemplaza** el plan de la cabecera de `src/lib.rs`. Lo
> complementa, y lo primero que hace es separarlos -- porque son dos metas
> distintas y confundirlas es la forma clasica de no terminar ninguna.

---

# ★ LAS DOS METAS, que no son la misma

| | **Meta A -- acelerar el compositor** | **Meta B -- correr juegos de Vulkan** |
|---|---|---|
| Donde esta escrita | `src/lib.rs`, cabecera | **este documento** |
| Que hace falta de la GPU | **un motor**: SDMA (copia de rectangulos) | **todo**: 3D, sombreadores, memoria, sincronizacion |
| Toca el display (DCN)? | **no** -- el firmware UEFI ya lo dejo programado | no, si se sigue usando el framebuffer del GOP |
| Compilador de sombreadores? | **no** | ★ **si, y es un proyecto propio** |
| Tamano | como el driver de AHCI | como el propio BMO-X |
| Sirve a la banca? | ◐ un poco: el escritorio va mas suelto | ✗ nada |

**La meta A es alcanzable y esta bien planificada. La meta B es este documento
y es un proyecto de anos -- pero con piezas contables, que es distinto de
imposible.**

---

# Lo que Vulkan abarata, y lo que no

El dueno trajo el argumento y es correcto: **Vulkan esta mas abajo que
OpenGL**, asi que el *driver* hace menos trabajo. No rastrea estado, no adivina,
no compila sombreadores al dibujar. Eso es real y juega a favor.

Pero el ahorro cae en **una sola** de las tres partes:

| Coste | Lo abarata Vulkan? |
|---|---|
| Inicializar el hardware, memoria de video, anillos de comandos | **no** -- identico |
| **SPIR-V -> instrucciones de RDNA** | **no** -- es otro compilador entero |
| La API encima (1.0 -> 1.1 -> 1.2 -> 1.3) | ★ **si, y mucho.** Y aqui la estrategia incremental del dueno es la correcta |

## Y dos cosas que un juego hace y que no son Vulkan

1. **Un juego no pide "Vulkan 1.3": pide una LISTA DE CARACTERISTICAS.**
   Consulta `VkPhysicalDeviceFeatures` y **si le falta una, no arranca**. Los
   juegos actuales piden `descriptorIndexing`, `timelineSemaphore`,
   `dynamicRendering` -- cosas de 1.2 y 1.3. Con un 1.0 honesto arrancan los de
   hace unos anos, no los de ahora.

2. **Vulkan es ~30 % de lo que toca un juego.** Lo demas: **hilos** (Vulkan
   esta disenado para construir command buffers en varios hilos), sistema de
   ficheros para los assets, audio, entrada y a veces red. Con un Vulkan
   perfecto y sin hilos, el juego no arranca igual.

---

# ★★ RUTA B1 -- VULKAN POR SOFTWARE (la que casi nadie considera primero)

Es lo que hacen **SwiftShader** (Google) y **lavapipe** (Mesa): implementar
Vulkan **sin GPU**, rasterizando en el CPU.

Y borra de un golpe **las dos partes caras**:

| | Con GPU | Por software |
|---|---|---|
| Inicializar el hardware | ★ el muro | **no existe** |
| SPIR-V -> ISA de la GPU | ★ otro compilador | SPIR-V -> **x86-64**, que es tu propia arquitectura |
| La API | igual | igual |
| Rasterizador | lo pone el silicio | <- **hay que escribirlo** |

## Lo que hay a favor, medido

- **6 nucleos fisicos / 12 hilos a 3.70 GHz**, comprobado con `info` en metal
- El framebuffer con **doble bufer y write-combining** ya funciona
- El compositor ya tiene la costura (`Volcador`) para meter otro backend

## ★ Y la conexion que puso el dueno solo

SPIR-V -> x86-64 **es un JIT**. Y el JIT necesita paginas ejecutables, que es
exactamente la pieza que el mismo diseno:

```
KIND_CODIGO   nace escribible y NO ejecutable
SELLAR        la vuelve ejecutable Y revoca la escritura, en el mismo acto
garantia      nunca los dos derechos sobre la misma pagina a la vez
```

**Su idea de W^X no era un tema aparte: es un requisito de esta ruta.**

## Las piezas de B1

| # | Pieza | Tamano |
|---|---|---|
| 1 | Cargador de SPIR-V (parsear el bytecode) | semanas -- es un formato documentado y sencillo |
| 2 | SPIR-V -> x86-64 (interprete primero, JIT despues) | ★ meses |
| 3 | `KIND_CODIGO` + `SELLAR` en el kernel | **3 piezas pequenas**, ya disenadas |
| 4 | Rasterizador: triangulos, z-buffer, texturas, recorte | ★ meses |
| 5 | La API de Vulkan 1.0: instance, device, queue, command buffer, pipeline, swapchain | meses de fontaneria |
| 6 | Hilos (`clone` no; los de BMO) | 3 piezas, ver `QUE_DESBLOQUEA` |

**Veredicto de B1**: seis piezas, dos de ellas grandes, **ninguna es un muro**.
Todo esta documentado y no depende de que AMD publique nada. Y da algo que se
ve moverse en pantalla mucho antes que B2.

> **Un driver de RDNA4 es para que los juegos vayan RAPIDO. Un Vulkan por
> software es para que los juegos VAYAN.** Y "que vayan" es lo primero.

---

# RUTA B2 -- VULKAN SOBRE LA RX 9060 XT

La de verdad. Se anota entera para que el dia que se tome, se tome con los
motivos delante.

## Las piezas

| # | Pieza | Estado del conocimiento |
|---|---|---|
| 1 | Enumerar PCIe y mapear los BAR | ★ **ya se hace** en BMO (xHCI, AHCI) |
| 2 | **Cargar el firmware por el PSP** | ⚠ el muro -- ver abajo |
| 3 | Anillos de comandos (GFX, compute, SDMA) + timbres | ★ **la forma es la de xHCI**, ya peleada en metal |
| 4 | Gestor de memoria de video: VRAM, GTT, tablas de pagina de la GPU | proyecto propio, documentado en `amdgpu` |
| 5 | SPIR-V -> ISA de RDNA | ★ proyecto propio. **La ISA de RDNA si esta publicada** por AMD |
| 6 | La API de Vulkan encima | se reutiliza entera de B1 |

## ⚠ El muro real: el PSP y el firmware

En las GPU de AMD modernas hay un **Platform Security Processor** que
**autentica el microcodigo antes de que la GPU funcione**. No es un `memcpy` a
un registro: hay una secuencia de arranque, y es la parte peor documentada
publicamente.

**Lo que se sabe con certeza:**
- Los blobs de firmware de AMD **estan publicados en `linux-firmware` y son
  redistribuibles**. Esa es la razon de elegir AMD y no Nvidia, y sigue en pie.
- `amdgpu` (el driver abierto de Linux) **hace todo esto y se puede leer**.

**Lo que hay que averiguar antes de prometer nada** (ley: se pregunta, no se
supone):
- La secuencia exacta del PSP para **Navi 4x concretamente**
- Si la RX 9060 XT tiene sus cabeceras de registros publicadas
- Cuanto de `amdgpu` hay que replicar para llegar al primer anillo vivo

**No escribas un plan de fechas sobre esto hasta haberlo mirado.** Es
exactamente el tipo de cosa que parece de dos semanas y son seis meses.

## Y la nota que ya estaba escrita, que sigue valiendo

De `src/lib.rs`: *si el firmware o los registros de la SKU concreta no
estuvieran publicados, las alternativas son RDNA 3 (Navi 33, RX 7600) o RDNA 2
(Navi 23, RX 6600), con mas anos de rodaje.* Para B2 eso importa **mas** que
para la meta A, porque aqui si se toca el 3D.

---

# ★ EL ORDEN, y el disparador

Esto **no es lo siguiente** y no debe serlo. El orden con motivo:

1. **La meta A primero** (SDMA para el compositor) -- pequena, y ensena el
   camino de anillos y firmware sin jugarse el 3D
2. **`KIND_CODIGO` + `SELLAR`** -- 3 piezas, sirven al JIT y son de diseno puro
3. **Hilos** -- B1 y B2 los necesitan los dos
4. **B1: Vulkan por software** -- algo que se ve moverse
5. **B2** -- solo si B1 demostro que la API y el rasterizador funcionan

## El disparador honesto

> **Nada de esto empieza hasta que BMO-X tenga enlazador, libc e indice.**

Porque un Vulkan sin `malloc` de verdad no se puede ni escribir, y porque un
sistema que todavia no sabe correr un banco no deberia estar escribiendo un
compilador de sombreadores.

## Y la medida que decide si vale la pena

`perf` en la caja de Ejecutar dice **KiB por fotograma y peor caso**. La caja
de sucio ya recorta casi todo el volcado.

**La respuesta puede perfectamente ser que la GPU no compre nada** para lo que
BMO-X hace hoy. Ese numero se mira antes de gastar un euro y se vuelve a mirar
despues para saber si sirvio. Es la regla 4 del plan original y aqui vale
igual.

---

# ★★ POR QUE AMD NO TE LIMITA -- y que si lo hace

> Ampliacion del 2026-08-04. El dueno lo senalo y tiene razon: *"AMD no
> limita"*. Conviene escribir POR QUE, porque el motivo cambia el plan.

## Lo que AMD publica, y no es poco

| Que | Donde | Para que existe |
|---|---|---|
| **El juego de instrucciones (ISA)** de cada arquitectura RDNA | PDF publico de AMD | para que cualquiera escriba un compilador de sombreadores |
| **Las cabeceras de registros** del chip | dentro del codigo de `amdgpu`, miles de ficheros | para programar el silicio |
| **El firmware** (microcodigo) | `linux-firmware`, con licencia de **redistribucion** | para que una distro pueda incluirlo |
| **Un driver de kernel entero y abierto** | `amdgpu` | referencia funcionando |
| **Un Vulkan entero y abierto** | Mesa **RADV** | referencia funcionando de la capa de arriba |

## Por que lo hacen -- los motivos reales

No es filantropia, y conviene entenderlo porque explica **que seguira abierto**:

1. **Venden silicio, no drivers.** Nvidia monetiza CUDA y su pila cerrada; AMD
   compite por precio y volumen. Un driver abierto no les quita ingresos.
2. **Las consolas ya obligan a documentar.** PlayStation y Xbox llevan
   arquitectura AMD, y esos fabricantes reciben documentacion completa. Lo que
   ya esta escrito para un tercero cuesta poco publicar.
3. **HPC y empresa exigen auditar.** Un laboratorio que compra mil tarjetas
   quiere poder leer lo que corre en ellas. Ahi lo cerrado es una desventaja
   comercial.
4. **Valve y Steam Deck.** RADV mejoro enormemente porque Valve pago ingenieros
   para ello. AMD se beneficio sin invertir.

★ **Consecuencia practica**: lo que esta abierto lo esta por motivos
estructurales, no por una campana que pueda revertirse el ano que viene. **Es
apostable.**

## Y entonces, que limita de verdad?

> **No es el permiso. Es el VOLUMEN.**

`amdgpu` son cientos de miles de lineas. Las cabeceras de registros son miles
de ficheros. RADV es otro proyecto grande. **Todo esta ahi y nadie te lo
impide** -- pero leerlo y destilar lo que hace falta es el trabajo, y es un
trabajo de **lectura**, no de ingenieria inversa.

Esa es una diferencia enorme respecto a Nvidia, donde el trabajo **si** era
ingenieria inversa a ciegas. Y es la razon, ya escrita en `lib.rs`, para elegir
AMD -- que sigue en pie y ahora con sus motivos.

**La unica parte que sigue siendo opaca** es la secuencia del **PSP** --el
procesador que autentica el microcodigo--. No porque AMD la prohiba, sino porque
esta descrita en codigo y no en prosa. Se puede leer en `amdgpu`; lo que no hay
es un documento que la explique.

---

# ★ QUE JUEGOS ABRE CADA NIVEL DE VULKAN

⚠ **Con una advertencia primero, que es la que de verdad manda**: un juego
**no comprueba el numero de version**. Comprueba una **lista de extensiones y
caracteristicas**, y si le falta una se niega a arrancar. La version es un
resumen, no el contrato.

Dicho eso, cada nivel corresponde grosso modo a una epoca:

| Nivel | Ano | Que trajo | Que epoca abre |
|---|---|---|---|
| **1.0** | 2016 | lo basico: pipelines, render passes, descriptor sets | los primeros titulos con Vulkan nativo -- la generacion de **DOOM 2016**, *The Talos Principle*, *Dota 2* |
| **1.1** | 2018 | subgroups, memoria protegida, multiview | motores de 2018-2020 |
| **1.2** | 2020 | ★ **timeline semaphores**, **descriptor indexing**, buffer device address | **aqui empieza lo moderno de verdad**, y es donde las capas de traduccion actuales ponen su minimo |
| **1.3** | 2022 | ★ **dynamic rendering**, synchronization2 | motores actuales |

★ **La conclusion util**: un **1.0 honesto y completo** ya da juegos de verdad,
de una generacion entera. No es un ejercicio: es DOOM.

Y **1.2 es el escalon que mas abre**.

---

# ⚠ DXVK -- la trampa que hay que ver antes de contar con el

**DXVK** traduce Direct3D 9/10/11 -> Vulkan. Es la pieza que hace que Proton
funcione, y la idea de usarlo para juegos antiguos es buena... hasta que se mira
**que** traduce.

> **DXVK traduce la API DE GRAFICOS. No traduce el sistema operativo.**

Un juego de D3D9 es un `.exe` de Windows. Ademas de dibujar, ese ejecutable:

- abre ficheros con `CreateFileW` de `kernel32.dll`
- crea ventanas con `user32.dll`
- lee el registro, saca sonido por `dsound`, lee el raton por `dinput`
- y arranca con el cargador de PE de Windows

**DXVK no hace nada de eso.** DXVK **presupone que hay un Windows debajo** --
real, o Wine.

La cadena completa es:

```
juego .exe  ->  Wine (TODO el sistema)  ->  DXVK (solo graficos)  ->  Vulkan
                ^
                aqui estan los 25 anos y los millones de lineas
```

**DXVK sin Wine no arranca ni un juego.** Y Wine es exactamente la frontera que
este proyecto decidio no cruzar -- ver `docs/identidad/ENTRAR_EN_SU_ECOSISTEMA.md`.

## ★ Y el camino que SI lleva a los juegos antiguos

No es traducir Windows: es que **muchos clasicos tienen motor abierto y
reescrito**, en C o C++ portable, y casi todos sobre **SDL**:

| Motor abierto | Juego original |
|---|---|
| **GZDoom**, Chocolate Doom | Doom, Heretic, Hexen |
| **ioquake3** | Quake III |
| **OpenMW** | Morrowind |
| **devilutionX** | Diablo |
| **OpenRCT2** | RollerCoaster Tycoon 2 |
| **ScummVM** | cientos de aventuras graficas |
| **DOSBox** | el catalogo entero de MS-DOS |

Todos son **codigo fuente que se compila**, no binarios de Windows que
traducir. Y todos hablan por SDL -- la palanca no 1 de
`docs/identidad/QUE_DESBLOQUEA.md`, cuya capa de plataforma son **cuatro funciones** y de
las que BMO ya tiene tres.

> **Para juegos antiguos el camino no es DXVK: es SDL + motores abiertos.**
> Y ese no necesita Vulkan, ni GPU, ni Wine -- necesita el enlazador y la libc.

## La comparacion que ordena las cuatro ideas

| Camino | Que hace falta | Que da |
|---|---|---|
| **SDL + motores abiertos** | enlazador, libc, SDL | ★ Doom, Quake, Morrowind, ScummVM, DOSBox -- **sin GPU** |
| **Vulkan por software** | + JIT, rasterizador, hilos | juegos con Vulkan nativo, lentos pero corriendo |
| **Vulkan sobre RDNA4** | + el driver entero | los mismos, rapidos |
| **DXVK** | + **Wine entero** | ✗ descartado: cuesta el proyecto |

★★ **Y fijate en la primera fila: la que mas juegos da es la que menos cuesta,
y no necesita nada de esta carpeta.**

Eso no invalida el plan de Vulkan -- **lo coloca**. Vulkan es para juegos que
**solo** existen en Vulkan. Para todo lo demas, el camino corto pasa por el
enlazador, que es el mismo que pide el banco.

---

# ★★ BSF -- BMO Shader Format

> Idea del dueno, 2026-08-04: *"el BSF es el encabezado para la GPU, seria como
> tener dos encabezados: CPU y GPU"*.
>
> **La idea es buena y aqui queda escrita con su limite** -- porque tiene una
> mitad que vale mucho y otra que costaria el proyecto.

## La simetria, que es correcta

```
  BEF   ->  el sobre de lo que corre en la CPU
  BSF   ->  el sobre de lo que corre en la GPU
```

Dos formatos, dos procesadores, un mismo criterio: **BMO no ejecuta nada que no
haya podido mirar antes**.

## ⚠ La mitad que NO se hace: inventar un idioma

El BEF existe por un motivo concreto: el kernel carga programas y **ningun
formato existente encajaba con el modelo de capabilities**. Habia una razon.

**Con los sombreadores no la hay.** SPIR-V no presupone nada de un sistema
operativo -- es matematicas y registros. Y sobre todo:

> **Todo el mundo emite SPIR-V.** glslang, DXC, Naga, rust-gpu, los motores de
> juego. Un BSF que fuera un lenguaje NUEVO no lo produciria nada en el mundo,
> y habria que escribir el compilador desde cada lenguaje.

Seria inventarse un idioma para no aprender uno que ya habla todo el mundo, que
es libre, y que ademas esta bien disenado.

## ★ La mitad que SI: **el sobre**

Igual que el BEF **no reinventa las instrucciones de x86-64** --es un contenedor
para ellas--, el BSF **no reinventa SPIR-V: lo envuelve**.

### Que guardaria, y por que cada campo se gana su sitio

| Campo | Para que |
|---|---|
| Cuantos modulos, y de que **etapa** (vertice, fragmento, computo) | saber que hay **sin parsear SPIR-V entero** |
| El **punto de entrada** de cada modulo | idem |
| ★ **Que caracteristicas de Vulkan asume** | **rechazarlo ANTES de cargarlo.** Un juego que pide `descriptorIndexing` se entera aqui, no a mitad |
| El **BLAKE3** de cada modulo | firma, el mismo criterio que el resto del sistema |
| ★ **Codigo YA COMPILADO** para un objetivo + el SPIR-V de reserva | no recompilar en cada arranque |

### La fila que mas vale es la ultima

Compilar SPIR-V a codigo maquina **al cargar** es lento. Es la razon de esos
*"compilando sombreadores... 3 min"* de los juegos modernos.

Un BSF que lleve **el resultado ya hecho y su firma** se lo ahorra. Y encaja
con el ethos entero: **si esta firmado y cuadra, no hay que rehacerlo.**

## Donde encaja -- el hueco ya estaba

En `SectionKind` hay **`Shaders = 0x0A`** y en `BefFlags` hay `HAS_SHADERS`,
reservados desde hace tiempo. **El BSF es exactamente lo que va ahi dentro.**

```
un .bex
 +-- Code        <- x86-64
 +-- RoData
 +-- Shaders     <- UN BSF
      +-- cabecera BSF
      +-- modulo 0: vertice   - SPIR-V + BLAKE3
      +-- modulo 1: fragmento - SPIR-V + BLAKE3
      +-- (opcional) lo mismo YA COMPILADO al objetivo
```

Y por la regla que ya esta escrita --*una seccion desconocida se salta*-- el
kernel **ni se entera de que existe**. Cero coste en Ring 0.

---

# ★ EL POTENCIAL, y la pregunta de las consolas

El dueno lo pregunto y la respuesta tiene dos mitades muy distintas.

## Lo que NO va a pasar

**Ninguna consola va a adoptar BSF.** PlayStation, Xbox y Switch tienen sus
propios formatos --PSSL, DXIL, NVN-- y sus plataformas estan cerradas por
contrato, no por tecnologia. No es una cuestion de calidad del formato.

## ★★ Lo que SI, y es mas interesante

**Las consolas ya trabajan como el BSF propone.** Todas ellas:

1. **Precompilan los sombreadores en el estudio**, no en casa del jugador
2. Los **empaquetan firmados** con el juego
3. Y **no compilan nada en tiempo de ejecucion**

Por que pueden? Porque tienen **hardware fijo y conocido**. Un PC no sabe que
GPU habra, asi que envia SPIR-V y compila al arrancar -- de ahi el tartamudeo.

> ★ **Y BMO-X esta en condiciones de consola, no de PC.**
>
> Una maquina, una GPU conocida, un sistema operativo. **El objetivo se sabe al
> compilar.**

O sea que el modelo de consola --precompilar, firmar, verificar, no recompilar
jamas-- **no es una aspiracion para BMO-X: es su situacion natural.** Y encaja
con lo que el sistema ya hace con los programas: `run` comprueba el `:firma`
antes de admitir un `.bex`.

**El potencial del BSF no es que lo usen otros. Es que le da a BMO-X la
disciplina de una consola en un sistema que ademas puede demostrarla.**

---

## Estado y disparador

| | |
|---|---|
| Se escribe ya? | **no.** No hay nada que lea sombreadores todavia |
| Cuando? | con la ruta B1 (Vulkan por software), cuando exista un consumidor |
| Tamano? | pequeno: una cabecera y una tabla. Como el BEF pero diminuto |
| Bloquea a algo? | no. Se apunta para que el dia que toque no se redisene de cero |

**La frase que lo resume**: *no inventes el idioma, inventa el sobre*. El
idioma ya lo habla todo el mundo y es gratis; el sobre es donde caben las tres
cosas que solo BMO ofrece -- **firma, requisitos declarados y precompilado
verificable**.
