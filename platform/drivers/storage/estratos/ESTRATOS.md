# ESTRATOS -- de ALFA a 1.0

> **[!] ESTE DOCUMENTO SE BORRO POR ACCIDENTE Y SE RECUPERO EL 2026-08-17.**
>
> Nacio en `9da276ab` y lo borro **`b33f3966` el 2026-08-03** --*"chore: fuera
> seis librerias huerfanas - 3671 lineas que nadie cableo"*--. Vivia dentro de
> `platform/services/timeback/`, y al quitar la carpeta de codigo que nadie
> cableaba **se fue el diseno del sistema de ficheros con ella**. Nadie lo noto
> en dos semanas: un puntero roto no falla, manda al lector a la nada.
>
> Lo destapo el guardian de citas (`toolchain/tools/enlaces/enlaces.py`,
> `87259db8`), que vio tres ficheros de codigo citandolo **con numeros de
> section y con frases suyas entrecomilladas** contra una ruta que ya no
> existia.
>
> Ahora vive junto a su codigo, que es donde manda la regla de colocacion de
> `docs/README.md`: un documento sobre una pieza de codigo vive junto a esa
> pieza. Su carpeta de antes era la del codigo que se borro -- por eso cayo.

---

## 0. QUE ES CADA MITAD DE ESTE FICHERO

**De la section 1 en adelante, el cuerpo es el documento recuperado.** No se ha
tocado una frase: es la version del 03-08, y por eso vale como fuente --dice lo
que se decidio y por que-- y **no vale como descripcion de hoy**.

[!] Con una excepcion que hay que decir, porque la primera version de esta
cabecera prometia *"ni una frase tocada"* y ya no es exacto al byte: **al volver
al arbol le alcanzo la regla del ASCII**, y `ascii-sweep` le tradujo 214 letras
acentuadas (los simbolos con significado --★, ✅, ⏳-- se quedan, que es la regla
que la propia herramienta tiene escrita para markdown).

★ **Y el motivo de que hiciera falta es la misma historia otra vez**: este
documento se borro el **03-08** y el barrido de las fuentes corrio el **08-08**.
Escapo de la regla por estar ausente. Ninguna palabra cambia de significado; lo
que cambia es que ahora cumple lo que cumple el resto del arbol.

Esta section 0 es la unica parte nueva, y existe para lo que pidio el dueno el
17-08: *modelar todo para poder mejorar, y hacer un 1.0 que funcione, porque lo
que habia era ALFA.*

### La regla que hace falta antes de que alguien cite el cuerpo

El cuerpo se escribio **antes** de que existiera casi nada del codigo, y el
codigo le ha llevado la contraria en sitios concretos. Un documento de diseno que
se lee como si fuera el estado actual es peor que no tenerlo: se cita, y lo
citado es falso.

---

## 0.1 EL ESTADO REAL, MEDIDO CONTRA EL CODIGO (2026-08-18)

No sale de la memoria de nadie: sale de leer las crates, correr sus pruebas y
mirar la historia.

[!] **Esta section decia el 17-08 que el paso 5 estaba A MEDIAS. Ya no lo esta,
y el documento se quedo un dia entero mintiendo.** Es la segunda vez que este
fichero cuenta algo que el codigo desmiente -- la primera fue estar borrado doce
dias. Un documento de estado se envejece solo: se pone al dia o se borra.

```
   crate bmo-estratos          2.206 lineas, 53 pruebas verdes, no_std sin alloc
     lib.rs        538   el formato: superbloque A/B, generacion, sumas   10
     objects.rs    572   bloques, atributos, nodos, entradas               13
     read.rs       214   el descenso por el arbol (kernel y fmt, el MISMO)  6
     escritura.rs  649   la maquina de estados de la transaccion           18
     espacio.rs    233   la contabilidad de la section 9                    6

   kernel ring0/fsys/estratos/   1.604 lineas -- monta, LEE y YA ESCRIBE
     mod.rs        437   el montaje y la identidad
     cursor.rs     441   el recorrido que Ring 3 usa para pintar
     nivel.rs      247   los niveles del arbol
     walk.rs       207   el descenso
     escribir.rs   204   ** CREAR UN FICHERO. Nuevo el 18-08
     dir.rs         68

   toolchain/tools/estratos-fmt      formatea un volumen desde el anfitrion
```

### Los pasos de la section 10, contra lo que hay

| paso | el documento decia | hoy |
|---|---|---|
| 1 FAT32 leer | desbloquea todo | **en metal** |
| 2 gate de identidad | sin esto no se escribe | **en metal** (07-26) |
| 3 capa de bloques | contrato unico | en codigo (`bmo-block`); AHCI si, **NVMe no** |
| 4 solo lectura | montar y leer | **en metal** (06-08): monta F:, pinta el grafo |
| 5 escritura | *"aqui empieza lo serio"* | **HECHO EN CODIGO** (18-08, `1c96b133`) -- ver abajo |
| 6 recolector | cuando haya que recoger | la CONTABILIDAD si (`espacio.rs`) y **el TRIM de la cola libre tambien** (17-08); el recolector no |
| 7 TimeBack encima | el historial deja de ser copia | no |

### El paso 5, con precision -- que se hizo y que falta

**Se escribe un fichero de verdad, desde Ring 3.** `1c96b133` cablea 582 lineas:
`ring0/fsys/estratos/escribir.rs`, dos syscalls, `userland/src/estratos.rs` y la
orden en la ventana. Ya no es el commit vacio del 17-08.

El copy-on-write se ve en el reparto, que es lo unico propio de esa pieza:

```
   base+0   el nodo del FICHERO      con su contenido dentro (residente)
   base+1   el bloque de ENTRADAS    las de antes + la nueva
   base+2   el nodo del DIRECTORIO   que apunta al bloque de entradas
   base+3   el ESTRATO               que apunta al directorio
```

Los tres ultimos no son del fichero: son **la version nueva del arbol**. En un
sistema que sobreescribe, anadir una entrada toca UN bloque; aqui no se toca
ninguno, se copian los que cambian. Por eso el arbol de ayer sigue entero y
alcanzable, que es la razon de que este sistema de ficheros exista.

