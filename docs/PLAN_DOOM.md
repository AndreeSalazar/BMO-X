# El plan largo: de "BMO C compila 69 de 81" a "DOOM se juega en el Ryzen"

> Escrito el **2026-08-08**, el dia que la sonda paso de 0 a 69 ficheros sueltos
> y el unity build empezo a parsear las 56.465 lineas enteras.
>
> ★★ **ACTUALIZADO EL 2026-08-09: LA FASE 1 ESTA HECHA.** El unity build ya no
> se para: con un backend de plataforma vacio, las 56.465 lineas salen en un
> `.bex` de **1.299.512 bytes**. Lo que queda para verlo correr es la FASE 2 --
> seis funciones-- y ya no hay nada desconocido delante.
>
> `docs/QUE_DESBLOQUEA.md` dice **que falta y por que**. Este dice **en que
> orden, que bloquea a que, y como se sabe que una casilla esta hecha.** Es el
> mismo trato que `toolchain/lang/cobol/PLAN_BANCA.md` tiene con la banca.
>
> Esta hecho para avanzar **poco a poco**: cada casilla se puede entregar sola,
> con su prueba, sin dejar el compilador roto entre medias.

## Como se lee esto

```
[ ]  pendiente        [~]  a medias, y se dice cuanto        [x]  hecho, con fecha
★    la pieza que decide su fase
⛔   BLOQUEADO, y por QUE -- comprobado en el codigo, no supuesto
⚠    tiene una decision dentro que hay que tomar antes de escribir codigo
```

Tamano: **S** una sesion - **M** dos o tres - **L** una semana de verdad -
**XL** la pieza grande de su fase.

## La regla que no se negocia

**Nada entra sin su fila en el banco de pruebas, y la fila EJECUTA el programa.**
Hoy son 311 en `bmo-c-front`. Las dos cosas que este proyecto encontro el 08-08
--`p->x++` que no hacia nada y `*p` que leia ocho bytes-- **no dan error**: dan
un numero. Solo se ven ejecutando.

---

# FASE 0 -- Donde estamos, medido

No es una fase de trabajo: es la linea de salida, para que las de abajo se lean
contra algo.

```
   ficheros sueltos:  0 -> 7 -> 27 -> 35 -> 41 -> 47 -> 55 -> 61 -> 67 -> 69
   unity build:       parsea (08-08) -> ** COMPILA A .bex ** (08-09)
                      1.299.512 bytes, con backend de plataforma vacio
```

| | |
|---|---|
| Lenguaje | 32/32 sondas, y siete tandas de arreglos el 08-08 |
| Relocations | las **tres** caras: cadena, funcion y global |
| Ficheros | `fopen`/`fread`/`fseek`/`fclose`, `ARCH_OP_LEER_EN` |
| Memoria | `KIND_MEMORIA`, y DOOM pide **un solo bloque** de 6 MiB |
| Pantalla | framebuffer con doble bufer, ya probado con `ray.bex` |
| Entrada | teclado USB con su ESC (que hasta el 08-08 **no existia**) |
| Tiempo | `INFO_TICKS` y el TSC medido |

★ Y los dos hallazgos que cambian la estimacion, comprobados en el codigo de
DOOM y no supuestos:

- **El renderer no necesita coma flotante.** El unico `atan()` esta dentro de un
  `#if 0` que dice *"UNUSED - now getting from tables.c"*.
- **El tope de 4 `malloc` por proceso no lo bloquea.** `I_ZoneBase` pide UN
  bloque y `Z_Malloc` reparte desde dentro.

---

# FASE 1 -- Que el unity build llegue al final

Es la fase que decide todo lo demas: mientras no salga un `.bex`, las fases de
abajo no se pueden ni empezar a probar.

| # | Casilla | Tam | Estado |
|---|---|---|---|
| 1.0 | ★ **`printf` con formato en tiempo de ejecucion** | XL | **[x] 2026-08-09** -- camino A |
| 1.1 | `sprintf` / `snprintf` sobre el mismo formateador | M | **[x] 2026-08-09** |
| 1.2 | `fprintf` -- 64 llamadas en DOOM | S | **[x] 2026-08-09** (van a consola) |
| 1.3 | Las ~20 triviales | M | **[x] 2026-08-09** |
| 1.4 | `system` `mkdir` `getenv` `remove` `rename` -- apuntaladas con su motivo | S | **[x] 2026-08-09** |
| 1.5 | Que el `.bex` quepa en `MAX_BEX` | S | **[x] 2026-08-09** -- ver abajo |

