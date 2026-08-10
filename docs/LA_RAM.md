# LA RAM NO ES UNA BODEGA

> *"Vamos a tratar a la RAM con respeto. Pero ese dia que sea tratada como
> bodega, SE ACABO."* -- Eddi, 2026-08-09
>
> Escrito para que BMO-X tenga **identidad propia** en esto. Windows y Linux ya
> tienen la suya y llevan decadas de codigo escrito encima; copiarlas seria
> heredar sus deudas sin heredar sus motivos.

## La frase que ordena el documento

**La RAM no es donde vive el programa: es donde el programa esta TRABAJANDO
ahora mismo.** Lo demas esta en el disco y se trae cuando toca.

Dicho de otra forma, que es la de Eddi: no una bodega donde se amontona todo por
si acaso, sino **un quirofano** -- entra lo que hace falta para la operacion en
curso, y sale cuando termina.

Esto no es una optimizacion. Es una decision sobre **cual es el techo del
sistema**:

```
   modelo BODEGA      tamano de la app  <=  tamano del bufer  <=  RAM
   modelo QUIROFANO   tamano de la app  <=  tamano del DISCO
```

Con el primero, un juego de 40 GB no es que vaya lento: **es aritmeticamente
imposible**. Con el segundo, la RAM deja de decidir que puede correr.

---

# PARTE 0 -- Donde esta BMO-X HOY, medido

Antes de la teoria, la verdad, porque este documento no sirve si empieza
mintiendo. Sobre el DOOM que compilamos el 2026-08-09:

```
   code      592.691          rodata     30.631
   data      645.008          relocs     30.768
   ---------------------------------------------------
   EN MEMORIA (lo que el cargador mapea):  1.268.330 B
   fichero:                                1.299.512 B
```

Con su WAD dentro del paquete, el fichero pesaria **~5,5 MB** y la memoria
seguiria siendo **1,27 MB**. Eso es el modelo quirofano, y ya funciona **para
los datos**: la seccion `Resources` no se mapea, se lee por offset cuando se
pide (`bex::is_loadable` salta todo lo que no sea Code/RoData/Data/Bss).

**Pero el codigo todavia no.** `lanzar.rs::con_buffer` hace `fs::load(path,
buf)`: lee **el fichero entero** a un bufer estatico de 4 MiB, y de ahi se copia
otra vez al espacio del proceso. Con el WAD dentro serian **4,2 MB copiados dos
veces para nada**.

> ★ **Estado honesto: los DATOS ya son quirofano; el CODIGO sigue siendo
> bodega.** Todo lo que sigue existe para cerrar esa frase.

---

# PARTE I -- Como se ha tratado la RAM, y que compro cada idea

No para copiar: para **saber que se esta rechazando y por que**. Cada fila dice
que compro la idea, que costo, y si entra en BMO-X.

### 1. Overlays (1950-60)

El programador partia el programa a mano en trozos que se pisaban en memoria, y
escribia el cuando. Compro: programas mas grandes que la maquina. Costo: **lo
hacia el programador**, y equivocarse era corromper el programa.

Sigue vivo donde no hay MMU (microcontroladores). En BMO: **no**. Pero conviene
saber que la idea de "solo lo que hace falta ahora" es la mas vieja del oficio,
y que lo que cambio no fue la idea sino **quien lleva la contabilidad**.

### 2. Memoria virtual -- Atlas, 1962

El Atlas de Manchester automatizo el trasiego entre un nucleo rapido pequeno y
un tambor magnetico grande y lento, dandole a cada usuario **la ilusion** de una
memoria enorme y rapida. Es el origen del termino.

Compro: que el programador dejara de llevar la contabilidad. Costo: **la
ilusion**, que es lo que hay que mirar de cerca -- desde entonces el sistema le
miente al programa sobre cuanta memoria hay, y de esa mentira salen casi todos
los problemas de las filas 6 y 7.

### 3. Demand paging

