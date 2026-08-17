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

✅ **Y ya corrio en el Ryzen el mismo dia**: las siete palabras se leen bien y
las tres restas cuadran. La foto y lo que convierte en `[MEDIDO]` estan en la
section 10.

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

- **R-DISCO11.** ★★ **UNA CAPACIDAD DECLARADA QUE NADIE EJERCITA NO ES UNA
  CAPACIDAD: ES UNA AFIRMACION SIN PROBAR.** Un campo del hardware es tan fiable
  como el camino que lo recorre, y para todo disco del mundo ese camino es el
  arranque del sistema mayoritario. De ahi que **cuanto mas NUEVO es un campo,
  menos se puede creer** -- la palabra 217 no fallaba por dificil, fallaba por
  recien nacida. Y de ahi la segunda mitad: **la unidad de confianza no es el
  aparato, es la terna aparato + ranura + driver**, porque el mismo disco con el
  mismo firmware corrompio datos en AMD y no en Intel. Desarrollo en la
  section 11.

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

## 10. ✅ LA PRIMERA TANDA YA CONTESTO (2026-08-17, `SALIDA.TXT`)

```text
   medio      ESTADO SOLIDO -- no paga busqueda de cabezal
   cable      SATA Gen3 soportado / Gen3 negociado
   cola       el disco admite 32, BMO usa 1   31 RANURAS PARADAS
   sector     1 logicos por fisico = 512 B
   perfil     reconocido   (sus cifras son de CATALOGO, no medidas)
   trim       si
   barrera    el FLUSH CACHE es LO UNICO: este disco no termina lo que empezo
   alinear a  2048 KiB   (declarado por el perfil, no leido)
```

**Las siete palabras se leen bien y las tres restas cuadran.** Lo que esta tanda
convierte de razonado a `[MEDIDO]`:

- **La palabra 217 dice `0001h`.** Por primera vez, BMO-X SABE que su disco no
  gira. La frase que abria este capitulo queda cerrada.
- **El sesgo de la 75 funciona**: el disco escribe `31` y el informe dice `32`.
  Leerlo crudo habria dado 31 para siempre sin que nada fallara.
- **El cable va al maximo**: la 76 y la 77 se leen las dos y **coinciden**, asi
  que el aviso `POR DEBAJO` no salta -- que es el caso en que la separacion
  soportado/negociado no se nota. La proxima vez que se note, sera verdad.
- **La identidad cuadra**: modelo y sectores dentro del 1%, sin `SIN PERFIL`.

### ★★★ Y la linea que demuestra R-DISCO8 sola, sin argumentar

```text
   sector      1 logicos por fisico = 512 B      <- lo que el disco DECLARA
   alinear a   2048 KiB  (declarado por el perfil, no leido)
```

**Dos lineas seguidas, y se contradicen en apariencia.** No hay contradiccion:
la palabra 106 es de la epoca del *Advanced Format* y describe el sector logico
contra el fisico de un PLATO. En un SSD ese numero es verdad --el disco
direcciona de 512 en 512-- y **no dice absolutamente nada de la NAND**: ni la
pagina, ni el bloque de borrado, que es la unica frontera que importa al
escribir.

O sea que la geometria que el aparato SI declara es la que no sirve, y la que
decide el diseno **no tiene campo donde vivir**. Eso era R-DISCO8 escrito como
prediccion; ahora esta impreso en pantalla, en dos renglones consecutivos.

### [!] Y un defecto que la tanda destapo, y era mio

`info` saco **dos secciones tituladas `disco`**, con el teclado en medio: la de
estado (listo / montado) y la de identidad. Un nombre repetido no identifica
nada -- el mismo fallo que esta casa persigue en los ficheros, cometido en un
informe. Juntadas: una seccion, el estado arriba y el aparato debajo.

---

### Lo que descartaba cada rama (se conserva: la proxima tanda vuelve a usarlo)

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

> **La primera dejo de ser verdad el 2026-08-17 por la tarde**, y se deja escrita
> arriba porque es el antes de este capitulo. Ahora el sistema **sabe mandar la
> orden** y hay un sitio desde donde pedirla: `disco trim` en la terminal del
> escritorio. Lo que sigue faltando es el recolector -- ver la seccion 12.1, que
> separa las dos mitades que se estaban confundiendo en una sola frase.

La segunda **no es un fallo que arreglar**: es la ficha de este aparato dicha en
voz alta, y tiene que estar delante el dia que ESTRATOS escriba contenido. Si
algun dia sale la otra frase sin cambiar de disco, el que miente es el perfil.

[!] Lo que esta tanda **no** contesta: ninguna cifra de rendimiento. Las cuatro
del perfil siguen siendo `[CATALOGO]` hasta que corra la sonda de la section 8 --
y `info` lo dice al lado del perfil en vez de callarlo.

---

## 11. ★★★ POR QUE PASAN ESAS TRAMPAS -- la capa que hay debajo

Windows no se fio de la palabra 217. Linux tuvo que prohibir el TRIM encolado en
una familia entera de discos. **Las dos historias parecen "hay firmware malo", y
esa lectura no sirve para nada**: no dice cual creer manana.