**Y cuatro cosas que no estaban en la lista y hubo que hacer**, porque no se
sabian hasta intentarlo:

| | Que era | Por que no estaba previsto |
|---|---|---|
| 1.6 | ★ **`__va_list()`** -- el `va_list` pasa a ser un PUNTERO | `__va_arg(i)` es un INDICE, y un indice no sobrevive a pasarlo a otra funcion: describe una posicion en el marco de quien pregunta. Sin esto no hay familia `v*`, y `M_vsnprintf` es exactamente eso |
| 1.7 | ★ **El `#elif` compilaba las dos ramas** | Ep. 36 de la bitacora. `i_swap.h` definia `SYS_LITTLE_ENDIAN` **y** `SYS_BIG_ENDIAN` |
| 1.8 | ★ **`double` como PARAMETRO** | Lo pedia `fabs`. El motivo escrito era *"falta la ABI de xmm"* y resulto que no hace falta ninguna: aqui los argumentos van por la pila |
| 1.9 | `sscanf` -- 8 llamadas, con `%i` de base automatica | Estaba contada como trivial y no lo es: `M_StrToInt` distingue `0x`, `0` y decimal con el formato |

## 1.5 -- El tope, y quien manda sobre el

`MAX_BEX` estaba en **1 MiB** y la imagen mide **1.299.512 bytes**: no cabia
por 248.936. Se subio a **4 MiB**, y la regla que se aplico queda escrita aqui
porque va a volver a hacer falta:

> **El programa ajeno manda sobre el tope, no al reves.** DOOM es de 1993 y es
> el codigo mas apretado que se va a portar aqui en mucho tiempo. Si no cabe, el
> que esta mal medido es el bufer.

Lo que cuesta: `.bss` del kernel, dentro del hueco de **16 MiB** que el cargador
UEFI ya reserva y pone a cero en `0x400000`. **El `.bin` no crece** --se midio:
909.696 B antes y despues-- porque `.bss` no viaja en la imagen.

## ★ 1.0 -- El formateador en ejecucion, que es la pieza de verdad

Hoy `printf` se emite **en linea desde un literal**: el compilador lee el
formato al compilar y escribe las llamadas. `I_Error(fmt, ...)` y `M_snprintf`
reciben el formato **como argumento**, y ahi no hay literal que leer.

Lo que hace falta es un `__bmo_vprintf(fmt, args)` que recorra la cadena al
vuelo. Las piezas ya estan casi todas:

- los formateadores sintetizados (`__bmo_fmt_i64`, `_u64_hex`, `_cstr`...),
- `__va_arg(i)`, que da el variadico `i` porque BMO pasa los variadicos por la
  pila detras de los nombrados,
- y `console::write_const` para lo literal.

⚠ **La decision que hay que tomar antes de escribir codigo**: donde vive.

| Camino | A favor | En contra |
|---|---|---|
| **A** -- en C, dentro del unity | se escribe una vez, se lee, se prueba con el resto | hay que interceptar `printf` en el codegen para que NO lo emita en linea |
| **B** -- como funcion sintetizada | no toca el codegen de llamadas | el mecanismo de sintetizadas recibe `&mut Vec<u8>` y **no sabe de etiquetas**: un bucle con saltos no cabe hoy |

Hoy A es mas barato, y B pide primero ensanchar la tabla de sintetizadas para
que acepte emisores con etiquetas -- que es un cambio de la TABLA, no de las
funciones. Ver la cabecera de `codegen/sintetizadas.rs`.

**Como se sabe que 1.0 esta hecha**: `printf(fmt, 42)` con `fmt` en una variable
imprime `42`, y la fila lo EJECUTA.

### [x] HECHO el 2026-08-09 -- se tomo el CAMINO A, y con un matiz

