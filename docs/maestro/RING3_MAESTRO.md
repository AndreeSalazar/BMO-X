# RING 3 MAESTRO -- el censo de lo que corre con privilegio, y que baja

> Escrito el **2026-08-26**, a peticion del dueno y con sus palabras:
>
> *"analizar TODOS los componentes x86-64, si la logica compromete algo TODOS
> tienen que estar en Ring 3, porque en Ring 0 no quiero sorpresa."*
>
> Mismo metodo que [`SMP_MAESTRO.md`](SMP_MAESTRO.md) y
> [`RED_MAESTRO.md`](RED_MAESTRO.md): **medir primero, y decir el numero
> incomodo antes de proponer nada.**

---

# 0. EL RESUMEN, PARA NO TENER QUE LEERLO ENTERO

```text
   lo que corre en Ring 0 hoy            ~69.300 lineas
   de eso, INSTRUCCIONES PRIVILEGIADAS   ~14.700   se queda, y no es opinable
   de eso, INTERPRETA BYTES AJENOS       ~18.100   ESTA es la lista del dueno
   de eso, PRESENTACION                  ~11.400   no compromete nada, y sobra
   el resto (pegamento, objetos, IPC)    ~25.100
```

* * **Y el dato que cambia el veredicto de casi toda la lista B**: de esas
18.100 lineas que interpretan bytes de un desconocido, **13.400 no tienen ni una
linea `unsafe`**. Un fallo ahi da un valor equivocado o un `panic`. No corrompe
memoria. **No es la clase de sorpresa que el dueno teme.**

Lo que si la es cabe en una lista corta, y esta en la seccion 4.

---

# 1. LA DISTINCION QUE HAY QUE HACER ANTES DE CONTAR NADA

**Un crate aparte NO es Ring 3.**

Este arbol tiene 23 crates de `platform/` que el kernel enlaza. Estan fuera de
`Ultra_kernel_x86-64/`, tienen sus propias pruebas, se compilan en el anfitrion
-- y **ejecutan con CPL=0 igual que el resto**. `bmo-fat32` no esta "fuera del
kernel": esta fuera de la *carpeta* del kernel.

```text
   MODULAR   separa QUIEN ESCRIBIO cada cosa   -> ya esta hecho
   RING 3    separa QUE PASA SI SE EQUIVOCA    -> es otra pregunta
```

Lo primero ya se pago y vale mucho: es lo que permite probar el parser de FAT32
en el anfitrion con `cargo test`. Pero **no compra ni un gramo de aislamiento**,
y confundir las dos cosas es la unica forma de creerse protegido sin estarlo.

---

# 2. EL CENSO, MEDIDO EL 2026-08-26

## 2.1 -- El kernel

| carpeta | lineas | que es |
|---|---|---|
| `core/` | 9.133 | arranque, shell, splash, autopsia, informes |
| `dev/` | 7.305 | USB, disco, PCI, red, teclado, framebuffer |
| `obj/` | 4.642 | los objetos con capability: fichero, memoria, audio... |
| `task/` | 4.204 | planificador, admision de `.bex`, lanzamiento |
| `plat/` | 4.028 | IDT, traps, timer, APIC, MADT, spinlocks |
| `syscall/` | 4.025 | la puerta, y el reparto de las operaciones |
| `fsys/` | 3.475 | FAT32 y ESTRATOS, la parte que toca el disco |
| `cpu_vendor/` | 2.499 | lo que el silicio confiesa de si mismo |
| `cabina/` | 1.436 | la caja negra |
| `mm/` | 1.007 | paginacion y asignador fisico |
| `cpu/`, `svc/` | 550 | |
| **total** | **42.865** | 149 ficheros |

## 2.2 -- Los crates de `platform/` que ENLAZA el kernel

23 crates, **27.779 lineas**, de las que 1.537 son ficheros de pruebas que no
viajan al binario.

