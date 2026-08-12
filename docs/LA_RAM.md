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
mintiendo. Sobre el DOOM que compilamos el 2026-08-09, **re-medido con el
compilador construido desde `HEAD`** (la primera version de estas cifras salio
de un `bmo-c-front.exe` de las 13:07, anterior a la tanda del PAQUETE: daba
`code` 592.789 y fichero 1.299.608, o sea 160 bytes menos de los reales. Una
medida vale lo que valga el binario que la produjo):

```
   code      592.945          rodata     30.631
   data      645.008          relocs     30.768
   ---------------------------------------------------
   EN MEMORIA (lo que el cargador mapea):  1.268.584 B
   fichero:                                1.299.768 B
   secciones en el .bex:  CUATRO -- y ninguna es Bss
```

Con su WAD dentro del paquete, el fichero pesaria **~5,5 MB** y la memoria
seguiria siendo **1,27 MB**. Eso es el modelo quirofano, y ya funciona **para
los datos**: la seccion `Resources` no se mapea, se lee por offset cuando se
pide (`bex::is_loadable` salta todo lo que no sea Code/RoData/Data/Bss).

**Pero el codigo todavia no.** `lanzar.rs::con_buffer` hace `fs::load(path,
buf)`: lee **el fichero entero** a un bufer estatico de 4 MiB, y de ahi se copia
otra vez al espacio del proceso. Con el WAD dentro serian **4,2 MB copiados dos
veces para nada**.

## ★★ Y el numero que nadie habia mirado: el 90,3%

De los **645.008 bytes** de la seccion `data` de DOOM, **582.291 son CERO**.

```
   data              645.008 B
   de eso, ceros     582.291 B   ->  90,3%
   ceros / fichero   582.291 de 1.299.768  ->  44,8% del .bex entero
```

**Casi la mitad del fichero de DOOM son ceros que viajan.** Se guardan en el
disco, se leen del disco, se copian al bufer de rebote y se copian otra vez al
proceso -- cuatro veces pagado un byte cuyo valor ya sabiamos al compilar.

El motivo era de una linea: **el codegen de C no emitia una seccion `Bss`**. Un
global sin inicializador, o inicializado a cero, salia como bytes de `data`. Y no
faltaba la maquinaria: `BefBuilder::bss()` existe y funciona
(`bef/writer.rs:98`), el escritor ya salta las `Bss` al colocar y al volcar
(`writer.rs:189` y `:220`), y el cargador del kernel ya mapea `Bss` con
zero-fill (`bex.rs` acepta `file_size == 0` **solo** si la seccion es `Bss`, y
`proc.rs` hace `zero_frame` antes de copiar). **Estaba todo escrito menos quien
lo pidiera.**

★ Y es el ejemplo mas limpio de la frase que ordena este documento: un cero no se
guarda, **se declara**. La cola de ceros al final son solo 679 B, o sea que no se
arregla recortando el final -- hay que **particionar los globales** por "su
inicializador es todo ceros", que es lo que hace cualquier compilador desde 1970.

## [x] HECHO el 2026-08-09 -- `Codegen::separar_bss`

Medido con el **mismo compilador** y solo ese cambio en medio:

```
                    ANTES        DESPUES     diferencia
   fichero       1.299.768      807.072     -492.696   (-37,9%)
   code            592.945      592.945      igual
   rodata           30.631       30.631      igual
   data            645.008      152.224     -492.784
   bss                   0      492.784      no viaja en el fichero
   relocs           30.768       30.768      igual
   ---------------------------------------------------------------
   EN MEMORIA    1.268.584    1.268.584      IGUAL
```

★★ **Que la memoria no cambie es el resultado correcto, no un fallo.** Lo que se
quita es el TRANSPORTE -- el disco, la lectura, y las dos copias-- no el sitio
donde el programa trabaja. Esa distincion es el documento entero en una fila.

**Por que 492.784 y no los 582.291 de ceros**: el reparto es **por global, no por
byte**. Una tabla de 40 KB con tres valores puestos se queda entera en `.data`,
y sus ceros con ella. Los 89.507 de diferencia son eso. Repartir por byte pediria
partir un global en dos secciones, y entonces `&tabla[0]` y `&tabla[9999]` no
estarian en la misma.

