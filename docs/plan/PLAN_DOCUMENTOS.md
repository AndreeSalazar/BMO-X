# PLAN DE LOS DOCUMENTOS -- el escritorio deja de listar PROGRAMAS y lista lo que abres

> Estado: **IDEA DEL DUENO, sin decidir a proposito.** Lo que hay aqui es el
> terreno medido -- que existe ya, que falta de verdad, y que cuesta cada
> camino. La eleccion no esta tomada porque el dueno pidio pensarla mas, y un
> plan que decide por el es un plan que le quita la decision.
>
> ★★ **Y la seccion 3b cambio el plan el mismo dia.** El dueno corrigio su idea
> --`.window` para lo que muestra pantalla, `.data` para los datos-- y al ir a
> medirla salio que **eso ya existe dentro del `.bex`**: cuatro secciones del
> BEF, con `CLASE_PANTALLA` escrita en cada binario desde el 10-08. Asi que la
> pregunta deja de ser *que extension* y pasa a ser *por que nadie lo ENSENA*.

## 0. De donde sale

El 2026-09-11, al entregarle `texto.bex` --el bloc de notas-- y explicarle que
para darle icono habria que meterlo en `apps\`, que significa *"lo que viene de
fuera del repo"*, el dueno contesto dos cosas. Las dos son mejores que la
pregunta:

> *"si `.bex` es confuso es mejor eso que el `.datex` = que es data en BMO-X que
> da todas la informacion necesaria como doom en `.wad`, por eso `.datex` ese
> mismo lleva lo necesario y abre el `.txt` en vez de bex"*

> *"para no confundir es mejor que sea una carpeta con que es **Formato texto**,
> lo mismo como PNG, JPG etc..."*

Y al aclararlo: **`.datex` es para que el `.bex` solo EJECUTE**, nada mas. O sea
el reparto de DOOM: el motor por un lado, los datos por otro.

Lo que las dos frases piden junto es un cambio de sujeto. Hoy el escritorio
ensena **programas** y tu tienes que saber cual abre lo tuyo. Lo que el dueno
quiere ensenar son **cosas suyas**, y que el sistema sepa quien las abre.

---

## 1. LO QUE YA ESTA PAGADO, y es mas de lo que parecia

Esto no es una lista de deseos: cada fila se comprobo leyendo el codigo antes de
escribir este documento.

```text
   el gesto ya existe        el panel de datos ya lista el volumen y ya ABRE un
                             `.txt` al doble clic, en un visor de solo lectura
                             con tope de 64 KiB -- `scene/data/visor.rs`

   el contenedor ya existe   un `.datex` seria un BEF **sin codigo**: solo la
                             seccion Resources (0x0B). `bmo-pack` ya la escribe,
                             `paquete.h` ya la lee EN EJECUCION --`caja.bex` lo
                             hace-- y el DIRECTOR ya parsea recursos BEF: es con
                             lo que saca los iconos (`read_icon` en
                             `scene/launcher.rs`)

   el icono dentro del       `BICO` como recurso del paquete. Sin `.lnk`, sin
   fichero                   cache de iconos, sin fichero de escritorio huerfano

   media app ya existe       `bmo_archivo_de(cap)` convierte una capability en
                             `FILE *`, y `bmo_mi_imagen()` es su gemelo exacto:
                             *"la diferencia entre pedir por NOMBRE y tener por
                             DERECHO"* -- `archivo/roja.h`

   el sitio del TITULO ya    `node_box` pinta un titulo que dice QUE ES encima
   esta reservado            del nombre que dice CUAL ES, y `class_color` hoy
                             solo sabe decir tres cosas: "directorio",
                             "archivo", "ilegible". Ahi es donde diria "Formato
                             texto" -- `scene/data/mod.rs`