| crate | lineas | lineas con `unsafe` |
|---|---|---|
| `shared/bmo-ciudad` | 3.123 | **0** |
| `usb/uhid` | 3.036 | 20 |
| `storage/estratos` | 3.031 | **0** |
| `storage/fat32` | 2.761 | 44 |
| `usb/xhci` | 2.143 | **66** |
| `shared/bmo-dibujo` | 1.700 | 1 |
| `services/cabina-core` | 1.347 | 0 |
| `usb/input` | 1.226 | 13 |
| `storage/ahci` | 1.179 | **28** |
| `usb/uaudio` | 1.135 | **0** |
| `net` | 1.045 | 2 |
| `abi/bmo-bex-gate` | 1.029 | 3 |
| `storage/identify` | 991 | 1 |
| `shared/bmo-firmware` | 944 | **0** |
| resto (9 crates) | 2.089 | pocas |

---

# 3. LAS TRES CLASES, Y SOLO UNA ES "SORPRESA"

## Clase A -- **NO PUEDE BAJAR**, y no es una decision de diseno

```text
   mm/                  CR3, tablas de pagina, invlpg
   plat/                IDT, GDT, TSS, APIC, spinlocks, manejador de faults
   cpu/, cpu_vendor/    CPUID, MSR, XSAVE
   task/                el cambio de contexto, iretq, el TSS.RSP0
   syscall/entry        syscall/sysret
```

~14.700 lineas. Un Ring 3 que pudiera hacer esto **seria Ring 0 con otro
nombre**. Aqui no hay debate, y el documento lo dice para que la lista de abajo
no parezca arbitraria.

## Clase B -- **INTERPRETA BYTES DE UN DESCONOCIDO**  <- la lista del dueno

Es el criterio correcto, y no es "driver si / driver no". Un driver que escribe
tres registros MMIO no tiene superficie; un parser de 3.000 lineas que come lo
que le da un aparato USB cualquiera, si.

```text
   la pregunta:  quien ELIGE los bytes que entran aqui?
   si la respuesta no es "el dueno de la maquina", es clase B
```

## Clase C -- **PRESENTACION**

`core/shell/` (2.942), `core/splash/` (2.197), `bmo-ciudad` (3.123),
`core/gato/` (786), `core/report.rs` (604), `bmo-dibujo` (1.700).

**~11.400 lineas que no comprometen nada** -- no leen nada de fuera, no tocan
hardware mas alla de escribir pixeles. No son un riesgo.

Pero son un problema distinto y ya documentado: `core/shell/` son 2.942 lineas
de un shell **al que el dueno no vuelve**. Es peso muerto en la imagen que se lee
entera en cada arranque, no una amenaza.

---

# 4. LA LISTA B, ORDENADA POR LO QUE DE VERDAD ARRIESGA

* * **Y aqui esta el hallazgo que ordena la lista.** No todas las 18.100 lineas
arriesgan lo mismo, y la diferencia se mide sin opinar: **cuenta las lineas con
`unsafe`.**

> Un parser en Rust seguro que se equivoca devuelve un numero malo o entra en
> panico. **No puede escribir donde no debe.** Un parser con aritmetica de
> punteros sobre un buffer que el aparato dimensiona, si.

| # | componente | lineas | quien elige los bytes | `unsafe` | veredicto |
|---|---|---|---|---|---|
| 1 | `usb/xhci` | 2.143 | el controlador (TRBs, eventos) | **66** | el peor de la lista |
| 2 | `usb/uhid` | 3.036 | **cualquier USB que enchufes** | 20 | candidato #1 a bajar |
| 3 | `storage/ahci` | 1.179 | el disco (FIS, respuestas) | 28 | hardware, dificil de bajar |
| 4 | `storage/fat32` | 2.761 | **cualquier pendrive** | 44 | ver la nota de abajo |
| 5 | `usb/input` | 1.226 | el aparato (informes) | 13 | |
| 6 | `abi/bmo-bex-gate` | 1.029 | quien te pase un `.bex` | 3 | ya hay gate de autoria |
| 7 | `storage/estratos` | 3.031 | el propio disco | **0** | seguro |
| 8 | `usb/uaudio` | 1.135 | **cualquier USB** | **0** | seguro |
| 9 | `net` | 1.045 | **cualquiera del cable** | 2 | casi seguro |
| 10 | `shared/bmo-firmware` | 944 | la placa (ACPI) | **0** | seguro, y AML nunca |
| 11 | `storage/identify` | 991 | el disco | 1 | |
| 12 | `storage/particiones` | 318 | **cualquier disco** | **0** | seguro |

## [!] La nota de FAT32, porque el 44 asusta mas de lo que debe