#### Los TOPES de esta version, dichos y no descubiertos

| tope | valor | donde | quien lo sufre |
|---|---|---|---|
| bytes por fichero (crear) | **96** | `RESIDENTE_MAX`, `objects.rs:134` | `Gesto::Fichero` |
| bytes por fichero (copiar) | -- | `flujo.rs` lo tumbo el 19-08 | nadie: `Gesto::Copia` parte en bloques |
| entradas por carpeta (escribir) | **36** | `ENTRADAS_POR_BLOQUE`, `escritura.rs` | republicar cualquier nivel |
| entradas por carpeta (listar/buscar) | **64** | `MAX_ENTRIES`, `dir.rs` | `buscar_en`, y por debajo `open` y `resolver` |
| entradas por carpeta (formatear) | **36** | `cabe_la_carpeta`, `estratos-fmt` | desde el 19-08; antes no habia |

Ninguno es del formato: `Attr::en_bloques` admite cuatro niveles de indireccion
y hasta el 19-08 **solo el formateador del anfitrion los escribia**. `flujo.rs`
los escribe ya desde el kernel y sin `alloc`, pero de momento **solo lo usa
`Gesto::Copia`**: el contenido de `new` viaja dentro del renglon del syscall
(`ES_GESTO_MAX = 96` en el ABI), asi que el techo que le queda a crear **no es
de disco, es de puerta**. La copia lo esquiva porque su contenido no cruza el
anillo -- lo va a buscar el kernel.

[!] O sea que *"BMO-X guarda un fichero en su propio sistema"* es cierto **y** el
fichero que se escribe con `new` mide como mucho 96 bytes. Las dos mitades de la
frase importan.

#### ** EL GUARDIA DE LOS TOPES (2026-08-19) -- lo que estaba colgando de una casualidad

Los topes estaban dichos. Lo que no estaba era **quien los vigila**, y al mirar
el camino entero aparecieron tres agujeros del mismo eje:

```
   estratos-fmt        SIN TOPE   podia parir carpetas que el kernel no sabe tocar
   buscar_en           64         se tragaba el flag de truncado con un `_`
   leer_entradas       36         no miraba `a.size` antes de leer
```

**1. `buscar_en` convertia "no cabia" en "no existe".** `entries` contesta dos
cosas --cuantas leyo y si se quedo corto-- y la segunda se tiraba. Por debajo de
esa funcion estan `open` --el que arranca un `.bex`-- y `resolver` --el que baja
por la ruta de CUALQUIER gesto de escritura--. En una carpeta de mas de 64, un
programa que esta en el disco contestaba *"esa ruta no existe"* por el sitio que
ocupa en la lista. Ahora el `None` sigue siendo `None` --es verdad que no se
encontro-- pero CABINA dice cuando esa verdad puede ser corta.

**2. `leer_entradas` no reventaba por una casualidad aritmetica.** Lee las
entradas que ya hay a UN bloque, y `walk::flujo` **trunca en silencio** cuando el
destino se llena --a proposito: su otro cliente es el panel que pinta--. Con la
lista cortada, republicar habria publicado la carpeta **sin las entradas de la 37
en adelante**: el arbol viejo entero, el vivo con nombres de menos y el gesto
diciendo que fue bien.

★ **No pasaba porque 4096 no es multiplo de 112.** El corte deja 64 bytes
sueltos y la crate del formato lo rechaza por *"esto no es una lista de
entradas"*. Con `ENTRADA_LEN` de 64 o de 128 el resto daria cero y la perdida
seria muda. Una garantia de datos no puede colgar de una division que no sale
exacta: ahora se mira `a.size` antes de leer y se contesta
`CarpetaNoCabeEntera`, que es un motivo y no un accidente. Y la casualidad tiene
prueba propia (`un_listado_truncado_no_pasa_por_una_lista_entera`), para que el
dia que alguien redondee `ENTRADA_LEN` a una potencia de dos se caiga una prueba
y no un disco.

**3. El formateador podia crear el estado malo.** Escribe con `Vec`, asi que una
carpeta de mil entradas se escribia sin pestanear -- y quedaba un volumen que
arranca, que se lee, y que **rechaza toda escritura dentro de esa carpeta**. El
sitio de decir que no es el unico momento en que esa carpeta todavia no existe:
`cabe_la_carpeta`, antes de meter el primer fichero.

[!] Y el error nuevo no toca el ABI: el syscall de gestos **no devuelve codigo
de motivo**, devuelve 0 y cuenta el porque por CABINA. Un motivo mas es una
variante y una linea de texto, no una puerta nueva.

### ★★★★ GUARDO UN FICHERO EN METAL (2026-08-19)

```
   new prueba.txt hola   ->   fichero HECHO. generacion 4
```

`prueba.txt` (4 B) en la rejilla y en el grafo del Ryzen. **El paso 5 dejo de
ser codigo.**

[!] Y hasta ese dia **no habia funcionado nunca**, por UNA LINEA: `crear_fichero`
llamaba `reserve` y despues `barrera_hecha` saltandose `cerrar_datos()`, asi que
la barrera contestaba `FueraDeOrden` y el commit no ocurria. Faltaba desde
`1c96b133`. Ver Ep. 45 de la bitacora -- la leccion es que **probar cada pieza no
es probar el camino**.

Lo que se anadio encima ese mismo dia, todo sobre el mismo camino:

```
   leer desde Ring 3     `Archivo` resuelve ESTRATOS primero, FAT32 despues
   republicar la rama    crear/borrar/renombrar/carpeta a cualquier profundidad
   el arbol de un flujo  el techo de 96 bytes, con indireccion y sin `alloc`
   copia                 el contenido NO cruza el anillo: dos nombres y ya
   la hora               el campo `tiempo` llevaba un cero desde el dia uno
   marca NOMBRE          lo que hace PERMANENTE a una version, y la ref de rama
   historial             la cadena de versiones, dibujada
```

### ESTRATOS ES 1.0 -- 2026-08-19

```
   PARA 1.0        [x] escribir contenido           18-08 en codigo, 19-08 en metal
                   [x] releerlo TRAS REINICIAR      19-08  <- la prueba que vale
                   [x] el nivel de ocupacion decide si se acepta
                   [x] NUNCA dos escritores (se monta desde un solo sitio)
```