El formateador vive en **`toolchain/forge/sem-asm/tables/stdio.h`**, escrito en
C, y el codegen desvia ahi el `printf` cuyo formato no es literal. De la tabla
de sintetizadas solo hizo falta **una** entrada nueva, `bmo_escribir`, que saca
a la consola un bufer que no existia al compilar -- su cuerpo es
`console::write_buffer`, que ya estaba escrito y solo alcanzaba el codegen.

★ **El detalle que hizo el desvio corto**: en BMO los argumentos se empujan por
la pila, asi que **empujarlos en orden inverso deja en memoria un `va_list`
tal cual**, y lo que se le pasa al formateador es `rsp`. No hay area de
argumentos que construir.

★ **Y una cosa que el camino B no habria dado**: este formateador **aplica la
anchura**. El de linea lee el `7` de `%7i` y lo tira --lo dice en su propio
comentario-- porque sus conversores escriben directo a la consola y para
rellenar hay que saber cuanto ocupa el numero ANTES. Aqui se arma en un array
primero, asi que una tabla alineada sale alineada.

Filas que lo ejecutan, en `tests/libc.rs`: el formato en una variable, la
anchura y las banderas, el `va_list` que viaja a otra funcion, el truncado de
`snprintf` sin desbordar, el hexadecimal con el bit alto puesto, y una
conversion desconocida que **no se come el argumento**.

---

# FASE 1.5 -- Lo que la lista NO tenia, y bloqueaba de verdad

Escrito el **2026-08-09**, tarde. Ninguna de las tres estaba en el plan y las
tres impiden que DOOM arranque. Salieron de contar en vez de suponer.

| # | Que era | Como se supo |
|---|---|---|
| 1.10 | ★ **El tope de 4 `malloc`** | El plan decia que no bloqueaba porque `I_ZoneBase` pide UN bloque. Contando los sitios: el arranque llama a `malloc` **una docena de veces** -- solo `I_AtExit` son siete. **[x]** `<bmo/monton.h>`, un asignador de Ring 3 |
| 1.11 | ★★ **El teclado no tenia SOLTAR** | `INPUT_OP_TECLA` entrega un CARACTER, y un caracter no tiene "solto". Quien echa a andar no para nunca; y Shift/Ctrl/Alt no producen caracter, asi que ni salian. **[x]** `INPUT_OP_EVENTO_TECLA` |
| 1.12 | **`fseek` ignoraba el origen** | `M_FileLength` mide el WAD con `SEEK_END`. Con el origen ignorado, **el WAD media cero bytes** sin una sola linea de error. **[x]** -- y de paso salio que `feof` daba EOF pasada la mitad de cualquier fichero |

★ La leccion, que vale mas que las tres: **el plan daba por bloqueado lo que
era visible (el lenguaje) y por resuelto lo que no lo era (la superficie del
sistema).** Las tres se encontraron mirando el codigo de DOOM y el del kernel a
la vez, no compilando.

Y una que no bloquea pero se llevaba media imagen: **el 90,3% de la seccion
`data` de DOOM eran ceros** que viajaban en el fichero. Ver `docs/LA_RAM.md`.

---

# FASE 2 -- La capa de plataforma: seis funciones

DOOM (doomgeneric) habla con el sistema por **seis funciones**, y las seis ya
tienen con que hacerse. Es `doomgeneric_bmo.c`, y es el fichero que hay que
escribir de cero.

## [x] ESCRITA ENTERA el 2026-08-09 -- `doomgeneric_bmo.c`

| # | Casilla | Con que se hizo |
|---|---|---|
| 2.0 | `DG_Init` | `PANTALLA_RECLAMAR` + `ENTRADA_RECLAMAR`, `FB_BASE`/`DIMS`/`STRIDE`, centrado |
| 2.1 | `DG_DrawFrame` | `memcpy` fila a fila -- **por el stride**, ver abajo |
| 2.2 | `DG_GetTicksMs` | **TSC**, no `INFO_TICKS` |
| 2.3 | `DG_SleepMs` | girar sobre el TSC **cediendo** |
| 2.4 | `DG_GetKey` | `INPUT_OP_EVENTO_TECLA` + tabla de scancode a `doomkeys.h` |
| 2.5 | `DG_SetWindowTitle` | a consola: DOOM tiene la pantalla entera |

