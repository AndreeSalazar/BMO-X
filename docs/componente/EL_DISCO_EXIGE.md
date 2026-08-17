# EL DISCO EXIGE

> Capitulo de componente **C6**, en la forma de `META-KERNEL_HARD.md`: no *"que
> hace BMO-X con el disco"* sino **que exige el disco de quien quiera
> escribirle**.
>
> Escrito el **2026-08-17**. Nace de una pregunta del dueno que separa el
> problema mejor de lo que estaba separado en el arbol:
>
> > *"la placa base entrega los perfiles, y segun el perfil se exprime todo el
> > potencial (...) pero primero es importante saber que reglas son los disco
> > duro: HDD, SSD y ranura. El mio es 480GB SSD, asi que es para perfilar QUE
> > VIENE."*
>
> Las tres cosas que nombra --medio, ranura y aparato-- **son tres preguntas
> distintas y el arbol las trataba como una**. Ese es el contenido del capitulo.

---

## 0. LA FRASE QUE ORDENABA EL DOCUMENTO -- ✅ CERRADA EL 17-08

**BMO-X no sabia si su disco giraba.** Nunca lo habia preguntado.

```
   lo que se le preguntaba          modelo (27..46), serie (10..19),
                                    sectores (100..103)

   lo que NO, y decide el diseno entero de la escritura
      palabra 217        ** ROTACIONAL O NO **   <- a una palabra de distancia
      palabra 169 bit 0  soporta TRIM
      palabras 106/209   sector FISICO y alineacion del LBA 0
      palabras 76 / 77   generacion SATA soportada / NEGOCIADA
      palabra 75         profundidad de cola
```

Y sin embargo el arbol **si tenia una opinion**: `ESTRATOS.md` habla de soltar
bloques *"o el SSD sigue creyendo"*, y la ley dice que un disco *"da caudal
cuando tiene cola"*. O sea que el diseno **daba por hecho** lo que el codigo no
habia comprobado. Es L5 al reves --*hardcodea contratos, pregunta hechos*-- y el
hecho estaba a una lectura de 16 bits **en un buffer que ya se pedia**.

### ✅ Lo que hay hoy, y donde vive

Las siete palabras se leen, y el reparto sigue L7 --cada generacion ignora para
que sirve la de arriba, que es lo que hace **falsable** cada afirmacion sobre
este disco:

```
   abuelo   bmo-identify::abuelo    la PALABRA n y el intercambio de bytes.
                                    No sabe que significa ninguna
   padre    bmo-identify::padre     Medio, Cola, Enlace, Geometria, Trim: una
                                    palabra, su sesgo y su guarda cada uno
   hijo     bmo-identify::hijo      Contraste: las restas entre dos del padre
   nieto    bmo-disco-juicio        el VEREDICTO y el PERFIL. En
                                    `platform/shared/`, con `cargo test` (L7b)
```

**45 pruebas de anfitrion**, y el kernel solo pega: `dev/disk/perfil.rs` toma la
foto en el arranque y la empaqueta en cuatro campos de `OP_INFO`
--`DISCO_MEDIO`, `DISCO_ENLACE`, `DISCO_GEOMETRIA` y `DISCO_JUICIO`-- que
`info` pinta en la seccion `disco`.

★ **Los tres primeros son HECHOS y el cuarto es el VEREDICTO, separados a
proposito**: asi se puede estar en desacuerdo con la conclusion sin perder la
evidencia. Un veredicto que aparece sin lo que lo sostiene no se puede discutir,
solo creer.

[!] **Nada de esto ha tocado un CPU todavia.** Compila, pasa sus pruebas y esta
razonado. Lo que la primera tanda contesta esta en la section 10.

---

## 1. ★★ TRES PREGUNTAS, NO UNA

Lo que el dueno separo, y hay que mantener separado porque **se contradicen**:

```
   EL MEDIO      gira o no gira        decide QUE es caro
   LA RANURA     por donde se habla    decide CUANTO puede haber en vuelo
   EL APARATO    que trae dentro       decide si la teoria del medio se cumple
```