La casilla que faltaba era la del section 7 de `docs/metal/VERIFICACION_METAL.md`, y no
se podia pasar en el anfitrion: hay que apagar el Ryzen. Se apago, y al volver
la cadena de versiones seguia entera --dos de ese mismo dia y el estrato
original del formateador, con su nombre y sin fecha--.

** Eso es lo unico que separa una barrera que funciona de una que se cree. El
mensaje verde dice que el disco acepto; el arranque siguiente dice que es
verdad.

### Lo que hay encima del 1.0, todo del 19-08

```
   leer desde Ring 3     `Archivo` resuelve ESTRATOS primero y FAT32 despues,
                         con la MISMA regla que usa `launch` para un binario
   republicar la rama    crear / borrar / renombrar / carpeta a cualquier
                         profundidad: UNA maquina y cuatro verbos
   el arbol de un flujo  el techo de 96 bytes, con indireccion y sin `alloc`
   copia                 el contenido NO cruza el anillo: dos nombres, y el
                         kernel lee la fuente el mismo
   la hora               `tiempo` llevaba un cero desde el dia uno
   marca NOMBRE          lo que hace PERMANENTE a una version -- y la
                         referencia que hace posible una rama
   vuelve N              un puntero. No copia, y no pierde lo de en medio
   historial             la cadena de versiones, con fechas y nombres
```

[!] Y el fallo que lo explica todo: `crear_fichero` **no habia funcionado
nunca**, por una linea (`cerrar_datos`). Ver Ep. 45 de la bitacora. La leccion
--*probar cada pieza no es probar el camino*-- es la razon de que esta casilla
de metal exista y de que no sea burocracia.

## 0.1.1 EL PLAN DESPUES DEL 1.0, ORDENADO (2026-08-18)

Reordenado a peticion del dueno: *"vamos a terminar con ESTRATOS, reorganizar el
plan para llegar a completar la base"*. Lo que sigue **no es una lista de deseos
por orden de ganas**: cada tramo desbloquea al siguiente, y donde no lo hace se
dice.

### TRAMO 0 -- el 1.0. Una prueba, y no es de codigo

Apagar el Ryzen y encenderlo. `docs/metal/VERIFICACION_METAL.md` section 7.
Nada de lo de abajo vale nada si esta falla: seria construir encima de una
barrera que se cree.

### TRAMO 1 -- LA BASE: que se pueda usar como un sistema de ficheros

[!] **Este tramo se escribio el 18-08 y el 19-08 lo dejo a medias de verdad.**
Decia *"solo en la raiz"* y *"no se puede volver a leer desde Ring 3"*: las dos
son falsas desde ese mismo dia. Lo que queda es el techo, y el techo se partio
en dos mitades que no son el mismo trabajo.

| # | que | estado |
|---|---|---|
| 1.1 | **leer el contenido desde Ring 3** | **HECHO** (19-08). `Archivo` resuelve ESTRATOS primero y FAT32 despues, con la misma regla que usa `launch`. |
| 1.2a | **el techo de 96, por el lado del DISCO** | **HECHO** (19-08). `flujo.rs` construye el arbol de indireccion sin `alloc`, y `Gesto::Copia` lo usa: una copia de FAT32 escribe lo que mida. |
| 1.2b | **el techo de 96, por el lado de la PUERTA** | El contenido de `new` viaja dentro del renglon del syscall: `ES_GESTO_MAX = 96` en el ABI. Para escribir 5 KiB desde Ring 3 hace falta pasar un **buffer**, no bytes en linea. ** Y esa misma puerta es la que necesita `guardar`. |
| 1.3 | **subir el tope de 36 por carpeta** | `ENTRADAS_POR_BLOQUE` = un bloque de entradas. Necesita que `:entradas` use indireccion **al republicar** -- la misma maquinaria de 1.2a, asi que sale casi gratis. Junta de paso los tres numeros distintos (36 escribir / 64 listar / 36 formatear). |
| 1.4 | **el guardia de los topes** | **HECHO** (19-08). No sube ningun tope: impide que se pasen en silencio. Ver *"El guardia de los topes"* en la section 0.1. |

★ **Y falta un verbo, que no estaba en ninguna lista.** No hay `guardar`: los
gestos son crear, carpeta, quitar, renombrar y copiar, y `entradas_con` rechaza
un nombre repetido. O sea que **un fichero tiene hoy una sola version para
siempre**; lo que versiona es el arbol. *"Cada escritura publica un estrato
nuevo"* es cierto del arbol y todavia no lo es del fichero. Es el quinto verbo de
la misma maquina --la lista con una CAMBIADA-- y comparte puerta con 1.2b.

### TRAMO 2 -- GESTIONAR: crear en cualquier sitio, borrar, renombrar, carpetas

[!] **Este tramo se reagrupo el 19-08, y la correccion la pidio el dueno.** El
"crear fuera de la raiz" estaba en el tramo 1 como si fuera otra cosa. No lo es:

```
   entradas_con(previas, nombre, nodo, dst)     "la lista con una MAS"
```

Borrar es esa misma funcion con una MENOS. Renombrar, con una CAMBIADA. Crear
carpeta es esa mas `nodo_de_directorio`. Y crear fuera de la raiz es ese mismo
trabajo aplicado a una RUTA en vez de a la raiz. **Son una sola maquina y cuatro
verbos**; hacer el de crear por separado seria escribirla dos veces.

La maquinaria de disco es la que `crear_fichero` ya tiene: reservar, escribir el
arbol nuevo, barrera, superbloque alterno. Lo que cambia es que ahora hay que
republicar **cada nivel de la ruta**, no solo la raiz.

```
   [x] la mitad PURA          `entradas_sin`, `entradas_renombrando`,
                              `nodo_de_directorio_vacio` -- 7 pruebas (19-08)
   [ ] la mitad que ESCRIBE   recorrer la ruta y republicar nivel a nivel
```