**Los tres motivos por los que un global a cero se QUEDA en `.data`** estan en la
cabecera de `separar_bss`, y el tercero es el que no es obvio: el codigo de
seccion de una relocation solo sabe decir code/data/rodata -- **no hay valor para
`bss`**, asi que un global cuya direccion se guarda en otro global tiene que
quedarse donde la reloc lo sepa nombrar. Ampliarlo toca el formato Y el cargador
del kernel; es otra tanda.

[!] Y una mina que dejo el gate del BEF a la vista: `type_stack_size` devuelve 0
para un tipo que no conoce, asi que **dos globales pueden compartir offset**. El
mapa de traduccion se indexa por offset, y con dos duenos por clave una reloc
acababa apuntando dentro de `.bss` (`reloc[293]: offset 0x614d0 exceeds target
section size`). Se descartan las regiones vacias, y ademas el compilador lleva
ahora su propio guardia: si una reloc quedara en `.bss`, lo dice **con el nombre
del global delante** en vez de dejarselo al gate.

Nueve filas nuevas que EJECUTAN (`tests/globales.rs` y `tests/cargador.rs`);
362 verdes.

> ★ **Estado honesto al 2026-08-09, con el escalon 0 ya hecho: los DATOS son
> quirofano, los CEROS ya no viajan, y el CODIGO sigue siendo bodega** -- el
> cargador se sigue trayendo el fichero entero, solo que ahora ese fichero mide
> un 37,9% menos. Todo lo que sigue existe para cerrar la frase que queda.

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
| 0 | ★ **La seccion `Bss`**: que los ceros se declaren en vez de viajar | **[x] 2026-08-09** -- `Codegen::separar_bss`. DOOM: 1.299.768 -> 807.072 B | S |
| 1 | **El asignador de Ring 3** sobre `KIND_MEMORIA` | -- (desbloquea `realloc`, los >4 `malloc` y el contrato de `fread`) | M |
| 2 | ★ **Que el cargador NO lea el fichero entero** -- cabecera + tabla + secciones cargables | **[x] 2026-08-10** -- `bex::necesita`. DOOM+WAD: 6.313.632 -> **813.552 B leidos (-87,1%)** | L |
| 3 | ★ **DMA al bufer del llamante**, fuera la pagina de rebote | **[x] 2026-08-10** -- `disk::tramo_dma` + un comando por CLUSTER en FAT32 | M |
| 4 | **E/S asincrona**: que pedir no bloquee | **a medias 2026-08-10** -- el disco tiene DUENO y el driver se deja preguntar; falta que el que pide pueda irse | L |
| 5 | **Las 32 ranuras**, varias peticiones en vuelo | el 4 | M |
| 6 | **El manifiesto declara lo que va a pedir** | -- | S |
| 7 | **Demand paging**: el recurso no se lee, se MAPEA | `file_offset` **congruente** con la VA modulo pagina -- la regla `p_offset == p_vaddr (mod pagesize)` de ELF, **ya escrita** en `bef/writer.rs:46` sin cumplir, a proposito | XL |
| 8 | ★ **El volcado del compositor: PAGE FLIP en vez de copia** | **el controlador de pantalla**, que es lo que esta aparcado con Vulkan | XL |

## El escalon 8, y por que faltaba de esta tabla (2026-08-12)

**Esta tabla existe para contar las copias, y llevaba una sin contar.**

Cada fotograma, el compositor copia su lienzo al panel. Es la ULTIMA copia del
sistema y la unica que ocurre **sesenta veces por segundo** en vez de una vez por
fichero. Y no estaba escrita aqui, asi que no se podia pagar.

El 2026-08-12 se troceo esa copia por regiones sucias (`userland/src/sucio.rs`):
de 8,3 MB por fotograma a los pixeles que de verdad cambiaron. **Eso es un
escalon, no el destino** -- y conviene decirlo con la frase que ya esta escrita
tres parrafos mas abajo en este mismo documento: *optimizar el transporte de unos
bytes que no deberian existir es el orden equivocado*.

### Lo que seria REFLEJAR aqui

**Page flip**: dos framebuffers y cambiar la direccion que lee el escaner de
video. Cero copia, cero desgarro, y el coste es escribir un registro.

### Y por que NO se puede hoy, dicho con su nombre

Porque despues de `ExitBootServices` **el GOP ya no existe**: BMO-X tiene la
direccion del framebuffer que le dio el firmware y ningun modo de decirle a la
tarjeta que mire a otro sitio. Cambiar la base del escaner son registros del
controlador de PANTALLA de la GPU -- lo que esta aparcado en
`platform/drivers/gpu/rdna4/PLAN_VULKAN.md`.