**El `.bex`: 812.736 bytes.** Vive en `BMO-externo/doom/doomgeneric/`, fuera del
repo, porque es GPL.

⚠ **La decision de 2.1 se resolvio sola, y para bien**: DOOM **ya entrega 32
bits**. `I_FinishUpdate` llama a `cmap_to_fb` y deja `DG_ScreenBuffer` con
640x400 pixeles listos -- la expansion por paleta la hace DOOM, no nosotros.
Aqui solo queda el blit. Lo que si hubo que hacer es copiar **fila a fila**: el
framebuffer tiene stride, y un solo `memcpy` de corrido funciona en el panel
donde stride == ancho y sale torcido en el primero donde no.

★ **2.2 NO usa `INFO_TICKS`**, y el motivo importa: el tick del LAPIC se calibra
en el arranque y **su frecuencia no esta declarada en ninguna constante que un
programa pueda leer**. La del TSC si (`INFO_TSC_HZ`, medida por el kernel), asi
que el reloj sale de `__rdtsc()` dividido por ciclos-por-milisegundo. Un reloj
que no se puede convertir a milisegundos no es un reloj.

★ **2.3 tiene una trampa que se paga cara**: girar sobre el reloj sin ceder no
solo quema el quantum -- deja al resto del sistema sin turno, **incluido el bus
USB, que se sondea desde dentro de un syscall**. O sea que un bucle de espera
mal escrito aqui apaga el teclado de este mismo programa.

**Como se sabe que la fase esta hecha**: sale el menu de DOOM en el Ryzen, con
foto. **Todavia no ha corrido.**

---

# FASE 3 -- El WAD, que es donde vive el juego

| # | Casilla | Tam | Nota |
|---|---|---|---|
| 3.0 | Leer `doom1.wad` (4.196.020 B) con la cadena de ficheros | S | **[x] escrito** -- `-iwad apps/doom1.wad` por `myargv` |
| 3.1 | ⚠ Que quepa: el WAD son 4 MiB | M | **no hace falta**: DOOM NO lo carga entero, ver abajo |
| 3.2 | `W_CacheLumpName` sobre el zone allocator | S | es codigo de DOOM, no de BMO |

★ **3.1 se cayo sola al mirarlo**: `w_file_stdc.c` **no slurpea el WAD**. Lee el
directorio de lumps al abrir y luego cada lump por `fseek`+`fread` cuando hace
falta, a memoria de la zona. Nunca hay 4 MiB en vuelo. Lo que si hizo falta fue
que `fseek` entendiera `SEEK_END`, porque el WAD se MIDE con el (fase 1.12).

★ **El WAD se nombra, no se busca.** `d_iwad.c` sabe rebuscar en directorios
estandar y en variables de entorno, y aqui no hay ni lo uno ni lo otro --
`getenv` contesta que no hay y lo dice. Se le pasa la ruta por `myargv`, que es
un camino que DOOM ya tiene y que no obliga a inventarse un sistema de ficheros
que BMO-X no promete.

### ★★ 3.1 SI hacia falta, y no por el motivo escrito -- 2026-08-11

DOOM no cargaba el WAD entero. **BMO se lo cargaba por el.** `archivo::open`
pedia los 4.196.020 bytes en marcos CONTIGUOS y los leia de golpe, justo despues
de que DOOM se llevara sus 12 MiB de zona:

```text
   M_LoadDefaults: Load system defaults.
   Unknown configuration variable: 'use_joystick'
   <- y aqui se acaba. Lo siguiente de `D_DoomMain` es `W_Init: Init WADfiles`
```

O sea que la casilla 3.1 estaba bien contada por el lado de DOOM --que no lo
slurpea-- y no se miro el lado de BMO, que si. **[x] Arreglado**: un archivo
abierto para leer ya no se trae, se **refleja** -- un cursor de FAT32, una
ventana de 64 KiB para las lecturas de siete bytes, y cada `fread` trayendo su
rango del disco al bloque del programa. Ver `docs/LA_RAM.md`, seccion del 08-11.