★ **Y en copy-on-write borrar NO destruye nada.** Es publicar un arbol *sin* esa
entrada; el estrato anterior sigue entero y alcanzable. Esa es la diferencia
entera con un sistema que sobreescribe, y es lo que permite que el explorador
tenga un boton de borrar sin que de miedo.

### TRAMO 3 -- las comodidades de la ventana

Lo que el dueno llama *"agregar con `clear` eso para facilitar"*. Van aqui y no
antes porque **ninguna desbloquea nada**: mejoran el uso de lo que ya funciona.
Pulsar una fila de la rejilla, scroll propio para el grafo, recortar nombres
largos, y las que salgan de usarlo.

### TRAMO 4 -- EL RECOLECTOR: **APLAZADO A PROPOSITO** (decision del 19-08)

El dueno lo dijo asi: *"COMPACTAR no es tan mal, asi que abandono el GC si es
por motivos, aunque igual es Git viviente, no es necesario el GC"*. Y tiene los
dos motivos de su parte.

**1. No hay donde anotar un bloque libre.** ESTRATOS reserva con `log_head`, un
puntero que **solo avanza**, y por eso la ocupacion es una resta. No hay mapa de
bits ni lista de libres, asi que un recolector **no puede soltar un bloque
suelto**. Solo hay dos salidas y las dos son tanda grande:

```
   A  MAPA DE LIBRES   estructura nueva en el formato, que hay que escribir de
                       forma atomica y que sobreviva a un corte. Rompe la
                       propiedad de que la ocupacion sea una resta.
   B  COMPACTAR        copiar lo vivo hacia adelante y bajar `log_head`. Es el
                       limpiador de un log-structured FS: no cambia el formato,
                       pero mover un bloque cambia quien lo nombra, asi que hay
                       que republicar los arboles que lo apuntan.
```

★ **Elegido B**, y no por gusto: el cuerpo de este documento ya apostaba por ese
mundo --escritura siempre secuencial, *"lo que ama un SSD"*-- asi que A seria
contradecir el diseno para arreglar algo que todavia no duele.

**2. Y todavia no duele, con numero.** En 414 GiB caben mas de **veinte
millones** de estratos antes del 70 %, y hay prueba
(`espacio.rs::en_414_gib_caben_millones_de_estratos`). Un gesto cuesta entre 4 y
7 bloques, asi que son del orden de **doce millones de gestos** antes del primer
aviso ambar. A cien escrituras al dia eso son siglos.

[!] Lo que NO se aplaza es saber cuando dejara de valer: el panel `[numeros]` ya
pinta la ocupacion y su nivel, y el nivel 3 --por encima del 95 %-- **pone el
volumen en solo lectura antes de perder nada**. El sistema avisa mucho antes de
llegar; lo que no hay es quien recoja cuando avise.

** Y la mitad honesta ya estaba hecha desde el 17-08: `disco trim` le dice al
SSD que la cola libre es libre. La section 9 metia dos trabajos en una frase y uno
esta cerrado.

### TRAMO 4-bis -- lo que era el recolector, cuando toque

★★ **Un recolector antes del tramo 2 no tendria nada que recoger.** Mientras
solo se crea, todo estrato es alcanzable desde el superbloque: no hay basura.
**Borrar es lo que CREA el trabajo del recolector** -- en copy-on-write, quitar
una entrada no libera un bloque, solo deja huerfano el arbol viejo.

Y el numero dice que tampoco corre prisa despues: en 414 GiB caben **mas de
veinte millones de estratos** antes del 70 % aunque nada se comparta, y hay
prueba (`espacio.rs`, `en_414_gib_caben_millones_de_estratos`).

[!] Ojo con la palabra: la section 9 metia DOS trabajos en una frase y **uno ya
esta hecho**.

```
   decirle al disco que lo libre es libre    <- HECHO (TRIM, 17-08)
   marcar lo alcanzable y soltar lo viejo    <- esto es el recolector
```

### Lo que sigue fuera de todo esto

TimeBack encima (paso 7) y NVMe debajo de la capa de bloques. Ninguno de los dos
bloquea nada de arriba.

---

[!] **El riesgo de la section 12 sigue igual, y no baja con el progreso**:
*"aqui un bug no da un fault bonito en pantalla: se lleva el trabajo de
alguien."* El 1.0 se estrena en F: --el disco de datos-- y no en el NVMe, que es
el Windows del dueno.

---

## 0.2 DONDE EL CODIGO LE LLEVA LA CONTRARIA AL CUERPO

Tres desviaciones **deliberadas**, decididas al implementar. El cuerpo de abajo
sigue diciendo lo de antes; se listan aqui en vez de editarlo para no perder por
que se cambio.

**1. `disco_id` no es `[u8; 20]`.** La section 5 lo declara asi y **no caben**:
el `IDENTIFY` da 40 bytes de modelo y 20 de serie. Es el BLAKE3 de
modelo+serie+capacidad. La propiedad que importa --dos discos distintos dan
identidades distintas-- se conserva; el tamano no.

**2. `Superblock.estrato` es un `BlockPtr`, no un `Hash`.** Lo destapo escribir
el formateador, y el motivo esta escrito al lado del campo: *"con un hash solo
no se puede encontrar nada"*. Haria falta un indice hash -> direccion, y la
decision del modelo de objetos es justamente que **el que lee no necesita
indice**. `ESTRATO_LEN` paso de 192 a 224.

**3. El superbloque tiene campos que el cuerpo no lista**: `total_blocks` y
`log_head`. No son adorno -- la ocupacion de la section 9 es **una resta** entre
esos dos, porque el log se reserva con un puntero que solo avanza.

---

## 0.3 QUE ES 1.0, Y QUE NO LO ES

**1.0 = escribir CONTENIDO desde Ring 3 y volver a leerlo despues de
REINICIAR.** Nada mas y nada menos. Es el paso 5 terminado y su prueba de
persistencia; todo lo demas ya esta o es post-1.0.

```
   PARA 1.0        escribir contenido (reserve con datos de verdad)
                   releerlo tras reiniciar   <- la unica prueba que vale
                   el nivel de ocupacion decide si se acepta la escritura
                   NUNCA dos escritores (se monta desde un solo sitio)

   POST-1.0        el recolector (section 9) -- el documento ya avisa de que es
                   *"lo dificil, no el formato"*
                   TimeBack encima (paso 7 del orden)
                   NVMe debajo de la capa de bloques
```