```

★ Esa ultima fila es la que importa para la idea de la carpeta: **el hueco ya
estaba hecho y no tenia nada que poner**.

---

## 2. EL UNICO HUECO DE VERDAD, y es uno solo

**Nada en BMO-X puede decirle a un programa QUE FICHERO abrir.**

`TASK_OP_EJECUTAR` lleva dos cosas: la ruta *del programa* --acumulada en trozos
de 8 bytes por `TASK_OP_RUTA`-- y el handle de la consola que el padre le
entrega. Y se acabo. No hay argumentos.

Por eso `texto.bex` tiene `datos/notas.txt` clavado en el codigo. No fue pereza:
es que no hay por donde pasarselo.

Y ese hueco es **el mismo** para las tres rutas de la seccion 4. Decidir el
formato sin abrirlo seria decidir el envoltorio de algo que no se puede entregar.

### 2.1 La forma correcta ya esta dibujada por el sistema

El handle de la consola es el precedente exacto: el padre tiene una capability,
la pasa en un argumento de `EJECUTAR`, y el kernel la re-asigna al hijo
(`assign_output`). Un documento puede viajar igual.

```text
   el escritorio abre el documento     ya sabe: `Archivo::leer_de`
   pasa el handle al lanzar            la ranura de al lado de la consola
   el hijo pregunta MI_DOCUMENTO       el hermano de MI_PAQUETE (0x26)
   y lo convierte en FILE *            `bmo_archivo_de(cap)`, que ya existe
```

** Y eso es mejor que pasar un NOMBRE por la razon que `bmo_mi_imagen` ya tiene
escrita: *quien puede escribir una ruta puede escribir otra*. Una app que recibe
un derecho sobre UN fichero no puede tocar el de al lado; una app que recibe una
cadena, si -- y entonces hay que confiar en ella.

### 2.2 ⚠ Y la arruga que no se puede esconder: GUARDAR

El kernel no sabe escribir DENTRO de un fichero que ya existe. Lo dice
`archivo/roja.h` de si mismo: el modo `a` se comporta como `w` --o sea TRUNCA--
porque *"el kernel abre un archivo de escritura con un buffer vacio y lo vuelca
entero al cerrar"*.

Consecuencia directa: una capability de LECTURA abre el documento y no puede
guardarlo; una de ESCRITURA entregada al arrancar **borraria el documento en ese
mismo instante**. Asi que hoy, para guardar, la app necesita el NOMBRE -- o el
kernel tiene que aprender a reabrir para escribir.

```text
   [ ] un programa recibe QUE abrir: `MI_DOCUMENTO`, hermano de `MI_PAQUETE`
       -- `syscall/ops.rs`, `obj/tarea.rs`, `proceso.rs`, `archivo.h`
   [ ] y decidir si viaja como DERECHO (capability) o como NOMBRE, sabiendo
       que guardar hoy exige el nombre -- esta seccion
   [ ] o que el kernel sepa reabrir un fichero para reescribirlo sin
       truncarlo al abrir -- `obj/file.rs`, `archivo/roja.h`
```

---

## 3. LA CARPETA QUE DICE EL FORMATO -- la idea del dueno, y su correccion

> *"para no confundir es mejor que sea una carpeta con que es **Formato texto**,
> lo mismo como PNG, JPG etc..."*

**Tiene sentido, y encaja con lo que esta casa ya hizo una vez.** El volumen de
datos se ordeno exactamente con este criterio --`sys\`, `c\`, `cobol\`, `ada\`,
`datos\`-- y el build dejo escrito por que: *"un `ls` daba diecisiete lineas sin
orden"*, y ordenar **ahorro tecleo** en vez de costarlo.

Para los DATOS, la carpeta que dice el formato hace tres cosas de una:

```text
   1. se entiende sin saber una extension     "Formato texto" se lee; `.datex`
                                              hay que aprenderlo
   2. le da al escritorio la regla de quien   la carpeta dice el formato -> el
      abre, sin manifiesto en cada fichero    formato dice quien lo abre. Una
                                              tabla, no un cerebro
   3. quita el `.bex` de delante              los programas en sus carpetas, lo
                                              tuyo en las tuyas
```

### 3.1 El numero incomodo: "Formato texto" NO CABE en el disco

El driver FAT32 del kernel **se niega a recortar un nombre**, y el build lo dice
de las carpetas igual que de los ficheros: *"los nombres de carpeta tambien son
8.3"*. "Formato texto" son catorce caracteres y lleva un espacio.

Eso no mata la idea: la parte. En el disco la carpeta se llama `texto\` (cinco
caracteres, cabe de sobra) y **el nombre bonito vive en la tabla del
escritorio**, que es la misma tabla que ya iba a decir quien abre el formato:

```text
   en el disco   en la pantalla      quien lo abre        su color
   texto\        "Formato texto"     c/texto.bex          el de ARCHIVO
   imagen\       "Formato imagen"    (nadie todavia)      --