Debajo hay algo que si predice, y son tres escalones.

### 1. ATA es un contrato que nadie verifica

No existe una certificacion que un disco tenga que pasar campo por campo antes
de venderse. **Un disco sale a la calle cuando arranca Windows y da buena cifra
en un banco de pruebas.** Eso es todo lo que su firmware tiene que sobrevivir.

### 2. ** Por tanto: un campo es tan fiable como el camino que lo ejercita

Y ese camino, para todo disco del mundo, es el arranque del sistema mayoritario.

```
   capacidad, modelo, LBA48   los lee Windows en CADA arranque   -> de fiar
   palabra 217 en 2008        no la leia NADIE todavia           -> una PROMESA
   TRIM dentro de NCQ         solo Linux lo apretaba de verdad   -> ahi vivio
                                                                    el bug
```

★★ **De ahi sale la regla, y es contraintuitiva: cuanto mas NUEVO es un campo,
menos se ha ejercitado, y menos se puede creer.** La 217 no fallaba por ser
dificil: fallaba **por ser nueva**. Windows 7 no desconfiaba de los fabricantes,
desconfiaba de un campo recien nacido -- y por eso lo cruzo con una medida propia
en vez de discutirlo.

Va como **R-DISCO11**: *una capacidad declarada que nadie ejercita no es una
capacidad, es una afirmacion sin probar.* Es L4 --*"un guardian que nunca ha
rechazado nada no esta probado"*-- dicha desde el otro lado del cable.

### 3. ★★ Y el escalon que cambia el DISENO: la unidad de confianza no es el
### aparato

El caso Samsung lo demuestra y es la parte que mas cuesta ver: **el mismo disco,
con el mismo firmware, iba bien en controladores Intel y corrompia datos en
AMD.** El fallo no estaba en el disco. Tampoco en el controlador. Estaba en la
COMBINACION.

```
   lo que o funciona o no funciona NO es el aparato:
   es la TERNA   aparato + ranura + camino del driver
```

Es R-DISCO7 --medio, ranura y aparato son tres ejes-- con una vuelta mas: **la
terna tiene propiedades que ninguno de los tres tiene por separado**, y por eso
no se pueden heredar de una ficha tecnica.

[!] **Consecuencia directa sobre el perfil que este arbol acaba de escribir**:
`Identidad` hoy son modelo y capacidad, o sea el APARATO. Mientras todos los
caminos rapidos esten apagados no muerde. **El dia que un perfil autorice algo
--encender la cola, mandar TRIM encolado-- esa autorizacion solo vale para la
terna en la que se probo**, y la identidad tiene que nombrar tambien al
controlador. Escrito antes de que haga falta, que es cuando sale barato.

---

## 12. ★★ QUE PUEDE APROVECHAR BMO-X QUE LOS OTROS NO

La pregunta del dueno --*"que BMO-X USE TODO lo que ofrece TODO"*-- tiene una
respuesta concreta, y sale justo de la section anterior.

### La asimetria, en una linea

```
   Windows y Linux tienen que acertar en TODOS los discos que existen.
   BMO-X tiene que acertar en UNO.
```

**Un sistema general paga la generalidad con pesimismo.** Linux prohibe el TRIM
encolado en toda la serie Samsung 800 no porque el tuyo falle, sino **porque no
puede probar el tuyo**. Windows 7 corria un banco de pruebas porque no podia
fiarse de la flota.

BMO-X puede pagar la especificidad con **una medida, una vez**, anotada en el
perfil con su origen. Eso es exactamente lo que ya hace `cpu_vendor/` con el
presupuesto de ciclos; el perfil de disco es la misma idea en otro componente.

> ★★★ **El general ASUME. El perfilado MIDE, y desbloquea solo lo medido.**
>
> Y por eso el perfil lleva `Origen` pegado a cada cifra: lo que esta en
> `Catalogo` es una asuncion heredada y no desbloquea nada; lo que esta en
> `Medido` es lo unico que puede autorizar un camino rapido.

### Lo que hay sobre la mesa HOY, medido, y en su orden

**1. ✅ TRIM, y va PRIMERO -- por una razon que sale de la historia.**

La lista negra de Linux era de **`NO_NCQ_TRIM`**: el TRIM *encolado*. El TRIM
normal --`DATA SET MANAGEMENT` a secas, sin NCQ-- **no tiene historial de
corrupcion**.

★★ Y BMO-X esta hoy en profundidad de cola 1. O sea que **esta exactamente en la
unica configuracion donde el TRIM es la variante segura**, y lo esta por accidente
--porque todavia no encola--. Ir a por el TRIM ANTES que a por la cola no es
conservador: es aprovechar la posicion en la que ya se esta.

Sin el, el recolector de la section 9 de ESTRATOS libera bloques para el sistema
de ficheros y **el disco los sigue creyendo vivos y copiandolos** (R-DISCO10).