[!] **Y el riesgo que el cuerpo dice en su section 12 sigue siendo el mismo: no
se ha reducido con el progreso.** *"Aqui un bug no da un fault bonito en
pantalla: se lleva el trabajo de alguien."* El 1.0 se estrena en F:, que es el
disco de datos -- no en el NVMe, que es el Windows del dueno.

---

## 0.4 LO QUE ESTRATOS VA A SER, Y NO ESTA EN EL CUERPO

Dicho por el dueno el 17-08, y cambia para que sirve el diseno: ESTRATOS no es
solo el formato en disco, es **la vista del disco entera**.

- **Explorador de ficheros al estilo del de Windows 11.** El cuerpo no lo
  contempla porque se escribio pensando en un formato, no en una interfaz. La
  tercera pestana `carpetas` --el mismo cursor pintado como lista de
  explorador-- ya es el primer trozo de esto.
- **Vistas de nodos graficos, y su verbo es RESTABLECER.** La pestana `nodos` ya
  pinta el arbol como grafo. Lo nuevo es para que: **ver los estratos como nodos
  y volver a uno**. Es la interfaz natural del copy-on-write, porque el historial
  ya tiene forma de grafo -- no hay que construirlo, hay que pintarlo.
- **Comandos unicos, y por eso hay TERMINAL para discos duros.** El principio ya
  se aplico una vez, al mudar `sella` del terminal principal a la ventana de
  ESTRATOS: **el verbo vive donde vive el objeto.**

  ✅ **El primer trozo esta puesto (17-08): la orden `disco`**, con `trim`,
  `espacio` y `barrera` dentro. Y el reparto que deja es el mismo principio
  llevado un paso mas: **`disco` administra el APARATO** --lo que gira, lo que
  se le devuelve, la barrera-- y la ventana de ESTRATOS administra el
  **VOLUMEN** --sellar, y algun dia restablecer un estrato--. Son dos objetos y
  por eso son dos sitios; lo que no puede volver a pasar es que el verbo viva
  donde no esta la cosa.

Eso convierte el recolector de la section 9 --hoy una politica escrita-- en algo
que **se VE**: el dueno mira los estratos y decide, en vez de leer una regla de
adelgazado. La section 9 ya dice *"politica, no automatismo"* y *"el dueno
manda"*; lo que faltaba era el sitio donde mandar.

[!] **Nada de esto entra en 1.0.** 1.0 es escribir y releer. Se escribe aqui
para que el diseno del paso 5 no cierre puertas que estas tres cosas van a
necesitar.

---
---

*A partir de aqui, el documento recuperado del 2026-08-03, sin una sola frase
tocada.*

---


> *Cada escritura deja una capa nueva encima sin destruir la de abajo.
> Leer hacia atras en el tiempo es bajar por los estratos.*

**Estado**: pasos 1-4 del section 10 HECHOS -- el kernel monta ESTRATOS y lo lee. La
escritura (paso 5) es lo siguiente. Este documento existe para que el formato se
decida ANTES de tocar un sector -- en un sistema de ficheros, equivocarse cuesta
datos.

---

## 1. La idea

TimeBack y BMO-FS parecian dos proyectos. No lo son.

Git guarda **blobs** (contenido direccionado por su hash), **arboles**
(nombre -> hash) y **commits** (una raiz con padres). Un sistema de ficheros
copy-on-write guarda **bloques** (contenido), **nodos** (nombre -> bloque) y
**superbloques** (una raiz que se cambia al final, cuando todo lo demas ya
esta en disco).

Son la misma forma. Git es un sistema de ficheros que resulto ser control de
versiones; un FS copy-on-write es control de versiones que resulto ser un
sistema de ficheros. Nadie los ha unificado del todo porque en Unix el FS ya
venia dado y Git tuvo que construirse encima, con un `.git/` que duplica todo
lo que el FS ya sabia.

BMO-X no tiene esa herencia. Puede hacer que **el historial no sea una
carpeta, sino una propiedad del suelo**:

- Guardar **es** commitear, porque nunca se sobreescribe nada.
- Recuperar un archivo de la semana pasada no es restaurar un respaldo: es
  leer un puntero viejo que jamas se borro.
- El sistema no puede "perder" el estado anterior por un fallo a media
  escritura, porque el estado anterior sigue intacto hasta que la raiz nueva
  esta completa y verificada.

---

## 2. Principios (no negociables)

1. **Nunca se sobreescribe un bloque vivo.** Se escribe uno nuevo y se cambia
   el puntero al final. Un corte de luz a media escritura deja el sistema en
   el estado ANTERIOR, entero, no en uno a medias.
2. **Todo lleva suma de verificacion.** El sistema de ficheros detecta su
   propia corrupcion en vez de confiar en que el disco devuelve lo que
   guardo. Un bloque que no cuadra con su hash es un FAULT en CABINA, no un
   archivo raro.
3. **Tener el handle ES el permiso.** Sin root, sin `chmod`, sin uid/gid, sin
   autoridad ambiental. Un proceso no puede *nombrar* un archivo al que
   nadie le dio acceso.
4. **La verificacion vive dentro.** El FS no entrega una capability
   *ejecutable* sobre una imagen que no paso `bmo-verify`. La admision deja
   de ser un paso del arranque y pasa a ser una propiedad del almacenamiento.
5. **El dueno manda sobre el espacio.** El recolector nunca decide solo que
   se pierde: avisa, propone y obedece (section 9).
6. **La particion de arranque NUNCA depende de ESTRATOS** hasta que ESTRATOS
   se lo haya ganado. A: se queda en FAT32; ESTRATOS vive en BMO-DATA.

---

## 3. Herencia: que se roba y que no