```

Tres columnas y una fila por formato. Es un FORMATO, no un cerebro -- la regla de
la casa.

### 3.2 ⚠⚠ La correccion, y es la unica parte donde digo otra cosa

**La carpeta ORDENA; no DEFINE.** Y la referencia que el dueno puso --PNG, JPG--
es justo la que lo demuestra: un PNG es un PNG **en cualquier carpeta**, porque
lo que dice que es PNG son sus primeros bytes. El tipo VIAJA CON EL FICHERO.

Si la carpeta fuera la verdad, **mover un fichero cambiaria lo que es**: copias
tus notas a `imagen\` y el sistema intentaria abrirlas con un visor de imagenes.
Eso es lo que pasaba en los noventa y es de lo que PNG y JPG salieron.

Asi que las dos cosas, y en este orden:

```text
   la carpeta    ORDEN: donde lo encuentras, como se llama en pantalla, y el
                 sitio por defecto donde se guarda lo nuevo
   el fichero    VERDAD: su extension hoy, sus bytes manana. Si los dos no
                 coinciden, **manda el fichero**
```

Y cuando no coincidan, se dice -- no se adivina. Un fichero en la carpeta
equivocada es un fichero en la carpeta equivocada, no un fichero de otro tipo.

---

## 3b. ** LA CORRECCION DEL DUENO: `.window`, `.data` -- Y YA EXISTE

El 2026-09-11, mas tarde, el dueno corrigio su propia idea. Y el motivo es el
mejor de esta pagina:

> *"en Windows tienes carpetas pero son desordenadas y eso es molesto... una app
> en Windows tiene OpenGL y todas esas cosas que no me sirven **ni para leer**,
> que me dan flojera. Entonces la estrategia ya no seria `.datex` sino otro
> enfoque: si es para mostrar pantalla ese seria `.window`, otro si es para que
> sean datos `.data`. Para aplicar asi en TODAS las APP."*

O sea: **que el artefacto diga PARA QUE ES y QUE NECESITA**, y que no traiga
nada escondido. Y la pregunta que hizo fue la correcta: *"eso cumple las reglas
o redefinimos"*.

### 3b.1 ★★ Su `.window` ya tiene NUMERO, y es `CLASE_PANTALLA = 0x0003`

La seccion `Requisitos = 0x15` del BEF. **Cada `.bex` que sale del escritor la
trae escrita desde el 2026-08-10**, y el kernel la lee en `task/admitir.rs`
antes del pase 2 -- *"el no barato es el que se da antes de la primera reserva"*.
Ocho clases, cada una con su MOTIVO, que viaja con el rechazo:

```text
   0x0001 MEMORIA    bytes que tienen que existir antes de la 1a instruccion
   0x0002 RECURSOS   lo que quiere RESIDENTE en RAM (lo que se lee a demanda,
                     NO se declara: no le cuesta RAM a nadie)
   0x0003 PANTALLA   <- el `.window` del dueno, y ya esta
   0x0004 AUDIO      el aparato de audio
   0x0005 ENTRADA    teclado y raton
   0x0006 CPU        extensiones cuyo estado hay que preservar
   0x0007 PROCESOS   huecos de proceso, si lanza hijos
   0x0008 MONTON     lo que va a repartir DESPUES de arrancar
```

Y no esta sola. En el mismo fichero hay **cuatro secciones** que hacen lo que la
idea pide:

```text
   Manifest   0x09   TOML: metadatos + capabilities -- `fs_read`, `fs_write`,
                     `net_raw`, **`gpu_direct`**, `audio_output`, `ipc_send`
   Resources  0x0B   el icono y los datos que viajan dentro
   Requisitos 0x15   las ocho clases de arriba, para que Ring 0 no DEDUZCA
   Katanas    0x16   por cada regla, DONDE esta su bloque de trampa dentro de
                     `Code`, para que un tercero lo compruebe sin el compilador
