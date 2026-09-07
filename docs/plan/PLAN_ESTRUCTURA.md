# PLAN DE ESTRUCTURA -- el taller de BMO-X, en F1

> **Lo que afirma**: que se pulsa `F1` en el escritorio, se abre una ventana con
> historial, se teclea `compilar hola.ada`, y aparece un `.bex` en el disco de
> BMO-X. Sin instalar nada, porque no hay nada que instalar.
>
> **Como se cae**: la ventana no abre, o abre y no recibe teclas, o compila y el
> `.bex` que escribe no lo admite el cargador.
>
> Escrito el **2026-09-06**, cuando el dueno pidio *"un terminal que se convierta
> compilador, que llames como F1 en Escritorio, pero que sea APP"*.

---

## 0. EL REPARTO DEL NOMBRE, PARA QUE NO SE CONFUNDA NUNCA

```text
   VALKYRIE-ABI   JUZGA      no ejecuta jamas. No tiene anillo
   ESTRUCTURA     FABRICA    un `.bex` de Ring 3, y apunta a V-ABI
```

** No son dos capas de lo mismo: son el estandar y una herramienta que lo
cumple. `gcc` apunta a POSIX y no es POSIX. Si algun dia ESTRUCTURA se
llamara VALKYRIE, la frase de `VALKYRIE-ABI/README.md` --*"no tiene anillo
porque no tiene ni una instruccion en la maquina de destino"*-- seria falsa el
mismo dia.

---

## 1. LO QUE YA ESTA, Y ES MAS DE LO QUE PARECE

`F1` **esta libre**. En `Ultra_userspace/services/director/src/desktop/keys/app.rs`
la constante `SC_F1 = 0x3B` existe solo como frontera del rango reservado
(`SC_F1..=SC_F10`), y **nada la usa**. Las teclas de funcion las retiene el
escritorio y no bajan a la app de delante, asi que atar `F1` es una linea en el
sitio donde ya se deciden `F11` y `F12`.

Y REX ya trae las seis piezas que un terminal necesita:

```text
   superficie.h   172   dibujar en TU memoria y ofrecerla al DIRECTOR
   entrada.h      372   teclado y raton por buzon
   scroll.h       140   una ventana que se mueve sobre un historial
   archivo.h       65   leer el fuente, escribir el .bex
   paquete.h      261   las tablas que viajan DENTRO del propio .bex
   monton.h       109   malloc sobre un bloque del kernel
```

★ `scroll.h` merece decirse aparte: es **scrollback de terminal**, escrito como
funciones puras sin heap ni buffer escondido. Se prueba entero sin encender la
maquina.

---

## 2. ** LA FRONTERA DECIDE QUE COMANDOS EXISTEN, Y NO ES NEGOCIABLE

El terminal de hoy **no es una app**: son 5.761 lineas en
`Ultra_userspace/services/director/src/commands/` mas 978 de
`scene/consola.rs`, todo dentro del DIRECTOR. Y cuando se mira que hacen:

```text
   reports.rs  1.199    system.rs  908    disco.rs  420    red.rs  309
```

son en su mayoria **lectores de instrumentos del kernel**. Y
`VALKYRIE-ABI/FRONTERA.txt` deja fuera de la superficie de app, por prefijo:

```text
   DISCO_  ES_  CABINA_  USB_  AUTOPSIA_  KLOG_  SYSCALL_  MAQ_
```

### ** CORREGIDO EL 06-09: la frontera es MAS ESTRECHA de lo que decia aqui

La primera version de esta seccion decia que ESTRUCTURA *"no puede correr `cpu`,
`mem`, `disco` ni `red`"*. **Es falso**, y lo desmiente la propia fila de la
frontera:

> `DISCO_` -- *los paneles. Un programa lee el disco por `BMO_INFO_DISCO_*`,
> **que SI esta***

Lo que la frontera deja fuera son las **operaciones de panel**, no las lecturas.
Y las lecturas ya estan publicadas: `bmo/verde.h` de REX trae **49 constantes
`BMO_INFO_*`**, entre ellas 12 de `CPU_`, 10 de `DISCO_` y 8 de `NET_`, todas
por `OP_INFO`.