O sea que el escalon 8 **no es trabajo de compositor**: es la primera cosa util y
pequena que desbloquearia ese driver, y por eso se apunta aqui. Un dia se pagara,
y ese dia el troceado por regiones deja de hacer falta.

### El kernel, en esto, ya cumple

`KIND_FRAMEBUFFER` contesta cuatro preguntas --base, dimensiones, stride,
bytes-- y se aparta. **No mira un pixel, no copia un byte**: verifica el trato y
refleja. La copia que queda es entera de Ring 3, hecha por el compositor sobre su
propia memoria.

El **0** va primero porque es el unico escalon que **encoge todo lo demas antes
de optimizarlo**: quita 582 KB del fichero, de la lectura de disco y de las dos
copias, y no toca ni el kernel ni el formato. Optimizar el transporte de unos
bytes que no deberian existir es el orden equivocado.

El **2** es el que convierte la frase de la cabecera en verdad, y su bloqueante
era bonito: **la firma por secciones es la misma pieza que hace posible verificar
sin leerlo todo** *y* la que permitiria un paquete firmado en FAT32, que hoy es
imposible porque la firma vive como atributo de ESTRATOS. Una pieza, dos
problemas.

## [x] HECHO el 2026-08-10 -- `bex::necesita`, y la mitad estaba en el ESCRITOR

La pregunta correcta no es *"cuanto mide"* sino **"que necesita"**, y el fichero
sabe contestarla: la tabla de secciones empieza SIEMPRE en el byte 48, asi que
con **2 KiB de prologo** se sabe hasta donde llega lo ultimo que el cargador
toca -- `Code`, `RoData`, `Data`, `Relocs` y `Signature`. Los recursos se quedan
en el disco hasta que el programa los pida por `TASK_OP_MI_PAQUETE`.

```
   doom.bex + WAD de 5,5 MB     fichero  6.313.632 B
                                se lee     813.552 B   (-87,1%)
```

Y con eso **el paquete arranca**: `MAX_BEX` (4 MiB) dejo de ser el tope del
fichero y pasa a ser el tope de **lo que se ejecuta**.

### ★★ La trampa: la primera version ahorraba CERO

El cargador estaba bien y el numero salia igual de malo, porque el problema
estaba en el otro extremo. `BefBuilder::build` colocaba la firma **al final del
fichero, detras de los recursos**:

```
   [cab][tabla][Code][RoData][Data][Relocs][Resources][Signature]
                                               ^ el WAD    ^ y esto detras
   necesita -> hasta aqui --------------------------------------->
```

**Basta con que UNA seccion que el cargador mira quede detras del bulto para que
el bulto haya que traerlo igual.** Ahora los offsets se reparten en dos pasadas
--lo del cargador delante-- **sin tocar el orden de la TABLA**, que es el que
usan las relocations (`SeccionAbs64` guarda indices) y los hashes. Lo unico que
cambia es donde caen los bytes, y eso ya nadie lo suponia: el cargador lee
`file_offset` de la tabla, nunca una constante.

### Las tres piezas que hicieron falta

1. **`bex::necesita(prologo)`** -- el fichero contesta cuanto hace falta.
2. **`est::leer_y_firmar`** -- una pasada en la que **todos** los bytes le pasan
   al `bmo_hash::Hasher` por delante y solo se copia el principio. La firma sigue
   cubriendo el archivo entero: esto ahorra **RAM, no disco**, y decirlo importa.
3. **`inspect(bytes, tam_fichero)`** -- dos limites que antes coincidian por
   casualidad. Confundirlos convierte un fichero cortado en uno valido.

### Lo que esto cuesta, dicho

Las secciones que no se leen **no se verifican**. El gate garantiza **lo que
EJECUTA**; lo que el programa lea despues lo lee por su puerta, y su hash sigue
escrito en el fichero para quien quiera comprobarlo. Fingir que se comprobo algo
que no se ha llegado a leer seria peor que no comprobarlo.

---

## [x] HECHO el 2026-08-10 -- escalon 3: el disco escribe DONDE VA

Los bytes que si hacen falta pasaban por **dos rebotes** antes de llegar a su
sitio, y ninguno de los dos estaba ahi por necesidad:

```
   ANTES   disco -> pagina DMA (4 KiB) -> buffer de FAT32 (512 B) -> destino
   AHORA   disco ------------------------------------------------> destino
```

### 1. FAT32: de un sector por comando a un cluster por comando

`read_file` leia **de 512 en 512** y siempre al buffer interno del volumen. Con
un `.bex` de 813 KB eso son **1.590 comandos al disco y 1.590 copias**, y cada
comando es armar el FIS, tocar MMIO y esperar al HBA.

El contrato `BlockReader` ya aceptaba varios sectores de una vez y **nadie lo
usaba**: el mecanismo escrito y sin lector, otra vez. Ahora se lee el tramo
entero que quepa **directo al destino**, y el rebote queda solo para el rabo de
menos de 512 bytes -- que es donde de verdad hace falta, porque ahi el disco
entrega un sector completo y el llamante quiere una parte.

### 2. El HBA escribe en el buffer del llamante

`disk::read` mandaba SIEMPRE a una pagina de rebote y copiaba. Ahora se le
pregunta al mapa de paginas si el trozo que toca esta seguido en memoria
**fisica**; si lo esta, esa direccion va al PRDT y no se copia nada.

**No se supone: se comprueba**, y si la respuesta es que no, se cae al rebote de
siempre. Cuatro lecturas de memoria por pagina frente a copiar 4096 bytes.

Y el buffer del cargador pasa a estar **alineado a pagina**: con alineacion 1
podia empezar a mitad de pagina, y entonces el primer tramo no llegaba ni a un
sector -- el camino rapido existiria y no se tomaria nunca, sin que nada fallara.

### ★★ La mina que aparecio por el camino: `translate` no contesta lo mismo

| mapeo | lo que devolvia `vmm::translate` |
|---|---|
| pagina de 4 KiB | la **base** de la pagina, sin desplazamiento |
| pagina de 2 MiB o 1 GiB | la direccion **exacta**, desplazamiento incluido |

Su documentacion dice *"the physical base of the mapped page"*, que es cierto
para el primer caso y no para los otros dos. Mientras sus usuarios eran mapear
paginas --donde hace falta la base-- la diferencia no se notaba.

**Con DMA se nota de la peor manera.** El HBA escribe donde se le diga: sumarle
el desplazamiento a una respuesta que ya lo llevaba dentro apunta unos bytes mas
alla, y **el physmap del kernel esta montado con paginas de 2 MiB**, o sea que
ese es justo el caso de cualquier buffer que viva ahi. No habria dado una lectura
mala: habria dado **el disco escribiendo encima de memoria de otro**.

Por eso la pregunta se hace ahora con su propio nombre, `vmm::fisica_exacta`, y
`translate` se queda como esta: quien quiere la base pide la base, quien quiere
la direccion pide la direccion.

### Y la medida, para que no se pierda sola

`disk::cuentas_dma()` lleva los bytes que fueron directos y los que rebotaron, y
cada lanzamiento apunta su delta en CABINA. **Un camino rapido que nadie mide es
un camino rapido que un dia deja de tomarse en silencio** -- una pagina que
cambia de sitio, un buffer que se desalinea, y todo sigue funcionando, despacio.

---

## [x] HECHO el 2026-08-11 -- **la puerta que se habia quedado fuera: `archivo`**

El escalon 2 quito la bodega del CARGADOR. Y el mismo error seguia vivo, intacto,
en el sitio por donde un programa abre un fichero:

```rust
let mide = fs::tamano(ruta)?;   // doom1.wad = 4.196.020
reserve(i, mide)                // <- 1025 marcos CONTIGUOS
fs::load(ruta, dst)             // <- y leer los 4 MB de golpe
```

**Abrir se tragaba el fichero entero.** Para el WAD son 4 MiB contiguos en
fisico pedidos justo despues de que DOOM se llevara 12 MiB para su zona, mas una
lectura bloqueante de cuatro megas dentro de un syscall.

★ Y lo que lo condena no es el coste, es que **nadie pedia esos bytes**:
`w_file_stdc.c` no slurpea el WAD -- lee el directorio de lumps al abrir y luego
cada lump por `fseek`+`fread`. DOOM hacia lo correcto; era BMO el que le traia la
bodega entera para servirle una copa.