Una pagina entra en RAM **cuando alguien la toca**, no antes. Compro: arranque
inmediato y memoria residente proporcional a lo que se usa de verdad. Costo: un
fallo de pagina en medio de cualquier instruccion, y la necesidad de decidir
**a quien echar** cuando no cabe (FIFO, LRU, reloj...).

En BMO: **si, y es el destino**. Ver la Parte VII.

### 4. `mmap` + page cache

Un fichero se **mapea** en el espacio del proceso; los bytes llegan solos al
tocarlos. Y como el cache de paginas es compartido, **la segunda vez es
gratis**.

Es lo que hace `llama.cpp` con los `.gguf`: mapea los pesos y el sistema pagina
lo que hace falta. Y GGUF, ojo con esto, **empaqueta pesos + tokenizador +
metadatos en un solo fichero portable**. O sea: la comunidad de IA llego a
*fichero unico + leer en el sitio* por su cuenta, que es exactamente el paquete
BEF. Dos caminos, la misma forma.

Compro: cargar sin copiar. Costo: el rendimiento pasa a depender de decisiones
del kernel que el programa no ve.

### 5. Huge pages / NUMA

Paginas de 2 MiB o 1 GiB para que la TLB no muera recorriendo tablas, y saber
**de que socket** es cada trozo. Compro: rendimiento real en cargas grandes.
Costo: fragmentacion y complejidad.

En BMO: **el sitio ya existe** (`cpu_vendor/profile.rs` con `CpuProfile`, y
`ryzen_5_5600x/topology.rs` que ya conoce CCD/CCX/SMT). No hoy, pero el dia que
toque no se empieza de cero.

### 6. ⛔ Overcommit -- **lo que BMO-X RECHAZA**

Linux te deja reservar mas memoria de la que hay, apostando a que no la vas a
usar toda. Compro: que `malloc` casi nunca falle y los programas no tengan que
manejar el "no hay". Costo: **`malloc` dejo de decir la verdad**.

### 7. ⛔ El OOM killer -- **el precio de la fila 6**

Cuando la apuesta se pierde, el kernel **elige un proceso y lo mata**. Es
polemico desde que se propuso en 1998, y la critica de fondo es exacta: si las
aplicaciones pidieran solo lo que necesitan, no haria falta -- el OOM killer
existe para tapar que `malloc` miente.

★★ **Aqui esta la identidad de BMO-X, y ya la tiene escrita en codigo.** El
kernel entrega **cuatro bloques por proceso** y el quinto `malloc` **devuelve
0**. No hay apuesta, no hay ilusion, no hay verdugo. Un programa que pide de mas
se entera **en el momento de pedir**, que es el unico momento en que puede hacer
algo al respecto.

> **Un sistema que nunca dice "no" acaba teniendo que decir "muerete".**

### 8. DirectStorage / el I/O complex de la PS5

Muchas lecturas pequenas en paralelo sin que la CPU las micro-gestione, y la
descompresion delegada a la GPU: el dato va del SSD a la VRAM por el camino
corto. Sony le puso silicio dedicado.

Compro: que el disco deje de ser el cuello de botella del streaming. Costo:
hardware y una API entera.

★ **Y el matiz honesto que casi nadie cuenta**: los juegos suelen operar a
profundidades de cola de **uno a cuatro**. Lo que gana DirectStorage no es
"cola mas larga" a secas -- es **quitar el coste de CPU por peticion**, que es
lo que impedia tener muchas en vuelo. Se dice aqui para que en BMO no se persiga
el numero equivocado.

En BMO: **es el modelo, y es la Parte V.**

### 9. CXL y memoria por niveles

Memoria conectada por PCIe, mas lejos y mas barata, con el sistema moviendo
paginas entre niveles. Es la frontera de hoy en servidores.

En BMO: **no**, y conviene decir por que -- pide hardware que no tenemos y
resuelve un problema (memoria de terabytes por nodo) que no tenemos.

---

# PARTE II -- El censo de crates, y que hace falta de verdad