**Ninguna de las tres implica a las otras.** Un SSD detras de una ranura de HDD
--que es exactamente esta maquina-- tiene la fisica de un SSD y el techo de
concurrencia de un disco de 1998. Y un SSD barato sin DRAM se comporta en
lecturas aleatorias mucho peor de lo que su medio permitiria.

Tratar las tres como "el disco" es lo que produce la frase *"los SSD son
rapidos"*, que no predice nada.

---

## 2. EL MEDIO: QUE EXIGE UN HDD

**PARA QUE EXISTE.** Es un brazo mecanico sobre un plato que gira. Todo lo que
cuesta caro en un HDD cuesta caro por eso, y por nada mas.

```
   [SPEC]        sector                          512 B logicos
   [LITERATURA]  busqueda (seek) media           ~9-12 ms
   [LITERATURA]  latencia rotacional a 7200 rpm  ~4,2 ms (media vuelta)
   [LITERATURA]  secuencial sostenido            ~100-200 MB/s
   [ARITMETICA]  una busqueda = ~13 ms = ~59 MILLONES de ciclos a 4,5 GHz
```

★★ **Esa ultima linea es la unica que hay que recordar.** Una busqueda de
cabezal cuesta **sesenta y dos mil puertas de BMO-X** (945 ciclos cada una). No
hay optimizacion de software que compre eso: en un HDD, el orden en que se piden
los sectores importa mil veces mas que el codigo que los pide.

**Lo que un HDD exige, en tres frases:**

- **Contiguidad, o nada.** Leer 1 MB seguido y leer 1 MB en 256 trozos
  desperdigados **no son la misma operacion**: la segunda puede costar 200 veces
  mas. Por eso los FS clasicos se desfragmentan.
- **No hay desgaste por escritura.** Un HDD se puede reescribir en el mismo sitio
  sin coste extra, indefinidamente. **TRIM no existe y no hace falta.**
- **La cola sirve, pero poco.** NCQ deja al disco reordenar las peticiones para
  hacer menos recorrido con el brazo (el "ascensor"). Gana del orden de 2x en
  aleatorio, no 10x.

---

## 3. EL MEDIO: QUE EXIGE UN SSD -- y no es "lo mismo pero rapido"

**PARA QUE EXISTE.** No hay partes moviles: hay celdas NAND. Y la NAND tiene una
asimetria que no tiene ningun otro componente de la caja.

```
   [SPEC]        se LEE por pagina         4-16 KB
   [SPEC]        se ESCRIBE por pagina     4-16 KB
   [SPEC]        se BORRA por BLOQUE       ** 256 KB - 4 MB **
   [SPEC]        una pagina escrita NO se puede reescribir: hay que borrar
                 el bloque ENTERO que la contiene
```

★★ **Esa es la exigencia entera del componente**, y de ella salen todas las
demas. Modificar 4 KB en el sitio significa: leer el bloque de borrado entero,
cambiarle 4 KB, borrarlo y reescribirlo. **Eso es la amplificacion de
escritura**, y es la razon de que exista un FTL --una capa dentro del disco que
miente sobre donde estan las cosas para no tener que hacer eso.

**Las cuatro consecuencias, cada una con su regla:**

- **Escribir en el sitio es caro; escribir hacia adelante es gratis.** Un diseno
  que nunca sobrescribe le ahorra al FTL su trabajo mas caro.
- **Hay un numero de escrituras y se acaba.** Se declara como TBW (terabytes
  escritos). No es una alarma lejana si el sistema reescribe metadatos a lo
  tonto.
- **★ El disco no sabe que has borrado un fichero.** Para el, un bloque que el FS
  ya no usa sigue conteniendo datos validos, y lo sigue copiando en cada
  recoleccion interna. **TRIM es la unica forma de decirselo**, y sin el, el
  rendimiento cae segun se llena aunque el FS diga que hay sitio.