| | antes | ahora |
|---|---|---|
| Abrir el WAD | 4 MiB contiguos + leer 4 MB | un cursor y una ventana de 64 KiB |
| Un lump de 40 KB | ya estaba en RAM | 40 KB del disco, **al bloque del programa** |
| Tope de tamano | la RAM contigua que haya | **ninguno** |

Las dos piezas ya existian y estaban probadas en metal --`fs::abrir_rangos` y
`fs::leer_rango`, las del escalon 2-- y lo unico que faltaba era usarlas al otro
lado de la puerta. La ventana de 64 KiB existe solo porque `ARCH_OP_LEER` entrega
**siete bytes por llamada**; `ARCH_OP_LEER_EN`, que es por donde va `fread`, no
pasa por ella.

### El cursor solo avanza, y aqui retroceder es lo NORMAL

En el cargador, volver atras pasa **dos veces por carga** (los hashes y las
relocations viven al final), y se resolvio con una copia suelta del cursor. Un
juego leyendo su WAD salta en los dos sentidos todo el rato. La regla es la
misma, aplicada de continuo: la ranura guarda el cursor del flujo **y una copia
sin estrenar**, y una peticion por debajo de donde va el cursor vuelve a empezar
desde el principio. Cuesta un recorrido de la cadena FAT, **se cuenta**
(`archivo::cuentas()`), y el dia que ese numero crezca con el de bytes se sabra
que hace falta un cursor por lump -- mirandolo, no suponiendolo.

## [x] Y el escalon 3, hasta el final: **el disco escribe dentro del programa**

`ARCH_OP_LEER_EN` entrega los bytes en un bloque `KIND_MEMORIA` del proceso. Ese
destino es una VA de Ring 3, y el HBA no sabe lo que es una VA: rebotaba.

Ahora se escribe **por el espejo fisico del kernel**, que es la misma memoria
vista por la otra ventana. La direccion no se le pregunta a las tablas de pagina
--ese camino ya se recorrio y se volvio-- sino que `obj::memoria` **la apunta al
entregar el bloque**: `alloc_frames_contig` la acaba de devolver y los marcos son
contiguos por construccion, que es exactamente lo que un PRDT de una entrada
necesita. Un lump de DOOM va del plato a su zona de memoria sin pasar por ningun
sitio.

[!] Y con eso volvio a hacer falta una comprobacion que se habia quedado sin
escribir: **el PRD tiene que estar alineado a 2 bytes**. Mientras por ahi solo
pasaban marcos del asignador no podia fallar --una pagina esta alineada a 4096--
pero un `fread` a mitad de un lump cae donde caiga. Sin ese `if`, eso no seria un
rebote: seria `bmo_ahci` contestando `BadRequest` y un fichero llegando a medias
con un fault que habla del disco. Cuesta un `and`, y esta en `disk::tramo_dma`.

> Una condicion que *"en la practica siempre se cumple"* deja de cumplirse el dia
> que un camino nuevo entra por la misma puerta.

---

## [~] A MEDIAS el 2026-08-10 -- escalon 4, y lo que se encontro por el camino

Se fue a hacer *"que pedir no bloquee"* y lo primero que aparecio fue que la
premisa estaba mal:

> ⚠ **`read` NUNCA bloqueo la maquina.** El temporizador expropia, asi que
> mientras el kernel gira esperando al HBA, el escritorio sigue pintando. Lo que
> bloquea es **al que llamo**, que se queda dentro de una funcion de Ring 0.

Y eso convierte la expropiacion en el problema, no en la solucion:

### ★★ El fallo de verdad: el disco no tenia dueno

El HBA tiene 32 ranuras y este driver usa **la 0**, siempre. Un comando en vuelo
es estado global del puerto: una tabla de comando, un PRDT, un `PRDBC`. Y la
secuencia *armar -> campana -> esperar -> leer `PRDBC`* **se puede partir por la
mitad en cualquier punto**, porque el temporizador expropia.

Quien podia entrar en medio, hoy, sin inventar nada:

| camino | quien |
|---|---|
| `fs::` -> FAT32 | los `.bex`, la GPT, cualquier archivo del volumen de datos |
| `bmo_block` -> ESTRATOS | el arbol, los nodos, las firmas |
| Ring 3 | cualquier proceso que abra un archivo |

Dos de esos solapados **escriben la misma ranura**, y el primero acaba leyendo el
`PRDBC` del segundo: sectores del sitio equivocado, sin que nada falle.