| Sistema | Lo que vale | Lo que se deja |
|---|---|---|
| **NTFS** | **Todo es un archivo, incluidos los metadatos** (la tabla maestra es ella misma un archivo, y por eso el FS puede hacer crecer sus propias estructuras con su propio asignador). **Atributos con nombre**: un archivo no es un chorro de bytes, es un conjunto de flujos. **Archivos pequenos residentes**: los que caben viven *dentro* de su registro y no gastan bloque | 30 anos de compatibilidad hacia atras |
| **ZFS / btrfs** | Copy-on-write, checksums en todo, arbol de Merkle (el hash raiz valida el arbol entero), instantaneas gratis | Volumenes, RAID, cache ARC, compresion -- nada de eso hace falta todavia |
| **Git** | Direccionamiento por contenido: **deduplicacion gratis**, y el historial son solo raices extra | Que viva en una carpeta aparte del FS |
| **Log-structured (NILFS2, LFS)** | Escribir **siempre secuencial**, que es exactamente lo que ama un SSD, y da instantaneas continuas | -- (pero trae el recolector: section 9) |
| **Plan 9 / Venti** | Archivo permanente por hash: lo que entra no se pierde | El servidor de red |
| **BMO** | Capabilities como permiso, `bmo-verify` como gate, CABINA como testigo | -- |

---

## 4. Modelo de objetos

Cuatro tipos, todos identificados por el **hash BLAKE3 de su contenido**.

```
  BLOQUE     bytes crudos. La unidad de datos.
  ATRIBUTO   un flujo con nombre: lista de bloques + tamano.
  NODO       un archivo o directorio = conjunto de atributos.
  ESTRATO    una raiz: nodo raiz + padre(s) + marca de tiempo + autor.
```

### Por que atributos y no "el contenido del archivo"

Esta es la idea que se le roba a NTFS y la que mejor encaja con el ABI de BMO.
Un `.bex` en ESTRATOS no es *un archivo*: es un nodo con varios flujos.

```
  hola_C.bex
    +-- :datos        el codigo, el que se ejecuta
    +-- :firma        el hash BLAKE3 firmado (lo que mira bmo-verify)
    +-- :manifiesto   que capabilities pide para correr
    +-- :origen       de que fuente salio, con que compilador, cuando
```

Ningun sistema de ficheros clasico permite eso sin inventarse convenciones de
nombres o archivos `.meta` sueltos que se pierden al copiar. Aqui el
manifiesto de capabilities **no puede separarse del binario**, porque es parte
del mismo objeto.

### ESTRATO: el commit que tambien es el superbloque

```
  estrato {
      raiz:     Hash        // el nodo raiz del arbol de directorios
      padre:    Hash        // el estrato anterior (0 = el primero)
      tiempo:   u64
      autor:    Autor       // kernel / usuario / proceso, con su pid
      motivo:   [u8; 64]    // "auto", "antes de instalar X", ...
      suma:     Hash        // BLAKE3 de todo lo anterior
  }
```

Montar el sistema de ficheros = leer el ultimo estrato valido. Volver atras
en el tiempo = leer uno anterior. **Son la misma operacion.** No hay codigo de
"restaurar": hay codigo de "montar", y se le pasa otro estrato.

---

## 5. Formato en disco

```
  LBA 0     SUPERBLOQUE A   + dos copias alternas. Se escribe la que NO
  LBA 1     SUPERBLOQUE B   + esta en uso; si el corte llega a media
                              escritura, la otra sigue entera.
  LBA 2..   MAPA DE ESPACIO  bitmap de bloques, el mismo un archivo
  ...       LOG              todo lo demas: bloques, nodos, estratos,
                             escritos SIEMPRE hacia adelante
```

**Superbloque** (el unico sitio con posicion fija):

```
  magico:      b"ESTRATOS"
  version:     u32
  bloque_tam:  u32           // 4096
  disco_id:    [u8; 20]      // modelo+serie del disco (IDENTIFY)
  estrato:     Hash          // el estrato mas reciente
  generacion:  u64           // el mas alto de los dos superbloques gana
  suma:        Hash
```

`disco_id` no es decoracion: es el **gate de identidad** grabado en el propio
volumen. Si ESTRATOS se monta en un disco cuyo `IDENTIFY` no coincide con el
que dice el superbloque, se monta **solo lectura** y CABINA grita. Un volumen
clonado a otro disco no se escribe por accidente.

### La escritura, paso a paso

1. Se escriben los bloques nuevos en la punta del log. *(Nada apunta a ellos
   todavia: si se corta aqui, es basura inofensiva.)*
2. Se escriben los atributos y nodos que los referencian.
3. Se escribe el estrato nuevo, con su suma.
4. **Barrera**: se espera a que el disco confirme que todo lo anterior esta
   en el plato, no en su cache (`FLUSH CACHE`).
5. Se escribe el superbloque alterno con la generacion +1.

El punto de no retorno es el paso 5, y es **un solo sector**. Antes de el, el
sistema es exactamente el de antes. Despues, el nuevo. No hay estado
intermedio observable -- que es la definicion de una transaccion.


### ⏳ La transaccion, hecha (2026-07-31) -- pero sin tocar el disco

`bmo_estratos::escritura` -- `Transaccion`, `Fase`, `Rechazo`. **Aqui no se
escribe un sector**: es la maquina de estados que decide el ORDEN, y el orden es
lo que cuesta datos si se equivoca. La E/S la hara el kernel.

Esa separacion es lo que permite **probar en el anfitrion la parte peligrosa**,
sin un disco delante. Hay 12 tests.

**Es una maquina de estados y no una lista de escrituras** porque la crate es
`no_std` sin `alloc`: un plan son varios KiB por bloque y no hay `Vec` que
devolver. Y la restriccion mejoro el diseno -- una lista se puede reordenar por
accidente; **una maquina de estados no deja**: `commit()` antes de
`barrera_hecha()` devuelve `FueraDeOrden`, no depende de que nadie se acuerde.

Lo que la maquina garantiza, cada uno con su test:

- el commit **no puede adelantarse a la barrera**;
- el superbloque nuevo va **siempre a la copia alterna** -- pisar la que manda
  deja el volumen sin ningun superbloque valido si el corte llega a mitad;
- no se reserva despues de cerrar los datos;
- el limite se comprueba en **cada** reserva, no solo al abrir: una transaccion
  puede empezar cabiendo y dejar de caber a mitad, y pasarse es escribir fuera
  de la particion;