- **La velocidad anunciada es la de la cache.** Los SSD de consumo escriben
  primero en una zona de SLC rapida y la vacian despues. Sostenido, un disco de
  gama de entrada puede caer a **una fraccion** de su cifra de catalogo.

### ★ Y aqui hay una coincidencia que conviene decir en voz alta

**ESTRATOS ya esta disenado para lo que un SSD quiere.** Copy-on-write, log que
solo crece hacia adelante, nunca sobrescribir: es exactamente lo que reduce la
amplificacion de escritura. No fue por eso --se eligio por el historial-- pero el
resultado es que el FS de esta casa **le pide a la NAND justo lo que la NAND hace
barato**.

[!] Con dos condiciones que hoy no se cumplen, y son las de la section 7.

---

## 4. LA RANURA: donde esta el techo de ESTA maquina

La ranura no cambia lo que cuesta un acceso: cambia **cuantos puede haber en
vuelo a la vez**. Y en un SSD eso es casi todo el rendimiento.

```
                       ancho              en vuelo a la vez
   SATA III / AHCI     6 Gb/s ~ 550 MB/s  1 cola x 32 comandos
   NVMe / PCIe 4.0 x4  ~7,8 GB/s          65.535 colas x 65.536 comandos
```

★★ **La diferencia que importa no es el ancho: es la segunda columna.** AHCI se
diseno para un aparato con un brazo, donde no tiene sentido pedir mil cosas a la
vez. NVMe se diseno para un aparato que puede atender cientos en paralelo. Poner
un SSD detras de AHCI le deja la fisica y le quita el paralelismo.

### Lo que esta maquina tiene, MEDIDO

```
   [MEDIDO]   HBA AHCI, CAP = 0xEF36FF27
   [MEDIDO]   el disco de BMO: Kingston SA400S37480G, SATA, 447 GiB
   [HECHO]    el NVMe de esta maquina es el Windows del dueno: NO se escribe
   [HECHO]    ** el driver usa la RANURA 0, SIEMPRE ** -- 1 de 32
```

Esa ultima esta escrita en el propio driver, con su motivo: *"un comando en vuelo
es un estado global del puerto (...) dos lecturas solapadas escriben la misma
ranura"*. La solucion que se puso fue **un dueno**, no una cola -- correcta para
la correccion, y deja el caudal donde estaba.

> ★★ **BMO-X corre hoy su SSD a profundidad de cola 1, que es la unica
> configuracion en la que un SSD se parece a un disco duro.** No es un defecto
> del diseno: es una casilla que nadie ha abierto porque hasta ahora no habia
> nada que escribiera.

Y la ley ya lo decia sin numero: *"un disco da caudal cuando tiene cola. Una
peticion en vuelo desperdicia el aparato"*. Es L0 --una regla escrita sin juez--
y este capitulo le pone el numero al lado.

---

## 5. EL APARATO: lo que la ranura y el medio no dicen

Dos SSD SATA de 480 GB pueden diferir en un factor de diez en escritura aleatoria
sostenida. La diferencia esta dentro, y **el disco no la declara**.

### El de esta maquina: Kingston SA400S37480G

```
   [MEDIDO]     modelo y serie, del IDENTIFY
   [MEDIDO]     capacidad: 447 GiB
   [CATALOGO]   TBW 160 TB
   [CATALOGO]   secuencial ~500 / ~450 MB/s (lectura / escritura)
   [CATALOGO]   ** SIN DRAM ** -- la tabla del FTL no cabe en cache propia
   [CATALOGO]   ** SIN proteccion ante corte de corriente (sin condensadores) **
```

[!] **Los cuatro `[CATALOGO]` son de la ficha del fabricante y NO estan medidos
aqui.** La sonda que los convierte en `[MEDIDO]` esta en la section 8.

**Por que esas dos ultimas lineas mandan mas que las cifras de MB/s:**