Y la nota vieja de esta fila --*"el camino de lectura copia por un bufer de
rebote del kernel... si duele, lo arregla el DMA al bufer del llamante"*-- queda
cerrada de paso: `ARCH_OP_LEER_EN` escribe por el espejo fisico del bloque, asi
que el HBA deja los sectores enteros **dentro de la zona de DOOM**. Lo que se
mide sigue siendo `disk::cuentas_dma()`, y ahora ademas `archivo::cuentas()`.

---

# FASE 4 -- Que se pueda jugar de verdad

| # | Casilla | Tam |
|---|---|---|
| 4.0 | Guardar partida (`fwrite` + modo I-O) | M |
| 4.1 | El menu y los cheats (dependen de `M_snprintf`, o sea de 1.0) | S |
| 4.2 | Medir fotogramas por segundo y decirlo | S |

---

# FASE 5 -- SONIDO

> **Estado real, comprobado**: `platform/drivers/audio/` existe y son **109
> lineas de altavoz de PC** -- `outb` a un puerto y un retardo por TSC. Sirve
> para un pitido, no para DOOM. Y **no lo llama nadie**: es uno de los crates
> huerfanos de la auditoria de deuda tecnica.

O sea que el audio de verdad **empieza de cero**, y por eso va al final: DOOM se
juega entero sin sonido, y ninguna de las fases de arriba lo necesita.

## ★★ LA FASE ESTABA MIRANDO AL SITIO EQUIVOCADO (2026-08-10)

El 10 de agosto Vivaldi corrio en el Ryzen y no se oyo. El log dijo por que, y
de paso reordeno esta fase entera:

```
   info uaudio  el aparato guardo OTRO volumen =35
```

`=35` es el eco piano de la pieza, y **el audifono USB lo guardo**. O sea que la
cadena del volumen funciona de punta a punta hasta el aparato de verdad. Lo que
no llega es la NOTA: `AUDIO_OP_VOLUME` va al altavoz del PC **y** al audifono,
y `AUDIO_OP_BEEP` va **solo** al altavoz -- que en esta placa no tiene zumbador.

** Asi que el camino corto no es HD Audio, es USB, y lo caro ya esta pagado:

```
   xHCI                      HECHO       enumerar el aparato    HECHO
   leer sus descriptores     HECHO       control transfers      HECHO
   transferencias ISOCRONAS  <- FALTA, y es lo unico
```

`platform/drivers/usb/uaudio` lo dice en su primera linea. `bmo-xhci` tiene
`queue_interrupt_in` y le falta su equivalente isocrono de salida.

| # | Casilla | Tam | Nota |
|---|---|---|---|
| 5.0b | ★ **Isocronas de salida en `bmo-xhci`** | L | El camino corto. Un anillo de TRBs isocronos y el alt-setting 1 del aparato |
| 5.0 | ~~Decidir el aparato~~ | M | ~~HD Audio o AC'97~~ **Contestado por el metal: USB**, porque ya esta enumerado y contestando |
| 5.1 | Enumerar el codec y abrir un stream de salida | XL | es un driver entero, con DMA y su anillo de buffers |
| 5.2 | `KIND_AUDIO` como capability | M | un proceso que no la tiene **no hace ruido**, igual que la pantalla |
| 5.3 | Mezclar los canales de DOOM (`i_sound.c`) | L | DOOM mezcla el mismo, solo pide un buffer |
| 5.4 | Musica MUS -> MIDI (`mus2mid.c` ya compila) | XL | y sin sintetizador MIDI no suena: es OTRO proyecto |

★ **La linea honesta**: 5.0 a 5.3 son "DOOM con efectos". 5.4 es "DOOM con
musica", y eso pide un sintetizador. **Se paran en 5.3 y se dice.**

---

# La cuenta, para poder repartir

Actualizada el **2026-08-09**, tarde.

| Fase | Casillas | Faltan | Estado |
|---|---|---|---|
| 1 -- el unity termina | 6 | 0 | **[x]** compila a `.bex` |
| 1.5 -- lo que no estaba en la lista | 3 | 0 | **[x]** monton, tecla cruda, `fseek` |
| 2 -- la plataforma | 6 | 0 | **[x]** escrita, `doomgeneric_bmo.c` |
| 3 -- el WAD | 3 | 0 | **[x]** escrito -- `-iwad apps/doom1.wad` |
| 4 -- jugable | 3 | 2 | guardar partida pide `fwrite`, que devuelve 0 |
| 5 -- sonido | 5 | 5, y **empieza de cero** | nada lo bloquea |