**Ahora el disco tiene DUENO**, con dos decisiones dichas:

- **Un dueno, no un cerrojo.** Un `SpinLock` que tome alguien que despues muere
  se queda cerrado para siempre. Aqui se apunta *quien* lo tiene: el que espera
  comprueba si ese tid sigue vivo y, si no, **se lo quita y lo dice**. Es el
  mismo trato que la pantalla.
- **Un testigo, no un par `tomar`/`soltar`.** `read` tiene seis salidas; la
  version con dos llamadas se olvida en la sexta, y el sintoma es un disco que
  deja de contestar para siempre.

`disk::cuentas_dueno()` cuenta las esperas y los robos, y el lanzamiento los
apunta. **Cada espera era, antes de esto, una lectura solapada.**

### El driver ya se deja preguntar

`run_command` armaba, tocaba la campana y **se quedaba dentro** girando. Ahora
son dos: `emitir` (arma y toca) y `sondear` (mira y contesta `EnCurso` /
`Hecho(n)` / `Fallo`). `run_command` se queda como el bucle de los dos, porque
casi todos sus usuarios --montar, la GPT, el arranque-- no tienen a donde ir.

Eso no acelera el disco: hace que **el estado del comando se pueda mirar desde
fuera**, que es lo que "pedir sin esperar" necesita para existir.

### ⚠ Lo que FALTA, y por que no se entrego

Se escribieron `disk::pedir_lectura` y `disk::lectura_lista`, compilaban, y **se
quitaron antes de entrar: no habia quien los llamara.** Este proyecto ya se
tropezo tres veces con el mismo patron --el foco con doce pruebas y sin lector,
el arrastre de dos ventanas que nadie invocaba, `BlockReader::count` sin usar--
y en las tres la version escrita y muerta dio la impresion de que la funcion
existia. Un mecanismo sin lector es peor que un hueco: el hueco se ve.

Lo que falta **no es un driver**: es que el que pide pueda irse a otra cosa. Y
hoy no puede, por dos motivos concretos:

1. **`archivo::open` lee el fichero ENTERO** antes de devolver el handle. No hay
   ningun momento en el que "pedir y volver" cambie nada: el bloqueo esta en
   abrir, no en leer. Hasta que abrir sea *"empieza a traerlo"*, la asincronia no
   tiene donde vivir.
2. **El planificador solo cambia de tarea en la frontera de un trap**
   (`schedule_locked`: *"must run from a trap boundary only"*). Asi que una
   funcion de Ring 0 **no puede dormirse a mitad**: `yield_current()` desde
   dentro de `disk::read` no cede, marca -- y sigue corriendo con el CR3 de otra
   tarea. Dormir de verdad pide la INTERRUPCION del HBA y un `wait_key`, que ya
   existe en `Task` y ahora mismo solo usan los canales.

Ese es el orden del escalon 4 que queda: **interrupcion del HBA -> `wait_key` ->
`open` que empieza y no termina**. Y el 5 (las 32 ranuras) va detras, porque
varias peticiones en vuelo solo significan algo cuando hay quien las espere sin
girar.

### [x] El primer paso, hecho el mismo dia: **el disco AVISA**

`plat/irq.rs`, vector 49. El aparato ya no espera a que le pregunten: cuando
termina, **escribe el numero de vector en el LAPIC** y el CPU entra ahi.

**Por MSI, y eso quita un subsistema entero.** La forma clasica (INTx) es un
cable: el aparato lo baja, un IOAPIC lo traduce, y el kernel tiene que saber por
que patilla entra cada dispositivo -- routing de la placa, tablas del firmware,
el `_PRT` del ACPI. Burocracia de verdad: un intermediario al que hay que
pedirle permiso para que dos partes que ya se conocen se hablen. MSI es una
**escritura en memoria**: "cuando termines, escribe este numero en esta
direccion". Aqui no hay codigo de IOAPIC y con esto no hace falta.

Lo que se gana hoy, que no es la asincronia todavia:

> `sondear` lee tres registros por MMIO, y **el MMIO no pasa por cache**: cada
> lectura es un viaje al chipset. Girar sobre eso son millones de viajes para
> averiguar algo que el aparato sabia desde el primer microsegundo. Ahora se gira
> sobre un **atomico en memoria normal** --que sale de cache-- y solo se pregunta
> de verdad cuando el aparato ha dicho algo.