```text
   ESTRUCTURA SI PUEDE    cpu  mem  disco  red        (las LECTURAS, por OP_INFO)
   ESTRUCTURA NO PUEDE    cabina  klog  autopsia      (las ORDENES de panel)
                          usb  syscall  maq  es
```

** Y lo que queda fuera tiene un motivo que no es tecnico: *"cada fila de la
puerta de los terceros es una promesa que hay que mantener despues"*. Publicar
`KLOG_` convierte el formato del log del kernel en un contrato con terceros, y
ese formato cambia cada vez que se depura algo.

### La consecuencia sigue siendo la misma: son DOS terminales, no uno

```text
   la consola del DIRECTOR   el panel de INSTRUMENTOS   cpu, mem, red, cabina
   ESTRUCTURA (F1)           el TALLER                  compilar, leer, escribir
```

Y eso **no es una limitacion que rodear**: es el primer caso en el que la
frontera se cobra, y contesta sola una pregunta de diseno que si no habria que
discutir. La consola de instrumentos se queda donde esta porque **es un
instrumento**; el taller sale porque es una app.

---

## 3. ⚠ EL CELO OBLIGA A QUE SEA UN SOLO FICHERO

`EJECUTAR` pide autoridad, se fija al nacer y solo desde Ring 0
(`Ultra_kernel_x86-64/kernel/src/ring0/task/autoridad.rs`). Un `.bex` **no puede
lanzar otro**.

Eso descarta el diseno obvio --un terminal que invoca al compilador-- y deja el
correcto:

```text
   estructura.bex   el terminal Y el compilador, en el MISMO fichero,
                    con sus tablas en la seccion 0x0B
   el ESCRITORIO    lanza lo que ESTRUCTURA compilo, cuando el dueno hace clic
```

** Y eso convierte "sin instalar" en algo literal en vez de en un eslogan: **un
fichero que trae dentro lo que necesita**, y que se lee con `paquete.h` sin
copiar nada. La cabecera ya cita al dueno diciendo la idea:

> *"es un bef pero ese bex es el mismo que abre la caja: no lo duplica, lo lee y
> punto."*

---

## 4. ★★ ESTRUCTURA Y EL AUTOHOSPEDAJE SON EL MISMO TRABAJO

El frontend de Ada es **Rust**. Si ESTRUCTURA tiene que contenerlo, ESTRUCTURA es
un `.bex` de Rust -- y la pregunta es que tiene ya la cara de Rust.

### ** CORREGIDO EL 06-09: SE MIDIO CONTRA LA CRATE EQUIVOCADA

La primera version de esta seccion comparaba REX contra **`bmo-rt`** y concluia
que *"la cara de Rust tiene cero"*. Era falso, y el error fue de medida: `bmo-rt`
(`toolchain/lang/base/`, 1.371 lineas) es **el arranque y el monton** --crt0,
syscall, heap, string, fmt, ffi--, o sea el equivalente de la `crt0` y poco mas.

**La cara de Rust de la superficie es `bmo-userland` v2.0.0**, cuya propia
descripcion lo dice: *"Runtime de Ring 3: los dos syscalls, capabilities y la
pantalla"*. Son **3.901 lineas** y es lo que enlaza el DIRECTOR.

```text
   bmo-rt          1.371   crt0, syscall, heap, string, fmt, ffi   EL ARRANQUE
   bmo-userland    3.901   archivo, pantalla, entrada, memoria,    LA SUPERFICIE
                           disco, red, sonido, proceso, estratos
```

### El hueco de verdad, medido contra la crate correcta

| lo que necesita ESTRUCTURA | REX (C) | `bmo-userland` (Rust) |
|---|---|---|
| `archivo` -- leer el fuente, escribir el `.bex` | si | **ya estaba** (344 lineas) |
| `pantalla` -- dibujar | si | **ya estaba** (623) |
| `entrada` -- teclas y raton | si | **ya estaba** (139) |
| `monton` | si | **ya estaba** (`memoria`, 143) |
| `paquete` -- las tablas dentro del fichero | si | **HECHO el 06-09** (escalon 2) |
| `scroll` -- el historial | si | falta como modulo reutilizable |