```

### 3b.2 ⚠⚠ Y por que la EXTENSION seria un paso atras -- la frase ya estaba escrita

`bef/katanas.rs`, sobre este problema exacto:

> *"Un binario puede decir 'no tengo comportamiento indefinido' en su manifiesto
> y no traer ni una comprobacion dentro. **Hoy no hay forma de desmentirlo**."*
>
> *"**Declarar sin comprobar es propaganda. Comprobar sin declarar es adivinar.
> Las dos juntas son un contrato.**"*

Un nombre de fichero es una promesa que **nadie puede desmentir**: renombras
`doom.bex` a `doom.window` y no has cambiado un byte. Una seccion declarada SI
se puede contrastar con los bytes -- eso es literalmente para lo que nacieron las
katanas. Poner el papel en el nombre seria cambiar un contrato por una etiqueta.

### 3b.3 Tres problemas medibles de partir la app en FICHEROS

```text
   1. no hay compilacion separada ni enlazado dinamico. Un `.window` que sea
      CODIGO no se puede cargar y unir a otro fichero -- REX *"no es un
      runtime: lo que incluyes compila hacia dentro de tu `.bex`"*.
      ** Y el reparto por papeles YA EXISTE en BMO-X, y no son ficheros: son
      PROCESOS. La superficie ES "la parte que muestra pantalla", y la frontera
      del proceso es mas fuerte que cualquier extension
   2. rompe *"una app es UN fichero"*, que es lo que mata el `.lnk`
   3. una extension lleva UN papel y una app tiene VARIOS: `texto.bex` muestra
      ventana Y guarda datos. `texto.window` o `texto.data`? La LISTA de
      papeles no cabe en un nombre; cabe en una declaracion
```

### 3b.4 El dolor del OpenGL ya esta resuelto, y se mide

REX no es un runtime, asi que una app **no lleva un byte de lo que no incluye**.
Los tamanos del build del 11-09 lo dicen enteros:

```text
   holac.bex     2.791 B     un hola mundo
   blit.bex      7.260 B
   ray.bex      33.972 B     2.5D en punto fijo, con su menu
   texto.bex    37.594 B     fuente + ficheros + superficie + buzon
   doom.bex    874.896 B     DOOM entero
```

Un hola mundo son **2,7 KB**. Eso es lo que Windows no puede decir, y es la
mitad del problema del dueno ya pagada. **La otra mitad es la que falta**: nadie
le ENSENA lo que un `.bex` declara. Las secciones estan, el kernel usa una parte
--hoy solo juzga las clases que se miden en BYTES; PANTALLA, AUDIO y ENTRADA se
declaran y todavia no se juzgan-- y ni el escritorio ni una herramienta se lo
cuentan a nadie. Ninguna herramienta de `toolchain/tools/` lee `Requisitos` ni
`Manifest`.

### 3b.5 ★ La redefinicion, y no hace falta abolir nada

L6g dice: **un fichero declara su carril, su coste y su riesgo.** Vale para el
codigo fuente, dentro del repo.

> Redefinido hacia fuera: **un `.bex` tambien declara lo que necesita, y el
> dueno lo VE.**

No es una ley nueva; es la misma cruzando la frontera del artefacto. Y el
precedente de como se hace ya existe: **`.ibex`**. Ese nombre no dice lo que el
programa hace -- dice que **se comprometio**, y lo declara dentro (su perfil, sus
piezas, su mesa de katanas), de forma que un guardian puede desmentirlo. *"El
MISMO formato, con un nombre que dice a que se ha comprometido."*

O sea que si una extension nueva tiene que significar algo, lo que puede
significar es **"este se comprometio y se puede comprobar"**, no "este dibuja".

### 3b.6 Y que queda de `.data`

Esa mitad sobrevive tal cual, y es la de la seccion 3: **para los DATOS, la
extension que dice el formato es lo correcto** -- es lo que hacen PNG y JPG, y el
tipo viaja con el fichero. Lo que no funciona es para los PROGRAMAS, donde el
papel no es uno y la declaracion se puede comprobar.

```text
   [ ] ENSENAR lo que un `.bex` declara: una herramienta que vuelque
       `Manifest` + `Requisitos` + `Katanas` + `Resources`. Hoy NINGUNA de
       `toolchain/tools/` lee las dos primeras -- y es la queja literal del
       dueno: *"ni para leer"*
   [ ] y que el escritorio lo ensene al senalar un icono: que pide esta app
       -- `scene/launcher.rs`
   [ ] juzgar las clases que no se miden en bytes: PANTALLA, AUDIO y ENTRADA se
       declaran y no se juzgan -- `task/admitir.rs`