⚠ **Y la red de seguridad es la que permite encenderlo.** Cada 4096 vueltas se
pregunta por MMIO aunque no haya habido aviso. Si la placa no enruta MSI, si el
firmware dejo el vector enmascarado o si el aviso se perdio, el disco funciona
exactamente como antes. Un camino nuevo que solo funciona cuando el hardware
colabora **no puede ser el unico camino**: la placa que no colabore se quedaria
sin disco, o sea sin arrancar, y el sintoma no se pareceria a la causa.

El orden de encendido tampoco es negociable: primero MSI (**a donde** avisar) y
solo si eso quedo armado, `GHC.IE` (**que** avise). Al reves, el disco anuncia a
una direccion que no escucha nadie y se queda esperando respuesta.

`disk::irq_estado()` devuelve `(armada, avisos)`, y son dos cosas distintas a
proposito: un chipset puede aceptar la programacion de MSI y **no enrutarla**.
Si lo primero es cierto y lo segundo no sube, la respuesta esta en esa pareja.

**Lo que sigue faltando es el que duerme.** El manejador ya tiene donde llamar a
`wake_by_key`; lo que no hay es nadie bloqueado en esa clave, y para haberlo hace
falta que `archivo::open` empiece la lectura y no la termine.

### [x] `open` QUE EMPIEZA Y NO TERMINA -- hecho, y con la mitad justa

`TASK_OP_ARCHIVO_ASINC` (`0x27`) devuelve el handle **en cuanto sabe que el
archivo existe**. Los bytes llegan a trozos de 128 KiB, y **preguntar por el
archivo es lo que lo trae**: cada `ARCH_OP_LISTO` avanza uno y contesta
`(entero, bytes que ya llegaron)`.

```text
   antes   UN syscall dentro del kernel durante 813 KB
   ahora   SIETE syscalls de 128 KB, y entre ellos se vuelve a Ring 3
```

Volver a Ring 3 entre trozos es lo que importa: ahi hay **frontera de trap**, o
sea que el planificador puede dar el turno a otro por decision suya y no por
expropiacion. Y quien no quiera esperar tiene `leer_de_asinc`: pide un trozo,
pinta un fotograma, pide otro.

Por debajo, `bmo_fat32::leer_tramo` hace la lectura **reanudable**. El cursor es
el CLUSTER y no un offset: seguir la cadena desde el principio en cada llamada
seria recorrer el archivo entero por cada trozo -- cuadratico, y justo en el caso
que se queria arreglar. La prueba `leer_a_trozos_da_lo_mismo_que_de_una` compara
byte a byte contra `read_file`, porque si eso no cuadra el archivo cambia segun
cuantas veces se haya preguntado, y eso no falla: **corrompe**.

`Archivo::leer_de` usa el camino nuevo **sin cambiar por fuera**, y se cae al
viejo si el kernel no lo conoce. Un camino nuevo que deje sin archivos al que no
lo tenga no es una mejora, es una ruptura.

### ⚠ Y lo que TODAVIA no duerme, dicho sin adornos

Se escribio el brazo de `wait` para `KIND_ARCHIVO`: bloqueaba la tarea sobre la
clave del disco y la interrupcion la despertaba. Compilaba, **y no esperaba a
nada** -- porque traer el trozo (`archivo::avanzar`) sigue siendo sincrono, asi
que cuando la llamada vuelve el dato YA esta. Dormirse despues es dormirse hasta
que otro use el disco.

Se quito, junto con el contador de trozos que existia solo para el.

> **Lo que falta es una sola pieza, y ahora esta aislada**: que `leer_trozo`
> EMITA y vuelva, en vez de emitir y esperar. Con eso, `avanzar` deja un comando
> en vuelo, `wait` tiene a quien dormir, y la interrupcion a quien despertar --
> y las tres piezas ya existen por separado.

★ **Precision sobre `SectionHash`, porque la primera version de este documento lo
decia mal**: no esta "vacio". Esta **escrito y probado** -- BLAKE3 de 256 bits,
`verify`, y `chain_hash` para encadenar todas las secciones
(`bef/signing.rs:153-220`). Lo que falta es **quien lo escriba y quien lo lea**:
`signing::` no se referencia fuera de `bmo-abi`, o sea que ningun `.bex` producido
hoy lleva seccion `Signature` y el cargador no la busca. Es cableado, no diseno.

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