** Asi que el escalon 2 no eran cinco modulos: era **UNO**, y el unico que
quedaba de verdad. Y su cimiento ya estaba puesto sin que nadie lo dijera --
`Archivo::saltar` lleva escrito desde antes *"hacia falta para leer un PAQUETE:
la seccion de recursos vive al final"*.

Lo que queda es `scroll`, y no bloquea nada hasta el escalon 4: `scene/
historial.rs` (172 lineas) ya lo hace dentro del DIRECTOR y de ahi sale la forma.

---

## 4b. ** CUANTAS PUERTAS CUESTA UNA COMPILACION -- contadas, no estimadas

El dueno pidio el numero. Sale de **leer el codigo y contar**, no de estimar:
cada llamada esta escrita en `Ultra_userspace/userland/src/archivo.rs` y se
puede senalar con el dedo.

```text
   fuente        ->  salida          HOY   CON BLOQUE   factor
   ------------------------------------------------------------
   hola_C.c      ->  holac.bex       656         11        59x
   blit_C.c      ->  blit.bex       2144         11       194x
   leer_C.c      ->  leer.bex       3490         11       317x
   coste_C.c     ->  coste.bex     10657         11       968x
```

### El desglose de `hola_C.c`, que es de donde sale todo

```text
   abrir fuente (ruta 2 + abrir 1)           3
   esperar (cabe en la ventana de 64 KiB)    1
   leer      1839 / 7                      263
   COMPILAR                                  0     <-- todo en memoria
   crear salida (ruta 2 + crear 1)           3
   escribir  2695 / 7                      385
   cerrar                                    1
                                          ----
                                            656
```

** Y la fila que manda es la de **CERO**. Compilar no cuesta ni una puerta: el
parser, el codegen y el emisor trabajan en memoria del proceso. **Todo el coste
de puerta de una compilacion es mover bytes**, y por eso el camino de bloque lo
cambia todo y afinar el compilador no cambiaria nada.

### Por que el numero nuevo es 11 y no crece

```text
   abrir 3 + esperar 1 + pedir el bloque 1 + leer_en 1
   + crear 3 + escribir_de 1 + cerrar 1  =  11
```

**No depende del tamano.** `ARCH_OP_LEER_EN` no pasa por la ventana de 64 KiB
--el rango va del disco al bloque, sin escala-- asi que un fuente de 30 KiB
cuesta las mismas once puertas que uno de 1 KiB.

### [!] LO QUE ESTA CUENTA NO ES

**No es una medicion en metal.** Es una cuenta del codigo, y tiene dos huecos
declarados:

1. **El bucle de `esperar_entero`** vale 1 vuelta solo porque los ficheros de
   ejemplo caben en la ventana de 64 KiB (`obj/file.rs`, `WINDOW`). Un fuente
   mayor gira mas veces, y cuantas depende del disco.
2. **La ruta** se cuenta como 2 paquetes de ocho bytes. Una ruta larga cuesta
   mas, y es la parte que menos importa.

El kernel ya sabe contar puertas --`INFO_SYSCALL_CUENTA`-- asi que confirmarlo
es restar el contador antes y despues. Eso pide **un arranque**, y entra en la
misma deuda que todo lo demas de esta semana.

---

## 5. LOS ESCALONES

Ordenados por la regla de la casa: **lo que no toca nada va primero.**