Rust `no_std` tiene asignadores hechos. **Ninguno resuelve el problema de BMO,
y esa es la conclusion util**, pero uno de ellos es el que hay que copiar.

| Crate | Que es | Para BMO |
|---|---|---|
| `linked_list_allocator` | Lista enlazada sobre los huecos libres, sin estructuras aparte. El clasico de `rust-osdev` | ★ **El candidato**: simple, auditable, y encaja exacto sobre UN bloque grande |
| `buddy_system_allocator` | Buddy, casi drop-in del anterior | Menos fragmentacion externa, mas desperdicio interno. Segunda opcion |
| `talc` | Estilo dlmalloc con boundary tagging y buckets. `O(n)` peor caso al reservar, `O(1)` al liberar y al recrecer en el sitio | El mas rapido de los tres. Y el mas codigo que auditar |
| `slab_allocator_rs` | Slabs por tamano + buddy para lo grande de 4096 | Cuando haya objetos de tamano fijo repetidos |
| `buddy-alloc`, `simple-chunk-allocator` | Variantes para embebido | -- |

**Lo que BMO necesita no es un asignador de kernel: es uno de RING 3 sobre
`KIND_MEMORIA`.** El kernel entrega *un bloque grande, entero y contiguo* y no
sabe repartirlo -- a proposito. Quien reparte es codigo de usuario, encima.

Ese es exactamente el hueco donde hoy `realloc` devuelve 0 y lo dice.

★ **Y la regla de este repositorio se aplica igual aqui**: se estudia como lo
hacen, no se enlaza. Un asignador es ~300 lineas y es **el sitio donde vive la
politica de memoria del sistema**; que sea de otro es regalar la decision mas
propia que hay.

---

# PARTE III -- La identidad: siete reglas

Lo que hace a BMO-X distinto de Windows y de Linux en esto. Cada una con su por
que, porque una regla sin motivo no sobrevive al primer apuro.

**1. La RAM guarda el conjunto de TRABAJO, no la app.**
Todo lo demas esta en el fichero y se trae por offset. El paquete BEF ya esta
hecho para eso: el indice dice donde esta cada cosa sin leerla.

**2. `malloc` no miente. Nunca.**
Sin overcommit, sin OOM killer. Si no hay, se contesta que no hay, **en el
momento de pedir**. Ya es asi y no se toca.

**3. Lo que el kernel entrega, lo entrega ENTERO y CONTIGUO.**
Un bloque a trozos sin que el llamante lo sepa es peor que un "no". Es la misma
regla que ya sigue el bufer de archivos.

**4. Copiar es un coste, y se cuenta.**
Hoy hay **dos copias** por lanzamiento: disco -> bufer de rebote -> proceso. La
primera sobra entera y la segunda debe acabar siendo DMA.

**5. El destino de una lectura es una CAPABILITY, no un puntero.**
`ARCH_OP_LEER_EN` no valida direcciones: exige el handle del bloque, y comprobar
es **una resta contra lo que el kernel entrego**. Sin validador de punteros, sin
superficie de ataque. Esto ya funciona y es la pieza que hace posible todo lo
demas.

**6. Sin swap.**
Un sistema que pagina a disco lo que el programa creia tener en RAM vuelve a la
ilusion de la fila 2. Si no cabe, no cabe, y se dice. **Leer del disco lo que
NUNCA se prometio que estuviera en RAM es otra cosa distinta** -- eso es la
regla 1, y es lo que si se hace.

**7. Lo que se declara, se cumple o se grita.**
Ver la Parte IV.

---

# PARTE IV -- La burocracia inteligente que verifica

*"Burocracia"* aqui no es papeleo: es que **cada peticion deje rastro y cada
promesa se pueda comprobar**. Un sistema que no sabe contar su propia memoria no
puede prometer nada sobre ella.

Lo que ya hay:

- `INFO_MEM_ENTREGADA` -- cuanto se le dio a Ring 3, **lo dice el kernel y no el
  programa**. Es lo que permitio verificar `KIND_MEMORIA` en metal sin creerse
  la palabra del proceso.
- La **autopsia** cuenta fugas al morir un proceso, por objeto (`LA PANTALLA`,
  `EL SONIDO`...).
- El contador de peticiones se suelta en `revoke_all`: un pid reutilizado no
  hereda las del muerto.

Lo que falta, y es el trabajo de esta parte:

| Que | Por que |
|---|---|
| **Cuanto se COPIA por lanzamiento** | Hoy nadie lo mide, y es el numero que dice si la regla 4 se cumple |
| **Residente vs. fichero, por proceso** | La cifra que demuestra el modelo quirofano: `1,27 MB residentes / 5,5 MB de fichero` |
| **Cuantas lecturas por recurso** | Un `fread` por byte y uno por bloque se ven igual desde fuera |
| **Marca de agua de RAM** | El maximo alcanzado, no el actual. El actual no dice si estuvo a punto de no caber |
| ★ **Un `.bex` declara lo que va a pedir** | La seccion `Manifest` (`0x09`) existe **vacia**. Un paquete que declara "necesito 8 MiB" permite decir que **no** ANTES de arrancarlo, en vez de a mitad |

La ultima es la que cierra el circulo con la regla 2: hoy se dice "no" al quinto
`malloc`; con el manifiesto se puede decir "no" **antes de empezar**, que es
cuando el fallo no cuesta nada.

---

# PARTE V -- Las 32 ranuras, y por que son 32 aqui

Eddi: *"que lo declare el 32 slots o bueno MAS si es que puede"*. Los numeros,
exactos:

| Interfaz | Colas | Comandos por cola |
|---|---|---|
| **AHCI / SATA** | 1 | **32** |
| **NVMe** | hasta 65.535 | hasta 65.536 |

**En BMO hoy**: `platform/drivers/storage/ahci/src/controller.rs` documenta en su
cabecera *"32 cabeceras de 32 bytes, una por ranura"* -- y todo el driver usa
**la ranura 0**. Treinta y una peticiones en vuelo que no se estan pidiendo.

Asi que la respuesta a *"o mas si es que puede"* es de dos pisos:

1. **En el Kingston, 32 es el techo fisico.** No es una eleccion de BMO: es lo
   que AHCI permite. Pasar de una a 32 es la mejora que si esta disponible, y no
   cuesta hardware.
2. **Mas de 32 exige NVMe**, y la maquina lo tiene... pero **ese NVMe es el
   Windows del dueno** y la escritura esta cerrada a proposito. Asi que NVMe en
   BMO es "otro disco", no "otro driver".

★ Y lo aprendido de la Parte I.8, para no perseguir el numero equivocado: **la
cola larga por si sola no acelera nada**. Lo que hay que quitar es el coste por
peticion y la espera sincrona. La secuencia correcta es:

```
   1. E/S asincrona     (que pedir no bloquee)      <- sin esto, 32 ranuras no sirven
   2. varias en vuelo   (usar las 32)
   3. DMA al destino    (que no haya rebote)
```

En ese orden. Empezar por la 2 seria llenar 32 ranuras y esperarlas de una en
una.

---

# PARTE VI -- Para que: IA, juegos, y lo que NO

**Juegos.** Es literalmente el modelo. El streaming de un motor moderno *es*
esto: indice, offset, traeme estos bytes ahora. No es que sea compatible -- es
lo mismo. Y el techo realista de esta maquina (3D por software de los 90 hecho
bien) cabe de sobra.

**IA.** Aqui hay que ser preciso, porque es facil prometer de mas:

- ✅ **Cargar**: mapear los pesos hace que un modelo mayor que la RAM arranque.
  Es real y es lo que hace `llama.cpp`.
- ✅ **Residencia**: solo esta en RAM lo que se toca.
- ⚠ **Generar**: la inferencia densa **toca todos los pesos en cada token**. Leer
  del SSD no salva la velocidad de generar; salva la carga y la memoria.