Las 44 son casi todas **el mismo patron**, y es benigno:

```rust
   let entries = self.buf.as_ptr() as *const DirEntry;   // buf: [u8; 512]
   for i in 0..(512/32) { let de = unsafe { &*entries.add(i) }; }
```

16 entradas de 32 bytes en un buffer de 512, con el limite escrito a mano en el
bucle. **Es `unsafe` de estilo, no de riesgo.** Quitarlo es mecanico
(`chunks_exact(32)`) y valdria la pena, pero no es lo que se lleva la maquina.

## Y el que si asusta, y es el que menos se sospecha: `usb/xhci`

66 lineas `unsafe`, aritmetica de punteros sobre estructuras cuyo **tamano lo
decide el controlador** (`ctx_sz`, contextos de 32 o 64 bytes), anillos de TRBs
en memoria DMA, y `evt_poll_block` leyendo eventos que el silicio escribe.

*** **Este es el unico de la lista que no puede bajar a Ring 3 sin resolver
antes los tres mecanismos de la seccion 5** -- y es, a la vez, el que mas
sorpresa puede dar. Es la contradiccion central de este documento y no se
disimula.

---

# 5. LO QUE FALTA PARA QUE UN DRIVER VIVA EN RING 3

No es una cuestion de voluntad. Un driver necesita **tres cosas** que hoy Ring 3
no tiene, y conviene saber cuanto vale cada una porque **dos ya estan medio
hechas**.

## 5.1 -- MMIO por direccion fisica  ->  **el molde YA EXISTE**

`obj::fb::claim` mapea el framebuffer fisico en el espacio del que lo reclama:

```rust
   vmm::map_page_wc(aspace, FRAMEBUFFER_VA_BASE + off, fisica + off, true, true)
```

Es exactamente lo que necesita un driver, con dos diferencias: la fisica esta
**clavada** (`info::FB_ADDR`) y el tipo de cache es WC. Un `KIND_MMIO` que tome
`(fisica, bytes)` de una capability y mapee **uncacheable** es el mismo codigo
con la direccion como dato. **Dias, no meses.**

## 5.2 -- Memoria DMA con fisica conocida  ->  **el molde YA EXISTE**

`MEM_OP_OFRECER` / `TASK_OP_TOMAR` ya prestan un bloque entre procesos, y
`vmm::translate` ya da la fisica. Lo que falta es **prometer que no se mueve**
(hoy nada la mueve, asi que la promesa es escribirla) y **exponer la fisica** al
dueno del bloque.

[!] Y esto ya esta escrito como plan en `RED_MAESTRO.md` seccion 4: *"los
anillos de recepcion de la NIC se mapean en el espacio de la pila de Ring 3"*.
No es una idea nueva de este documento: es la misma, aplicada a todos.

## 5.3 -- La interrupcion  ->  **ESTO ES LO QUE NO EXISTE**

```text
   hoy:   una IRQ despierta al manejador de Ring 0 y ahi se acaba
   falta: una IRQ tiene que poder DESPERTAR A UN PROCESO
```

Es el unico de los tres que no tiene molde, y se comprobo leyendo: **ni un solo
sitio de `plat/` ni de `dev/` escribe en un endpoint de `obj/endpoint.rs`.** Los
endpoints son IPC de Ring 3 a Ring 3 y nada mas.

* * **Y aqui BMO-X tiene una ventaja que casi nadie tiene, por una decision vieja
que se tomo por otro motivo.** El sistema ya tiene `WAIT` como syscall congelado.
Un driver de Ring 3 que espera una interrupcion **no necesita una llamada
nueva**: necesita que el manejador de IRQ haga `reply_to` sobre un endpoint, y
`WAIT` ya sabe despertarse con eso.

```text
   IRQ 11 --> manejador minimo en Ring 0 (enmascarar + EOI + reply_to)
                                  |
                                  v
                       el driver de Ring 3 vuelve de WAIT
```

El manejador de Ring 0 se queda en **decenas de lineas**: enmascarar la linea,
mandar el EOI, despertar. **No sabe lo que es un TRB.** Eso es lo que baja las
2.143 lineas del xHCI a Ring 3 sin hundir la latencia.