★★ **NO QUEDA NINGUNA CASILLA POR ESCRIBIR. Lo que queda es un FALLO EN METAL,
y eso ya es otra clase de trabajo.**

DOOM entero --56.465 lineas, mas la capa de plataforma-- compila a un `.bex` de
814.616 bytes, `build.ps1` lo despliega con su WAD y su icono, y el escritorio
lo ensena. **No arranca.**

## ⛔ EL BLOQUEANTE DE HOY -- 2026-08-09, 23:30

```
   83 WARN proc:   el .bex de disco no paso la admision =4
   84 WARN lanzar: el .bex no paso la admision =3
```

**Lo que ya esta descartado**, y se dice para que nadie lo vuelva a mirar:

- **No es el tope.** 814.616 B contra 4 MiB.
- **No es la tabla de secciones.** Se volco fuera y pasa todas las
  comprobaciones de `bex::inspect`: seis secciones, `file_size <= mem_size` en
  todas, la `Bss` con `file_size = 0` --que es justo lo que esa comprobacion
  permite-- y el `entry` (597.982) dentro del codigo (598.925).
- **No son los codigos de seccion.** El kernel los lee a mano y coinciden con
  los del ABI: `0x01/0x02/0x03/0x04`.
- **No es el paquete.** `caja.bex` tambien lleva `Resources` y arranca en metal
  desde el 08-09.

★ **El sospechoso que queda**: es el `.bex` **mas grande que se ha cargado
nunca** --2,7 veces `gui.bex`, que era el record-- y en la misma tanda de fotos
sale `lanzar: el archivo no cabe en el buffer`. Una lectura corta de FAT32
produce exactamente este sintoma: la tabla apunta mas alla de lo leido y la
seccion se declara invalida.

★★ **Y EL SIGUIENTE PASO NO ES TOCAR CODIGO, ES MEDIR.** Que `lanzar` diga
**cuantos bytes trajo del disco frente a cuantos mide el fichero**. Son dos
numeros y una linea; mientras no esten en la pantalla, cualquier arreglo es una
apuesta -- y este documento ya tiene una leccion de haber supuesto en vez de
contar (fase 1.10).

## Lo que puede salir mal DESPUES, para no volver a escribirlo

| Sintoma | Sospechoso |
|---|---|
| `DOOM: no hay pantalla` | se lanzo desde el escritorio con otra ventana delante |
| `W_AddFile: doom1.wad no encontrado` | la ruta del WAD, o FAT32 no monta |
| Se para tras `M_LoadDefaults` y **no sale `W_Init`** | el WAD. Era `archivo::open` tragandoselo entero (arreglado el 08-11); si vuelve, mirar `arch` en CABINA |
| Arranca y muere sin pintar | el monton: 12 MiB CONTIGUOS en fisico. CABINA dice si el kernel los nego |
| Pinta y no responde | `DOOM: sin teclado` en la consola lo dice antes |
| Anda solo y no para | la cola cruda no llega: el `soltar` se perdio |
| Va a tirones | el blit, o `DG_SleepMs` cediendo mal |

## Lo que SI se vio, y no es poco

El escritorio arranco con **el icono de DOOM y su nombre debajo**: listar
`apps/`, abrir el `.bex`, encontrar `Resources`, leer el indice `BRES`, sacar el
recurso `icono`, descifrar `BICO` y pintarlo -- siete pasos y dos formatos
leidos a mano, ninguno fallo.

[!] El icono salio **blanco** y deberia ser una cara roja. La silueta es la
correcta --el recorte transparente esta bien-- asi que lo que llego mal es el
color, no el dibujo. Abierto, y distinguible a ojo del otro caso: si fuera el
icono por defecto seria un cuadro macizo con una `D`.

---

Ver [`QUE_DESBLOQUEA.md`](QUE_DESBLOQUEA.md) para el censo, `AVANCES.md` para el
estado y `BMO-externo/doom-port/` (fuera del repo) para la sonda y el unity.