- una reserva absurda no da la vuelta al contador;
- el gate de identidad y el 95 % rechazan **al abrir**;
- abandonar no deshace nada y no hace falta: los bloques quedan sin que nada
  los apunte y el volumen sigue entero. Es el regalo de no sobreescribir;
- ★ **el commit conserva el `disk_id`**. Construir el superbloque de cero lo
  dejaba en ceros, y el sintoma seria de los peores: se escribe bien *una vez*,
  y al siguiente arranque el gate de identidad da falso y ESTRATOS se monta en
  solo lectura **para siempre**, sin que nada lo explique.

**Lo que falta para escribir de verdad**: cablearlo al dispositivo (`write` +
`FLUSH CACHE` de verdad), construir los nodos y el estrato, y decidir el mando
que lo dispara. Nada de eso se ha tocado, y la escritura al disco sigue cerrada.

---

## 6. Nombres y rutas

Un directorio es un nodo cuyo atributo `:entradas` mapea nombre -> hash de
nodo. Nada mas.

- Nombres en **Latin-1**, un byte por caracter, igual que la consola y el
  teclado (ver `keyboard.rs`). Sin UTF-8 en Ring 0, sin decodificador en el
  camino. `n` y acentos funcionan porque el font ya los dibuja.
- Sin distincion de mayusculas al comparar, **pero conservando** como se
  escribio. Es lo que espera cualquiera que venga de Windows y no cuesta nada.

---

## 7. Capabilities: el permiso ES el handle

En Unix cualquier proceso puede *nombrar* cualquier ruta y el kernel decide
con uid/gid si le deja -- autoridad ambiental, justo lo que BMO-X rechaza.

En ESTRATOS, abrir no es "pedir por nombre y rezar":

```
  Un proceso recibe una capability a un NODO (tipicamente un directorio).
  Desde ella puede derivar capabilities a lo que hay dentro, nunca hacia
  fuera ni hacia arriba. No existe ".." que escape del arbol concedido.
  Los derechos (leer / escribir / ejecutar / listar) viajan EN el handle y
  solo pueden reducirse al derivarlos, jamas ampliarse.
```

Consecuencia practica: un compilador al que le das el directorio de su
proyecto **no puede tocar nada mas**, y no porque se lo prohiba una lista de
permisos, sino porque el resto del disco no existe para el. No hay root que
pueda saltarselo, porque no hay root.

### El gate de ejecucion

`abrir(nodo, EJECUTAR)` comprueba el atributo `:firma` contra el contenido y
lo pasa por `bmo-verify`. Si no cuadra, **no hay handle ejecutable** -- el
archivo se puede leer, copiar y borrar, pero no correr. La admision de
binarios deja de ser un paso del arranque y pasa a ser una propiedad del
suelo.

---

## 8. TimeBack: no es una capa, es la misma cosa

`platform/services/timeback` ya tiene el modelo (blobs, arboles, commits,
refs, journal, rollback, CLI: ~54 KB). Lo que cambia es donde vive:

| Hoy (TimeBack sobre un FS) | Con ESTRATOS |
|---|---|
| `tb add` copia el archivo a `objects/` | No copia nada: el bloque **ya esta** direccionado por contenido |
| `tb commit` escribe un objeto commit | Es el estrato que la escritura crea de todos modos |
| El historial ocupa el doble | El historial ocupa lo que **cambio**, y nada mas |
| Hay que acordarse de commitear | Escribir es commitear |

Los mandos de TimeBack siguen teniendo sentido, pero pasan a ser vistas sobre
el disco en vez de una base de datos paralela: `tb log` recorre la cadena de
estratos, `tb diff` compara dos arboles por hash (los subarboles con el mismo
hash se saltan enteros -- eso es gratis y es lo que hace a Git rapido), y
`tb restore` monta un estrato viejo.

### Un cambio obligatorio antes de nada

`timeback::hash` usa **FNV-1a**. Es rapido, determinista y perfectamente
valido para un indice... pero **no es criptografico**. En un sistema de ficheros
direccionado por contenido, dos bloques distintos con el mismo hash significan
que uno sustituye al otro **en silencio**: perdida de datos que ninguna suma
detecta, porque la suma es justo lo que colisiono. Y siendo FNV, provocar esa
colision a proposito es trivial.

**ESTRATOS usa BLAKE3** (`platform/abi/bmo-abi/src/bef/blake3.rs`, ya
presente, y el mismo que usa `bmo-verify`). Un solo algoritmo de hash en todo
el sistema: contenido, firmas y verificacion hablan el mismo idioma.

---

## 9. El recolector (GC)

Un FS que nunca sobreescribe llena el disco de versiones viejas. Alguien tiene
que decidir que se puede soltar. Esta es la parte dificil de verdad -- la que
todo el mundo subestima y la razon por la que a btrfs le costo una decada ser
fiable.

**Decision del dueno**: se implementa, con avisos, y el usuario manda.

### Como funciona

Un bloque se puede soltar cuando **ningun estrato conservado lo alcanza**. Se
recorren las raices vivas marcando lo alcanzable, y lo demas vuelve al mapa de
espacio. Como todo esta direccionado por contenido, un bloque compartido por
diez versiones se cuenta una vez.

### Politica, no automatismo

```
  conservar todos los estratos de la ultima hora
  conservar uno por hora del ultimo dia
  conservar uno por dia del ultimo mes
  conservar los marcados a mano (los que tienen nombre) PARA SIEMPRE
```

Un estrato con nombre -- *"antes de reparticionar"*, *"COBOL funcionando"* -- no
se toca nunca, aunque el disco este lleno. Los automaticos se van adelgazando
hacia atras en el tiempo.

### Los avisos (esto es CABINA)

El FS **nunca borra en silencio ni se llena por sorpresa**:

- Al 70 % de ocupacion: aviso ambar con cuanto ocupa el historial y cuanto se
  liberaria aplicando la politica.
- Al 85 %: FAULT rojo y propuesta concreta -- *"soltar 47 estratos automaticos
  de mas de 30 dias libera 12 GiB"*.
- Al 95 %: **modo solo lectura**. Antes de perder datos por falta de sitio, el
  sistema se planta y te lo dice.