[!] Y el numero que decide si esto es viable ya esta medido: **una puerta cuesta
969 ciclos**. A 250 latidos por segundo (el audio) son 242.000 ciclos/s de un CPU
que da miles de millones. **Para el audio y el disco no se nota. Para la red a
gigabit habria que mirarlo**, y por eso `RED_MAESTRO` no mete el syscall en el
camino del dato: el anillo se comparte.

---

# 6. EL ORDEN QUE PROPONE ESTE DOCUMENTO

No por tamano ni por miedo: **por lo que cada paso deja probado para el
siguiente.**

### Paso 1 -- `KIND_MMIO` y la fisica del bloque prestado (5.1 + 5.2)

Sin esto no baja nada. Son los dos que ya tienen molde.
**Como se prueba**: un `.bex` que mapea el registro de version del xHC y lo
imprime. Cero escrituras. Si lee lo mismo que `cabina`, esta hecho.

### Paso 2 -- La IRQ que despierta a un proceso (5.3)

**Como se prueba**: un `.bex` que hace `WAIT` sobre el timer y cuenta 250
despertares en un segundo. Sin driver, sin aparato, sin riesgo.

Estos dos pasos **no bajan ni una linea a Ring 3**. Construyen el suelo. Y esa
es justo la parte que un plan optimista se salta.

### Paso 3 -- El primero que baja: `usb/uaudio`  (1.135 lineas, 0 `unsafe`)

*** **Y se baja precisamente porque NO es el peligroso.** El parser de
descriptores de audio no toca hardware: come un buffer de bytes y devuelve
numeros. Es la prueba de que el mecanismo funciona **con la pieza mas barata de
equivocarse** -- y es la que el dueno esta mirando ahora mismo.

### Paso 4 -- `usb/uhid`  (3.036 lineas)

El candidato real. Un descriptor de informes HID lo elige **cualquier cosa que
se enchufe**, y son 3.036 lineas de gramatica.

Con un aviso que hay que decir antes: **el teclado de Ring 0 no puede depender de
un proceso de Ring 3.** Si el driver HID muere y con el el teclado, no queda
forma de arreglarlo desde la maquina. La respuesta es que el teclado de arranque
(boot protocol, 8 bytes, sin gramatica) se queda en Ring 0 y **solo el parser de
descriptores baja**.

### Paso 5 -- `net`, y ahi engancha `RED_MAESTRO` paso 2

### Paso 6 -- `fat32` y `particiones`, cuando ESTRATOS pueda arrancar solo

**No antes.** FAT32 es hoy como se lee la particion de arranque. Un fallo del
sistema de ficheros en Ring 3 mientras es el unico camino a los datos es una
maquina que no arranca.

### NUNCA en esta lista -- `xhci`, `ahci`

Bajan **al final o no bajan**, y el documento no lo promete. Son los dos que
tocan DMA de verdad: un descriptor mal construido por un driver de Ring 3 hace
que **la tarjeta** escriba donde no debe, y el anillo del que la escribe da
igual. Eso solo lo arregla una IOMMU, y esa es otra conversacion.

---

# 7. LO QUE ESTE DOCUMENTO SE NIEGA A PROMETER

- **"Todo en Ring 3" no es alcanzable ni deseable.** La clase A son 14.700
  lineas que definen lo que es un kernel. La pregunta buena no es *cuanto baja*
  sino *que queda arriba que interpreta lo que le dan.*
- **Bajar un driver sin IOMMU no cierra el DMA.** Se puede decir que un parser
  ya no corrompe el kernel; no se puede decir que una tarjeta no escriba donde no
  debe.
- **Ninguna de las 18.100 lineas ha causado todavia un fallo conocido.** El unico
  `#GP` de Ring 0 que se ha pagado de verdad --el del 25-08-- estaba en
  `mm/vmm.rs`, o sea en **clase A**, que es la que no puede bajar. Conviene
  tenerlo delante: la sorpresa vino de donde no se estaba mirando.
- **Los numeros de esta hoja son de un dia.** Estan medidos, y en cuanto alguien
  escriba mil lineas dejan de valer. El metodo esta en la seccion 2 para poder
  repetirlo.

---

# El resumen en una frase

> **Lo que baja no es "el codigo peligroso": es el codigo que come lo que le dan
> y no necesita privilegio para hacerlo -- y antes de que baje nada hay que
> construir tres cosas, de las que dos ya tienen molde y una no existe.**
