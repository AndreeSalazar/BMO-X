# IOMMU MAESTRO -- la disciplina del DONDE, y su letra pequena

> Escrito el **2026-09-09**, junto a [`DMA_MAESTRO.md`](DMA_MAESTRO.md). Este
> contesta **DONDE** puede escribir un aparato; aquel, **CUANDO**.
>
> ** Y el orden entre los dos importa: **una IOMMU sobre un sistema que no sabe
> cuando un bufer esta en vuelo arregla el eje equivocado.**

---

# 1. QUE ES, EN UNA FRASE

Una MMU para aparatos. El aparato emite una direccion, y un chip en medio la
traduce contra unas tablas de pagina **que solo el kernel escribe**. Si no hay
entrada, el acceso se rechaza y se anota.

```text
   sin IOMMU   el aparato escribe donde le dijeron. Punto
   con IOMMU   el aparato PIDE escribir, y el silicio decide
```

*** Es lo unico de toda esta historia que pone la comprobacion **fuera del
alcance del software**. Los cuatro escalones baratos del plan son disciplina: un
fallo en el propio kernel se los salta todos. Esto no.

---

# 2. LAS TRES DEL MUNDO, Y CUAL TOCA AQUI

| | quien | como se descubre |
|---|---|---|
| **AMD-Vi** | AMD | tabla ACPI **IVRS** |
| **VT-d** | Intel | tabla ACPI **DMAR** |
| **SMMU** | ARM | el device tree |

** En esta maquina --Ryzen 5 5600X sobre MSI A320M-- toca **AMD-Vi**, y
`plat/placa.rs:261` **ya lee el IVRS**: cuantos hay y donde viven. La tabla es
**estatica**, asi que se lee bajo la ley de la casa sin discutir: *"tablas ACPI
estaticas SI, AML NUNCA"*.

`NEUTRO/ARQUITECTURAS.md` ya tenia las tres en una fila con la misma respuesta
en la ultima columna: **"esta en BMO-X? no"**.

---

# 3. QUE COPIAR DEL MUNDO

## ★ 3.1 La distincion identidad / aislado, que es toda la utilidad

Linux le da a cada aparato un **dominio**, y solo hay dos formas utiles:

```text
   IDENTIDAD   la direccion del aparato ES la fisica. No aisla nada, pero
               TRADUCE -- y ya puede rechazar lo que no este mapeado
   AISLADO     el aparato ve su propio espacio. Dos aparatos no se ven
```

*** Y el primero es el que hay que copiar **primero**, porque es casi gratis y
ya compra lo importante: **un aparato que se vuelve loco deja de poder escribir
en una pagina que nadie le mapeo**. El aislamiento entre aparatos es un segundo
escalon, y aqui hay tres aparatos, no trescientos.

## 3.2 La proteccion desde el arranque, que es lo que casi nadie hace bien

El firmware entrega la maquina con los aparatos ya vivos --el disco arranco, el
USB enumero--. Entre ese instante y el momento en que BMO-X programe la IOMMU
**hay un hueco en el que nadie vigila**.

```text
   lo que hace el mundo   UEFI puede dejar la IOMMU armada y pasar el relevo
                          (DMA protection). Linux lo aprovecha si esta
   lo que se puede hacer  ACORTAR el hueco: programarla antes de AHCI y del xHC
   lo que NO se puede     cerrarlo del todo sin ayuda del firmware
```

** Ese hueco **se declara, no se tapa**. Es exactamente la forma que ya tiene
`NEUTRO/CENSO.txt` con el microcodigo y el SMM: se escriben aunque no se puedan
controlar, porque *"un censo que solo lista lo que puede controlar no es un
censo"*.

## 3.3 Que un fallo se ANOTE, no solo se rechace

AMD-Vi tiene un registro de eventos: quien pidio que direccion y por que se le
dijo que no. **Eso vale mas que el rechazo.** Un aparato que escribe donde no
debe hoy no deja rastro; con esto deja su BDF, su direccion y su motivo.

> El objetivo no es que el DMA no pueda hacer dano. Es que cuando lo haga, se
> sepa cual y cuando.

---

# 4. [!] Y QUE SERIA UN ERROR COPIAR

## Error 1: una capa generica de IOMMU

Linux tiene `iommu_ops` con backends para VT-d, AMD-Vi, SMMU, s390 y mas. Es la
abstraccion correcta **para quien tiene que arrancar en todas**. Aqui hay UNA
placa perfilada, y LEY 24 dice que el hardware se PERFILA:

```text
   una abstraccion de un solo implementador no es una abstraccion:
   es una capa de indireccion con un fichero de mas
```

Si algun dia hay una segunda maquina, la abstraccion se saca **de las dos**, que
es como salieron todos los perfiles de esta casa.

## Error 2: los grupos de IOMMU

Linux agrupa aparatos que no puede aislar entre si, por topologia PCIe --puentes
sin ACS, funciones que comparten un requester ID--. Es complejidad real y
necesaria **cuando pasas aparatos a maquinas virtuales**.

** BMO-X no virtualiza. Traerse los grupos seria traerse la solucion de un
problema que no se tiene, y esta casa ya lleva un aviso escrito sobre eso.

## Error 3: creer que sustituye a la disciplina

Y es el error caro. La IOMMU contesta *"puede este aparato tocar esta pagina"*.
No contesta:

```text
   [ ] si el bufer sigue siendo del aparato o ya se libero   -> DMA_MAESTRO
   [ ] si dos aparatos se hablan sin pasar por la RAM        -> peer-to-peer
   [ ] el microcodigo y el SMM                               -> imposible
```

---

# 5. LA LETRA PEQUENA: LO QUE CUESTA CUANDO FUNCIONA

```text
   1. ES UN VMM ENTERO      tablas, cache de traducciones del aparato, cola de
                            comandos e invalidacion. No es un `if`
   2. CUESTA LATENCIA       cada acceso del aparato traduce, y su cache falla
                            igual que un TLB. La invalidacion, ademas, es una
                            orden POR COLA: no es un `invlpg` local
   3. RIESGO DE ARRANQUE    programada mal, la maquina no arranca. Y el fallo
                            no se parece a un fallo de IOMMU: se parece a un
                            disco que no responde
```

[!] El 3 es el que decide que este sea el ULTIMO escalon y no el primero. Los
cuatro de antes se prueban en el anfitrion y no pueden dejar la maquina sin
arrancar; este si.

---

# 6. LO QUE HARIA FALTA, EN ORDEN

```text
   1. leer el IVRS de verdad     ya se lee -- `plat/placa.rs:261`
   2. mapear sus registros MMIO  y su juez ya existe: `bmo-mmio-juicio`
   3. la tabla de aparatos       una entrada por BDF
   4. las tablas de pagina       en modo IDENTIDAD primero
   5. la cola de comandos        y su invalidacion
   6. el registro de eventos     que es lo que da el rastro
   7. encenderla ANTES de AHCI y del xHC
```

** Los pasos 1 y 2 estan hechos o casi, y no por este plan: estaban hechos
porque la placa se confiesa entera desde el 07-09. Eso es lo que hace que este
maestro pueda escribirse con lineas de codigo en vez de con deseos.

Ver [`PLAN_EL_NEUTRO_VIGILADO.md`](../plan/PLAN_EL_NEUTRO_VIGILADO.md), paso N7.