- Y un mando manual: `estratos limpiar` con lo que va a soltar **listado antes
  de hacerlo**.

Tienes razon en que en un disco enorme esto importa poco. En tus 414 GiB de
BMO-DATA importa bastante, y en un SSD hay un motivo extra: los bloques
soltados hay que devolverselos al disco con `TRIM`, o el SSD sigue creyendo
que estan ocupados y se le acaba el margen de escritura.

### ✅ TRIM, hecho (2026-08-17) -- y son DOS trabajos, no uno

Esta seccion llevaba las dos mitades metidas en la misma frase, y solo una era
la dificil:

```text
   decirle al disco que lo libre es libre    <- HECHO
   marcar lo alcanzable y soltar lo viejo    <- el recolector, sigue faltando
```

★★ **La primera no necesita a la segunda.** La cola libre del volumen es todo lo
que hay por encima de `log_head`, y ese puntero **solo avanza**: ahi no llega
ningun estrato, sin recorrer nada y sin marcar nada. Es la misma resta de la
contabilidad de aqui arriba, leida al reves.

Y hacia falta ya: sin ella el SSD sigue creyendo vivos --y copiando en cada
recogida interna suya-- **todos los bloques que el volumen no ha usado nunca**,
que recien formateado son casi todos.

Se pide desde la terminal del escritorio, y **obedece a la regla de esta
seccion**: `disco trim` ensena la propuesta --cuanto, desde que bloque, cuantas
ordenes-- y no manda nada; `disco trim ya` la manda. Politica, no automatismo:
no hay ningun demonio que recorte solo, y no va a haberlo.

El camino entero y sus cuatro guardianes estan en
`docs/componente/EL_DISCO_EXIGE.md`, seccion 12.1. Lo que importa aqui: **TRIM
pasa por las mismas puertas que escribir**, porque es igual de destructivo.

### ✅ La contabilidad, hecha (2026-07-31)

`bmo_estratos::espacio` -- `Ocupacion` y `Nivel`, con los cuatro umbrales de
arriba, **probados en el anfitrion**. El kernel los expone en `estratos`.

Y la cuenta resulto ser **una resta**: ESTRATOS reserva con un puntero que solo
avanza (`log_head` es el primer bloque libre), asi que todo lo de debajo esta
usado. Ni mapa de bits, ni listas de huecos, ni fragmentacion que medir -- y eso
es consecuencia directa de no sobreescribir nunca. El precio es que la cuenta
solo sube hasta que exista el recolector, y que suba **y se vea** es justo lo
que se quiere.

### El numero que zanja cuando hace falta el GC

Con 414 GiB y bloques de 4 KiB son ~108 millones de bloques. Un `.bex` de C
ocupa cinco. Aunque cada estrato guardara uno entero **sin compartir nada**,
caben **mas de veinte millones** antes de rozar el 70 % -- y como todo esta
direccionado por contenido, lo que no cambia no se copia, asi que el numero real
es mucho mayor.

Hay un test que lo comprueba (`en_414_gib_caben_millones_de_estratos`), y por eso
el orden de la section 10 no se toca: **el recolector va despues de escribir, no antes**.

Es la misma postura de Git y no por casualidad: `git gc` no borra tu historia,
solo lo que ya nadie alcanza, y el reflog guarda 90 dias por si acaso. Para quien
acumula a proposito, el historial **es el producto**. Lo que hace falta desde el
primer dia no es recoger: es **avisar**.

---

## 10. Orden de construccion

ESTRATOS no se empieza hasta que lo de abajo este firme. Cada paso deja algo
que funciona por si solo:

1. **FAT32 sobre A: (leer)** -- el disco ya se lee por sectores y la GPT esta
   parseada. Esto desbloquea la caja negra de CABINA y sacar los `.bex` de
   dentro del kernel. *No toca ESTRATOS.*
2. **Gate de identidad** -- `IDENTIFY` ya da modelo y serie; falta que sea una
   comprobacion de verdad. Sin esto no se escribe nada, en ningun sitio.
3. **Capa de bloques** -- el contrato unico `leer / escribir / capacidad /
   identidad`, con AHCI y NVMe debajo. ESTRATOS habla con eso, no con SATA.
4. ✅ **ESTRATOS solo lectura** -- formatear desde el anfitrion con una
   herramienta del toolchain, y que el kernel lo monte y lea. Sin riesgo:
   si el formato esta mal, se reformatea.
5. **ESTRATOS escritura** -- log, estratos, barreras. Aqui empieza lo serio.
   *(La contabilidad de espacio de la section 9 ya esta: sin saber cuanto queda no se
   puede decidir si se acepta una escritura.)*
6. **Recolector** -- cuando haya algo que recoger.
7. **TimeBack sobre ESTRATOS** -- el historial deja de ser una copia.

---

## 11. Lo que ESTRATOS NO va a ser

Mismo criterio que BMO C y BMO C++: **acotado a proposito, terminable**.

- No hay volumenes, RAID ni espejos.
- No hay compresion ni cifrado en la v1.
- No hay cuotas, ACLs ni usuarios: hay **capabilities**.
- No hay enlaces duros. Los simbolicos, quiza.
- No es POSIX y no lo intenta.
- No hay red.

Un sistema de ficheros que hace bien seis cosas es infinitamente mas util que
uno que hace treinta a medias -- y sobre todo, es uno que **se puede terminar**.

---

## 12. Riesgos, dichos antes

- **Aqui se pierden datos.** Es el componente donde un bug no da un fault
  bonito en pantalla: se lleva el trabajo de alguien. De ahi las reglas de la
  section 2 y el orden de la section 10.
- **El recolector es lo dificil**, no el formato.
- **Las barreras de escritura hay que respetarlas.** Un SSD que dice "ya esta"
  cuando el dato sigue en su cache convierte cualquier diseno transaccional en
  decoracion. `FLUSH CACHE` no es opcional.
- **Nunca dos escritores.** Mientras no haya SMP y bloqueo de verdad, ESTRATOS
  se monta desde un solo sitio.

---

*Documento de diseno. La implementacion empieza cuando los pasos 1 a 3 de la
section 10 esten hechos y probados en hardware real.*