```text
   [ ] 1  F1 abre una ventana VACIA   en `Ultra_userspace/services/director/
                                      src/desktop/keys/app.rs`, donde ya se
                                      deciden F11 y F12. Sin compilador y sin
                                      terminal: solo que la tecla llegue

   [x] 2  PAQUETE en Rust            HECHO 06-09 --
                                      `Ultra_userspace/userland/src/paquete.rs`
                                      y `Archivo::mi_imagen`. Era el UNICO que
                                      faltaba de los cinco: los otros cuatro ya
                                      estaban en `bmo-userland` (ver seccion 4)

   [ ] 2b la ventana con REJILLA      `scroll` como modulo reutilizable, de la
                                      forma que ya tiene `scene/historial.rs`

   [ ] 3  estructura.bex DIBUJA       una ventana con su rejilla y su cursor,
                                      sin leer una tecla. Se compara contra
                                      `scene/consola.rs`, que ya lo hace

   [ ] 4  y LEE TECLAS                por el buzon de `entrada`, con el
                                      historial de `scroll`. Ya es un terminal,
                                      y todavia no compila nada

   [ ] 5  los comandos que la         `ls`, `cat`, `escribir`. Y los que la
          FRONTERA permite            frontera deja fuera SE DICEN, con el
                                      motivo: "eso es un instrumento, esta en
                                      la consola del DIRECTOR"

   [ ] 6  `compilar hola.ada`         el frontend de Ada dentro del mismo
                                      .bex, con sus tablas en la seccion 0x0B.
                                      Es el escalon 6 de PLAN_AUTOHOSPEDAJE
                                      visto desde aqui

   [ ] 7  exportar donde le digan     a ESTRATOS o a FAT32, y que la ventana
                                      de datos lo ensene
```

### Como se cae cada uno

| escalon | si esta bien | si falla |
|---|---|---|
| 1 | aparece un rectangulo al pulsar F1 | la tecla no llega: se la come el rango reservado |
| 2 | `cargo test` de `bmo-rt` cubre los cinco | un modulo compila y no hace lo que su gemelo en C |
| 3 | la rejilla se ve igual que la del DIRECTOR | el DIRECTOR no compone su superficie |
| 4 | se teclea y sale, y la rueda sube | el buzon se llena y se pierden teclas |
| 5 | `cpu` contesta **por que** no esta | contesta "no existe", que es una respuesta peor |
| 6 | sale un `.bex` que `bmo-verify` admite | se queda sin monton, o el `.bex` sale distinto |
| 7 | el fichero aparece en la ventana de datos | -- |

★ El escalon 5 no es de relleno. *"Ese comando es un instrumento y vive en la
consola del DIRECTOR"* es una respuesta que ensena el sistema; *"comando
desconocido"* deja al que lo teclea creyendo que falta trabajo.

---

## 6. LO QUE **NO** ENTRA, Y SE DICE PARA QUE NO CREZCA SOLO

* **Un editor.** ESTRUCTURA compila lo que hay en el disco. Editar es otra app
  y otro plan.
* **Los comandos de instrumentos.** Seccion 2. Si algun dia uno hace falta de
  verdad, se borra su prefijo de `VALKYRIE-ABI/FRONTERA.txt` **y se escribe por
  que** -- que es el momento en que la decision se toma a la vista.
* **Lanzar lo que compila.** Seccion 3. Eso es del escritorio, y es el celo.
* **Los otros tres lenguajes.** Ada primero por lo que mide
  `PLAN_AUTOHOSPEDAJE` seccion 1; C, COBOL e INTI arrastran `toml`.

---

## 7. LO QUE SERIA UN ERROR

* **Sacar los 5.761 de `commands/` del DIRECTOR para "reaprovecharlos".** La
  mayoria no puede cruzar la frontera, asi que lo que se moveria es codigo que
  despues hay que devolver.
* **Darle autoridad a ESTRUCTURA** para que lance lo que compila. Es el tercer
  bit, y `autoridad.rs` ya dejo escrita la pregunta que va antes.
* **Llamarlo VALKYRIE.** Seccion 0.
* **Empezar por el escalon 6** porque es el que se ve. Sin el 2, no hay ventana
  que lo ensene.

---

Ver [`PLAN_AUTOHOSPEDAJE.md`](PLAN_AUTOHOSPEDAJE.md) (el mismo trabajo desde el
otro lado, y la medida que elige Ada),
[`VALKYRIE-ABI/FRONTERA.txt`](../../VALKYRIE-ABI/FRONTERA.txt) (los ocho
prefijos que deciden la seccion 2),
[`EL_ORQUESTAL.md`](../identidad/EL_ORQUESTAL.md) (por que la autoridad no
viaja) y [`PLAN_REX.md`](PLAN_REX.md) (las cabeceras de las que se copia la
forma).