**Sin DRAM.** El FTL --el mapa de "donde esta de verdad cada LBA"-- vive en la
NAND en vez de en una cache propia. Cada lectura aleatoria a una zona fria puede
costar **dos accesos**: uno para el mapa y otro para el dato. Por eso este disco
cae mucho mas de lo que su medio justificaria en accesos dispersos -- y por eso
el descenso por el arbol de ESTRATOS, que es punteros persiguiendo punteros, es
el patron que peor le sienta.

**Sin proteccion de corte.** El disco confirma una escritura cuando la tiene en
su cache volatil, no cuando esta en la NAND.

> ★★★ **Eso convierte el `FLUSH CACHE` de ESTRATOS en la unica cosa que separa
> una transaccion de la corrupcion.** El diseno ya lo dice --*"un SSD que dice
> ya esta cuando el dato sigue en su cache convierte cualquier diseno
> transaccional en decoracion"*-- y aqui queda dicho de quien se sospecha y por
> que: de este disco, porque no tiene condensadores para terminar lo que
> empezo.

---

## 6. LAS REGLAS

Las cinco de la ley (`R-DISCO1..5`) siguen enteras. Estas son las que este
capitulo anade:

- **R-DISCO6.** ★★ **EL MEDIO SE PREGUNTA, NO SE SUPONE.** La palabra 217 del
  IDENTIFY dice si el medio gira (`0x0001` = no rotacional; `0x0401`-`0xFFFE` =
  las RPM). Ningun camino de BMO-X puede optimizar "para SSD" ni "para HDD"
  antes de leerla. Es L5 aplicada al almacenamiento, y hoy esta incumplida: el
  arbol razona sobre TRIM y sobre colas **sin haber leido nunca ese campo**.

- **R-DISCO7.** ★ **MEDIO, RANURA Y APARATO SON TRES EJES Y SE NOMBRAN POR
  SEPARADO.** *"El disco es lento"* no es un diagnostico. Lo es *"la ranura no
  deja mas de un comando en vuelo"*, que se arregla en el driver, o *"el medio
  paga un borrado por cada escritura"*, que se arregla en el formato. Confundir
  los dos manda a optimizar en la capa equivocada.

- **R-DISCO8.** ★★ **LO QUE DECIDE EL DISENO ES JUSTO LO QUE EL DISCO NO
  DECLARA.** El tamano del **bloque de borrado** --el numero que dice si una
  escritura de 4 KB cuesta 4 KB o 2 MB-- **no lo expone ningun SSD de consumo**,
  en ninguna palabra del IDENTIFY. Tampoco el TBW, ni si hay DRAM, ni si hay
  condensadores. Por eso hace falta un PERFIL: no para repetir lo que el aparato
  ya dice, sino **para escribir lo que el aparato calla**.

- **R-DISCO9.** ★ **UNA CIFRA DE CATALOGO NO ES UNA MEDIDA, Y EN UN SSD MIENTE
  DOS VECES.** Miente por ser de fuera (L2) y miente por describir la rafaga en
  la cache SLC en vez del regimen sostenido. Todo `[CATALOGO]` de un disco lleva
  al lado **la ventana en que dejaria de serlo**: cuantos GB seguidos y a que
  profundidad de cola.

- **R-DISCO10.** ★ **SIN TRIM, EL RECOLECTOR DEL FS TRABAJA PARA NADA.** Soltar
  un bloque en ESTRATOS lo libera *para el FS*; el disco sigue creyendolo vivo y
  lo sigue copiando en cada recoleccion interna. El recolector de la section 9 de
  `ESTRATOS.md` **no esta completo sin la orden al aparato**, y si la palabra 169
  dice que no hay TRIM, eso hay que **decirlo**, no callarlo.

---

## 7. LO QUE ESTO LE EXIGE A ESTRATOS 1.0

1.0 es *"escribir contenido desde Ring 3 y releerlo tras reiniciar"*. Este
capitulo no lo cambia -- le pone tres condiciones que hoy no se cumplen:

```
   1  LEER LA PALABRA 217 ANTES DE ESCRIBIR NADA
      Es una lectura de 16 bits en un buffer que YA se pide. Sin ella, el paso 5
      se disena a ciegas.

   2  ALINEAR EL LOG CON EL BLOQUE DE BORRADO
      El log de ESTRATOS crece hacia adelante, que es lo correcto. Si ademas su
      frente cae en frontera de bloque de borrado, la amplificacion tiende a 1.
      Si no, cada avance puede tocar dos bloques.
      [!] El tamano no se puede preguntar (R-DISCO8) -> va al perfil.

   3  EL FLUSH CACHE ES LA UNICA RED
      Ya esta puesto y en el orden correcto. Lo que falta es la prueba, y es la
      misma que ya estaba pendiente: ** reiniciar y ver si sigue en generacion
      3 **. En un disco sin condensadores, esa prueba no es una formalidad.
```

★ Y una que **no** es condicion de 1.0 pero decide su rendimiento: la
profundidad de cola. 1.0 puede nacer con la ranura 0 --correccion primero-- y
entonces la cifra que salga **describe a AHCI usado como en 1998**, no a este
disco. Hay que decirlo cuando se publique, o se convierte en la linea base
equivocada.

---

## 8. EL PERFIL: lo que pidio el dueno, y por que encaja sin torcer nada

> *"la placa base entrega los perfiles, y segun el perfil se exprime todo el
> potencial"*

La mitad ya existe y **para el CPU**: `cpu_vendor/ryzen_5_5600x/` declara
identidad, topologia, TSC, errata y presupuesto, y su primera linea dice que
cambiar de CPU es cambiar de perfil y **nunca editar el kernel** (R-CPU8). El
almacenamiento no tiene su equivalente, y lo pide por la misma razon.

### La doctrina se hereda entera

```
   se PREGUNTA al aparato        rotacional, TRIM, sector fisico, cola,
   (y manda sobre el perfil)     generacion SATA, capacidad, identidad

   se DECLARA en el perfil       bloque de borrado, TBW, DRAM si/no,
   (porque nadie lo expone)      condensadores si/no, sostenido real
```

★★ **La linea que separa las dos columnas no es una preferencia: es lo que el
aparato responde y lo que no.** Un perfil que declarara la capacidad seria un
perfil que puede mentir sobre algo comprobable. Un perfil que declara el bloque
de borrado esta diciendo lo unico que nadie mas puede decir.

### Y el seguro es el mismo que el del CPU (R-CPU8, R-CPU9)

Si el `IDENTIFY` no coincide con el perfil --otro modelo, otra serie, otra
capacidad-- **el perfil no opina**: sus campos contestan `sin declarar` y
cualquier decision que dependiera de ellos toma el camino conservador. Y el "no
coincide" **lleva los dos lados**, lo esperado y lo leido, para que arreglarlo
sea cambiar una cifra y no leer codigo.

Estrenar un disco nuevo = copiar el perfil, arrancar (dira `SIN PERFIL`, que es
lo correcto), correr la sonda y pegar las cifras. **Cero lineas de kernel.**

### ★ La sonda que convierte los `[CATALOGO]` en `[MEDIDO]`

Cuatro numeros y no hace falta hardware nuevo. Se miden con el metro que ya
existe, declarando la ventana:

```
   secuencial sostenido   escribir 8 GB seguidos y dar la curva, no la media
                          -> donde cae, ahi se acabo la cache SLC
   aleatorio 4 KB         a profundidad 1 (lo de hoy) y a 32 (con NCQ)
                          -> la resta ES el precio de la ranura 0
   lectura fria           el patron del descenso por el arbol
                          -> es donde se paga el "sin DRAM"
   el bloque de borrado   escribir 4 KB con paso creciente (64K, 128K...
                          4M) y ver donde deja de doler
                          -> ** se DEDUCE, porque no se puede preguntar **
```