```

---

## 4. LAS TRES RUTAS, con lo que cuesta cada una

```text
   A  SIN `.datex`          el documento es el fichero llano. Tabla
      el fichero llano      formato -> quien abre. El `.txt` sigue siendo un
                            `.txt`: lo lee el visor, lo lee otro programa, y
                            GUARDAR funciona
                            coste: el escritorio tiene que llevar la tabla

   B  `.datex` que LLEVA    un fichero autocontenido con su icono y sus piezas
      los bytes             dentro, como el `.wad`. Nada que apuntar, nada que
                            se rompa al mover
                            ⚠ coste REAL: **hoy seria de SOLO LECTURA**.
                            `bmo-pack` es una herramienta de Windows; dentro de
                            BMO-X nada sabe ESCRIBIR un paquete BEF, solo
                            leerlos. El bloc de notas no podria guardar

   C  `.datex` que NOMBRA   un sobre con icono + quien lo abre + que fichero.
      el fichero            Editable y barato
                            ⚠ coste: es un PUNTERO, y se rompe al mover el
                            fichero -- exactamente el `.lnk` del que
                            `scene/launcher.rs` presume de no ser
```

### 4.1 Lo que de verdad decide entre B y A

El dueno lo dijo sin darle importancia: **`.datex` es para que el `.bex` solo
EJECUTE**. Ese es el reparto de DOOM, y ese reparto **paga cuando los datos son
grandes o son muchas piezas y el programa es generico**: un motor mas su `.wad`,
un manual con sus imagenes, una partida guardada, un paquete de fuentes.

Para una nota de dos kilobytes no paga: la nota **es** el dato, y meterla en un
contenedor anade un envoltorio y le quita el poder guardarse.

O sea que A y B no compiten: contestan a preguntas distintas.

```text
   lo que EDITAS cada dia        fichero llano (A)
   lo que un motor CONSUME       `.datex` (B), y de solo lectura esta bien --
                                 un `.wad` tampoco se edita jugando
```

---

## 5. ★ CASILLAS

```text
   [ ] DECIDIR que es un `.datex`: datos para un motor (el `.wad`) o documento
       con su abridor. Hoy la palabra cubre las dos cosas y son distintas --
       esta seccion
   [ ] la tabla del formato: disco corto, nombre bonito en pantalla, quien
       abre. Tres columnas -- `scene/data/mod.rs` (`class_color`) y
       `scene/iconos.rs`
   [ ] que el panel de datos diga "Formato texto" en vez de "archivo" -- es el
       titulo que `node_box` ya pinta y que hoy solo tiene tres valores
   [ ] abrir un documento para EDITARLO y no solo para verlo: hoy el doble clic
       lleva al visor de solo lectura -- `scene/data/visor.rs`
   [ ] si `.datex` gana: algo dentro de BMO-X que sepa ESCRIBIR un paquete BEF.
       Hoy solo lo escribe `bmo-pack`, que corre en Windows -- `tools/bmo-pack`,
       `paquete.h`
   [ ] y el icono de `texto.bex`: sin decidir a proposito. Si el escritorio va a
       listar DOCUMENTOS, un icono mas de `.bex` es justo la confusion que el
       dueno quiere quitar -- `scene/launcher.rs`
```

---

## 6. LO QUE NO SE HACE HASTA QUE ESTO SE DECIDA

Nada de lo de arriba. Y se dice aqui para que no se cuele por la puerta de atras:
**ni una excepcion en el build**, ni un `.bex` mas en `apps\`, ni una extension
nueva reconocida a medias en el escritorio. Una carpeta que se ordeno una vez se
desordena con la primera excepcion comoda, y eso ya paso una vez hoy -- ver el
comentario de `texto.bex` en `build/ejemplos.ps1`.