- ✅ **MoE**: ahi si, porque solo unos expertos se activan por token. Es el caso
  donde el modelo quirofano gana de verdad en ejecucion, no solo al cargar.

**Lo que NO resuelve**, dicho para que nadie lo suponga: la GPU, los hilos, y la
superficie de sistema. Tratar bien la RAM quita **una** pared de cuatro. Es la
que si no te deja clavado para siempre en "apps que caben en RAM" -- pero las
otras tres siguen donde estaban.

★ **La ventaja estructural sobre Windows y Linux**, y es la razon de este
documento: los dos llevan decadas de codigo escrito suponiendo *"carga todo y
luego mira"*. Aqui el formato **nace con el indice dentro**. No hay que quitar
nada, no hay compatibilidad que romper. Esa es la identidad, y solo se tiene una
vez -- se pierde el dia que alguien anada la primera app que suponga lo otro.

---

# PARTE VII -- El orden de trabajo

Cada escalon deja el sistema funcionando, que es la regla de la casa.

| # | Que | Bloqueante | Tam |
|---|---|---|---|
| 1 | **Que el cargador NO lea el fichero entero** -- cabecera + tabla + secciones cargables | ★ **La firma es un hash del fichero completo.** Hace falta `SectionHash`, que ya existe en `bef/signing.rs` **vacio** | L |
| 2 | **E/S asincrona**: que pedir no bloquee | -- | L |
| 3 | **Las 32 ranuras**, varias peticiones en vuelo | el 2 | M |
| 4 | **DMA al bufer del llamante**, fuera la pagina de rebote | -- | M |
| 5 | **El asignador de Ring 3** sobre `KIND_MEMORIA` | -- (desbloquea `realloc`) | M |
| 6 | **El manifiesto declara lo que va a pedir** | -- | S |
| 7 | **Demand paging**: el recurso no se lee, se MAPEA | `file_offset` **congruente** con la VA modulo pagina -- la regla `p_offset == p_vaddr (mod pagesize)` de ELF, **ya escrita** en `bef/writer.rs` sin cumplir, a proposito | XL |

El 1 es el que convierte la frase de la cabecera en verdad, y su bloqueante es
bonito: **la firma por secciones es la misma pieza que hace posible verificar
sin leerlo todo** *y* la que permitiria un paquete firmado en FAT32, que hoy es
imposible porque la firma vive como atributo de ESTRATOS.

Una pieza, dos problemas. Por ahi se empieza.

---

## Fuentes

- Atlas y el origen de la memoria virtual: <https://ethw.org/Milestones:Atlas_Computer_and_the_Invention_of_Virtual_Memory,_1957-1962>
- `mmap` y demand paging: <https://man7.org/linux/man-pages/man2/mmap.2.html>
- Overcommit y el OOM killer, con la critica: <https://www.baeldung.com/linux/memory-overcommitment-oom-killer> y <https://lwn.net/Articles/360439/>
- "When malloc() Never Returns NULL -- Reliability as an Illusion": <https://arxiv.org/pdf/2208.08484>
- DirectStorage: <https://learn.microsoft.com/en-us/gaming/gdk/docs/features/console/storage/directstorage/directstorage-overview>
- Profundidad de cola NVMe vs AHCI: <https://www.digitalcitizen.life/nvme-queue-depth-explained-why-it-matters-for-real-world-ssd-performance/> y <https://anandtech.com/show/7843/testing-sata-express-with-asus/4>
- `llama.cpp`, mapeo de pesos: <https://deepwiki.com/ggml-org/llama.cpp/3.2-model-loading-and-representation>
- Asignadores `no_std` de Rust: <https://lib.rs/memory-management>, <https://github.com/rust-osdev/linked-list-allocator>, <https://crates.io/crates/buddy_system_allocator>, <https://crates.io/crates/talc>, <https://crates.io/crates/slab_allocator_rs>