La ultima es la interesante: el unico numero que el perfil tiene que declarar
obligatoriamente es tambien el unico que se puede **cazar por su sombra**.

---

## 9. LO QUE ESTE CAPITULO NO CUBRE, Y POR QUE

- **NVMe.** No hay driver, y en esta maquina el NVMe es el Windows del dueno: la
  escritura esta cerrada a proposito. Cuando haya otro disco NVMe, la section 4
  crece con su columna -- las colas cambian el analisis entero, no un parametro.
- **SMART.** Es la via para leer horas encendido, sectores realojados y **el TBW
  consumido de verdad** en vez del de catalogo. Pendiente y nombrado.
- **RAID, cifrado, compresion.** Fuera por la regla de la esencia acotada, igual
  que en `ESTRATOS.md` section 11.

---

## 10. ★ LA PRIMERA TANDA: que contesta, y que descarta cada respuesta

Arrancar y escribir `info`, seccion `disco`. Cinco lineas, y **cada una descarta
algo distinto** -- ninguna es decorativa.

```text
   medio     ESTADO SOLIDO            lo esperado. Confirma la palabra 217
             ROTACIONAL, n rpm        ** el disco de BMO no es el que creemos
             el disco NO DICE si gira  R-DISCO6 se cumple igual: no se asume.
                                       Pasa en SSD tempranos, y entonces hace
                                       falta la prueba de Windows 7 -- medir
             valor RESERVADO           el disco dice algo fuera de la spec

   cable     Gen3 soportado / Gen3 negociado    lo esperado
             ... / Gen2 negociado  POR DEBAJO   ** es el CABLE o el puerto,
                                                no el disco. Se arregla con la
                                                mano, no con codigo

   cola      el disco admite 32, BMO usa 1  ->  31 RANURAS PARADAS
             ** Si dice otra cosa que 32, el techo de la tanda de escritura
             es otro y hay que rehacer la cuenta

   sector    1 logico por fisico = 512 B    un disco clasico
             8 logicos por fisico = 4096 B  ** ENTONCES LA ALINEACION IMPORTA,
             + LBA 0 desplazado n           y el aviso de abajo tiene que salir

   perfil    reconocido (cifras de CATALOGO)   lo esperado hoy
             SIN PERFIL                        ** la identidad no cuadra: la
                                               linea trae lo esperado y lo
                                               leido, y se arregla cambiando
                                               una cifra en `perfil.rs`
```

★★ **Y las dos que tienen que salir en rojo, porque son verdad:**

```text
   trim      si                       (y aun asi el recolector no lo manda)
   barrera   el FLUSH CACHE es LO UNICO: este disco no termina lo que empezo
```

La segunda **no es un fallo que arreglar**: es la ficha de este aparato dicha en
voz alta, y tiene que estar delante el dia que ESTRATOS escriba contenido. Si
algun dia sale la otra frase sin cambiar de disco, el que miente es el perfil.

[!] Lo que esta tanda **no** contesta: ninguna cifra de rendimiento. Las cuatro
del perfil siguen siendo `[CATALOGO]` hasta que corra la sonda de la section 8 --
y `info` lo dice al lado del perfil en vez de callarlo.

---

## 11. EL PRECIO

De C6 en la ley, y sigue siendo el mejor resumen de por que este componente no
se razona: `PI` declaraba los puertos 0,1,4,5 y el disco estaba en el 2. Y la
suma `+ part_lba` faltaba en los tres sitios del camino rapido, asi que un `.bex`
se leia de dentro de la ESP -- codigo x86-64 real y ajeno. Dos dias, y la firma
era *"el directorio se lee bien y el contenido no"*.

Y el precio propio de este capitulo, que todavia no se ha pagado y por eso se
escribe antes: **el arbol lleva semanas razonando sobre TRIM, sobre colas y sobre
SSD sin haber leido nunca la palabra que dice si el disco gira.** Ninguna de esas
frases era falsa. Ninguna estaba comprobada.