**2. ⏳ Las 31 ranuras paradas.** Es lo mas grande que hay sin usar, y lo que mas
cambiaria una tanda de escritura. Pero encenderla mueve la terna entera a un
sitio donde hay muertos documentados **en placas AMD**. Va con interruptor
propio, prueba propia y en su commit -- nunca de refilon con otra cosa.

**3. ⏳ Medir lo que hoy es `[CATALOGO]`.** Cuatro cifras y una tarde
(section 8). Mientras sigan siendo de catalogo, **el perfil no puede autorizar
nada**, y el propio `info` lo dice al lado del perfil en vez de callarlo.

**4. ✅ Ya aprovechado, y conviene decirlo**: el cable va a Gen3 y el medio es
solido. Ninguno de los dos es un cuello hoy, y saberlo **tacha dos sospechosos**
antes de la primera medida de escritura. Eso tambien es usar lo que el aparato
ofrece: usar su respuesta para no buscar donde no hay nada.

---

## 12.1 TRIM, HECHO (2026-08-17) -- y las DOS mitades que se confundian

El punto 1 de arriba ya no esta sobre la mesa: esta puesto. Lo que conviene
dejar escrito es **que se hizo exactamente**, porque la frase *"falta TRIM"*
tapaba dos trabajos distintos y solo uno era el dificil.

```text
   decirle al disco que lo libre es libre    <- ESTO, y ya esta
   marcar lo alcanzable y soltar lo viejo    <- el RECOLECTOR, y sigue faltando
```

★★ **La primera mitad no necesita a la segunda**, y esa es toda la noticia. La
cola libre de un volumen ESTRATOS es *todo lo que hay por encima de `log_head`*,
y `log_head` es un puntero que **solo avanza**: no hay que recorrer nada ni
marcar nada para saber que ahi no llega ningun estrato. Es una resta, la misma
que ya hacia la contabilidad de la seccion 9.

Y hacia falta desde el primer dia: sin esto el SSD sigue creyendo vivos --y
copiando en cada recogida interna suya-- **todos los bloques que este volumen no
ha usado nunca**, que en 414 GiB son casi todos.

### El reparto, que es el de siempre

```text
   bmo-trim         el FORMATO: rango de LBA -> descriptores    9 casillas
   bmo-ahci         el COMANDO: DATA SET MANAGEMENT + features
   dev/disk/trim.rs el PEGAMENTO y los guardianes
   fsys/estratos    QUE rango: la cola libre, de `log_head`
   commands/disco.rs   la terminal: propone, y obedece si le dicen `ya`
```

El empaquetado vive fuera del kernel por lo mismo que `bmo-identify`: **un
descriptor mal armado no da un fallo, hace que el disco olvide sectores que si
importaban**, y eso se prueba con `cargo test` -- no flasheando.

### Los cuatro guardianes, en orden

TRIM **es destructivo**, asi que pasa por las mismas puertas que escribir y por
una propia. Un TRIM sobre la ESP se lleva el `BOOTX64.EFI` igual de bien que una
escritura, y en esta maquina esa particion tambien lleva el cargador de Windows.

```text
   hay disco?                  si no, no hay nada que recortar
   lo declara la palabra 169?  no se manda "a ver si suena"
   esta armada la escritura?   el gate de identidad, el mismo de write
   cae dentro de una ventana?  ** el rango ENTERO, no tanda a tanda
```

★ El cuarto tiene su propia casilla y merece decirse: la cola libre son cientos
de millones de sectores y no cabe en el `u16` de una escritura, asi que la
tentacion era preguntar tanda a tanda. **Cada tanda habria caido dentro y nadie
habria mirado el final del rango.** Se ensancho el contador, no la ventana.

### Y lo que sigue faltando, dicho

- El **recolector** de la seccion 9 (marcar y soltar versiones viejas). Sigue
  siendo post-1.0 y sigue sin correr prisa: caben mas de veinte millones de
  estratos antes del 70 %.
- Un recorte **automatico**. No lo va a haber: la seccion 9 dice *politica, no
  automatismo*, y aqui lo pide una persona escribiendo `disco trim ya`.
- La prueba **en el Ryzen**. Esto compila y **ningun CPU lo ha ejecutado**: la
  comprobacion es `disco trim`, leer la propuesta, `disco trim ya`, y que
  `disco` diga despues cuantos sectores se devolvieron y en cuantas ordenes. Va
  a la lista de `docs/metal/PRUEBA_EN_METAL.md`.

---

## 13. EL PRECIO

De C6 en la ley, y sigue siendo el mejor resumen de por que este componente no
se razona: `PI` declaraba los puertos 0,1,4,5 y el disco estaba en el 2. Y la
suma `+ part_lba` faltaba en los tres sitios del camino rapido, asi que un `.bex`
se leia de dentro de la ESP -- codigo x86-64 real y ajeno. Dos dias, y la firma
era *"el directorio se lee bien y el contenido no"*.

Y el precio propio de este capitulo, que todavia no se ha pagado y por eso se
escribe antes: **el arbol lleva semanas razonando sobre TRIM, sobre colas y sobre
SSD sin haber leido nunca la palabra que dice si el disco gira.** Ninguna de esas
frases era falsa. Ninguna estaba comprobada.
