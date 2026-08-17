# CENSO DE EJES -- que parte de BMO-X gasta que recurso

> Aplicacion de `META-KERNEL_HARD.md`. Ese documento dice **que exige cada
> componente**; este dice **por donde pasa el trabajo de verdad** y, sobre todo,
> **que se puede TACHAR**.
>
> Escrito el **2026-08-16**. El objetivo del dueno es optimizar; el objetivo de
> este fichero es que la lista de lo optimizable sea **corta, finita y
> justificada con numeros**, en vez de infinita y elegida por intuicion.

---

## 0. La regla del censo

★ **El eje lo decide QUIEN LLAMA y CUANTAS VECES, no la carpeta.**

Por eso este censo se organiza por **CAMINO**, no por directorio. Un fichero no
tiene eje propio: **hereda el del camino por el que pasa**. Y si pasa por dos
caminos con ejes distintos, esta mal cortado (regla A2 del libro).

Un censo que no encuentra nada tambien es un resultado: sirve para contestar
*"por donde empiezo"* la proxima vez, y para que una regresion salte en un
segundo en vez de en tres flasheos.

---

## 1. La aritmetica que TACHA

Esta es la herramienta entera del documento, y cabe en tres lineas:

```
   coste real por segundo  =  coste por vez  x  veces por segundo
```

★★ **Y ANTES DE DIVIDIR NADA, LA UNIDAD -- que la primera version de este
fichero tenia MAL.** Todo lo que mide `rdtsc` son **ticks de TSC**, y el TSC de
este chip es invariante: cuenta a la frecuencia BASE pase lo que pase. Lo dijo
la propia maquina el 2026-08-16, con sus dos instrumentos a la vez:

```
   [MEDIDO]  reloj base    3700 MHz    el TSC
   [MEDIDO]  reloj ahora   4519 MHz    medido por MPERF/APERF
```

```
   un tick de TSC        = 1 / 3,7 GHz = 0,27 ns
   un tick de TSC        = 1,22 ciclos de nucleo a 4519 MHz
   una puerta = 884 ticks = 240 ns    = ~1.086 ciclos de nucleo
```

**Los dos denominadores, y hay que usar el que corresponde a la medida:**

```
   [MEDIDO]    un nucleo, en TICKS DE TSC     3.700 M ticks/s   <- este
               1% de un nucleo                   37 M ticks/s
   [CATALOGO]  el mismo nucleo, en CICLOS     ~4.500 M ciclos/s
```

[!] Los ratios de este documento no cambian --todo lo medido esta en la misma
unidad-- pero **la etiqueta estaba mal, y una unidad mal puesta es el patron 2
de la casa**: el campo que es un exponente, o que viene en otra unidad. Se
corrige aqui y se deja escrito de donde salio, porque el que lo destapo fue el
panel de la propia maquina.

- **R-CENSO0.** ★ **El numero y su denominador van en la MISMA unidad**, y la
  unidad se escribe. Ticks de TSC contra 3.700 M/s; ciclos de nucleo contra
  ~4.500 M/s. Mezclarlos da un porcentaje que parece razonable y es falso por un
  22%.
- **R-CENSO1.** Un camino que consume **menos del 1% de un nucleo** no se
  optimiza. Se **tacha**, y se anota **el numero por el que se tacho**.
- **R-CENSO2.** El numero que tacha vale **mientras no cambie la frecuencia de
  disparo**. Si algo pasa de 1.000 a 100.000 veces por segundo, vuelve al censo.
- **R-CENSO3.** ★ **Un coste por vez sin veces por segundo NO es un porcentaje**,
  y por tanto no puede ordenar el trabajo. Los dos numeros o ninguno.

---

## 2. Los diez caminos

| # | camino | disparo | veces/s | coste/vez | por segundo | eje | veredicto |
|---|---|---|---|---|---|---|---|
| P1 | **la puerta** | cada `INVOKE` | **sin contar** | 884 [MEDIDO] | ? | LATENCIA | **contrato** |
| P2 | **el pintado** | cada frame | 60 (deseado) | ~98 M [ARITM] | > 1 nucleo | THROUGHPUT | **VIVO, el que manda** |
| P3 | **la consola** | cada `printf` | segun el programa | **2,2 M** [MEDIDO] | 21 printf/s = 1% | THROUGHPUT | **VIVO** |
| P4 | cambio de contexto | tick + ceder | ~1.000 + N | ~2.000 [ESTIM] | ~2 M = 0,04% | LATENCIA | **TACHADO** |
| P5 | la carga de un `.bex` | cada `run` | ~0 | irrelevante | ~0 | TAMANO | **vivo por el TECHO** |
| P6 | el disco | por bloque | rafagas | 100 us+ [SPEC] | -- | THROUGHPUT | sin metro |
| P7 | la entrada | por pulsacion | < 20 | microframe 125 us | ~0 | ninguno | **TACHADO** |
| P8 | el arranque | una vez | 0 | -- | 0 | ninguno | **TACHADO** |
| P9 | el ocio | siempre que no hay nada | -- | -- | -- | ENERGIA | vivo, un dueno |
| P10 | el toolchain | al compilar | 0 en la maquina | -- | 0 | ninguno | **TACHADO (es productor)** |

### P1 -- LA PUERTA | eje LATENCIA | componentes C1, C2, C8

```
   ring0/syscall/{entry,mod,ops,meter,presupuesto}.rs
   ring0/obj/cap.rs                       (resolver el handle)
   platform/abi/bmo-abi/.../handle/opaque.rs
   Ultra_userspace/userland/src/sys.rs    (el lado de Ring 3)
```

Es el unico camino **completo**: doble testigo, juez fuera del metal, tres filas
con techo y meta, guardianes en el build.

[!] ★ **Y sin embargo le falta la mitad de R-CENSO3: nadie ha contado cuantas
puertas por segundo hace el escritorio.** El kernel ya lleva el contador
(`Medida::puertas`), asi que la sonda es leerlo dos veces separadas por un
segundo con el escritorio en marcha. **Es la medida mas barata que queda en todo
el arbol y es la que le pone porcentaje a cinco tandas de trabajo.**

Mientras tanto, la aritmetica condicional:

```
   [ARITMETICA]   10.000 puertas/s x 884  =  8,8 M ticks/s  =  0,24% de un nucleo
                 100.000 puertas/s x 884  =   88 M ticks/s  =   2,4% de un nucleo
```

★ **Por eso este camino queda como CONTRATO y no como cuello de botella.** Se
optimiza porque es el suelo que paga toda operacion de toda aplicacion y porque
es la promesa que BMO-X le hace a quien programe encima --el argumento de
Liedtke con L4--, **no porque la maquina vaya a ir visiblemente mas rapida**.
Decir lo contrario seria vender el trabajo por lo que no es.

### P2 -- EL PINTADO | eje THROUGHPUT | componentes C5, C2, C4, C3

```
   Ultra_userspace/services/gui/          (el compositor)
   platform/shared/bmo-dibujo             (rasterizador y LIENZO)
   Ultra_userspace/userland/src/pantalla.rs, dibujo/
   ring0/obj/fb.rs, ring0/core/splash/lienzo.rs
```

```
   [ARITMETICA]  1600 x 1000 x 4 B      =  6,4 MB por frame
   [MEDIDO]      blit                   ~300 MB/s  ->  21 ms por pantalla entera
   [ARITMETICA]  21 ms a 4,6 GHz        ~98 M ciclos POR FRAME
                 a 60 fps               ~5.900 M ciclos/s  >  UN NUCLEO ENTERO
```

★ **Este es el camino que manda en esta maquina, y no esta ni medido con ventana
limpia ni tiene fila.** Aqui un 10% son ~590 M ciclos/s: **mas de lo que cuesta
la puerta entera aunque el escritorio hiciera 100.000 llamadas por segundo.**

### P3 -- LA CONSOLA | eje THROUGHPUT | componentes C5, C1

```
   ring0/uconsole.rs, ring0/obj/console.rs
```

```
   [MEDIDO]  una escritura de consola  ~2,2 M ciclos   (dibuja glifos + scroll)
   [RATIO]   2.200.000 / 884           = 2.489 puertas peladas
```

★★ **Un `printf` cuesta lo que 2.500 puertas.** Veintiun `printf` por segundo se
comen el 1% de un nucleo; **2.100 por segundo se comen un nucleo entero**. Y no
es una anecdota: este numero ya destrozo una tanda de medida entera y costo un
flasheo (la ventana contaminada del 16-08).

Consecuencia directa para el trabajo diario: **cualquier programa que imprima
dentro de un bucle esta midiendo su propia consola**, no lo que cree medir.

### P4 -- CAMBIO DE CONTEXTO | TACHADO

```
   ring0/plat/{timer,trap,irq}.rs   (xsave64 / xrstor64 en cada entrada)
   ring0/task/scheduler.rs
```

Da miedo --hace `xsave64` de un area de 1 KiB mil veces por segundo-- y por eso
merece el numero:

```
   [ESTIMADO]  1.000 ticks/s x ~2.000 ciclos = 2 M ciclos/s = 0,04% de un nucleo
```

**TACHADO por R-CENSO1.** Y con la nota que lo protege de volver a discutirse:
el `XSAVE` de la puerta se pudo quitar porque **el ABI del syscall dice que
registros son volatiles**; el del timer **no se puede quitar** porque interrumpe
codigo cualquiera. No es la misma pieza aunque se llame igual.

[!] Lo que si queda pendiente y **no** esta tachado es el **vaciado del TLB al
cambiar CR3** (C3/C2), que no se paga en el cambio sino **despues**, en fallos
de TLB del que entra. Hoy no hay forma de medirlo: entra en la lista de C2.

### P5 -- LA CARGA DE UN `.bex` | eje TAMANO | componentes C3, C6

```
   ring0/task/{launch,bex,proc}.rs        MAX_BEX = 4 MiB | pila Ring 3 = 65.536 B
   platform/abi/bmo-abi/src/bef/, platform/abi/bmo-bex-gate
```

Ocurre una vez por lanzamiento: **por ciclos esta tachado**. Sigue vivo **por el
techo**, que es otro eje: aqui no se muere despacio, se muere de golpe --el
compositor no cargaba, y el marco de pila mato el escritorio cinco commits--.

### P6 -- EL DISCO | eje THROUGHPUT | componentes C6, C3, C4

```
   platform/drivers/storage/{ahci,block,fat32,particiones,estratos}
   ring0/fsys/, ring0/obj/file.rs, ring0/dev/
```

Sin metro. Su latencia es del orden de **100 microsegundos o mas** y **no se
arregla con ciclos de CPU**: lo que da caudal es la cola. Optimizar aqui contando
instrucciones es trabajar en la columna equivocada.

### P7 -- LA ENTRADA | TACHADO

```
   platform/drivers/usb/{xhci,input,uhid}, ring0/dev/, ring0/obj/input.rs
   Ultra_userspace/services/input
```

```
   [SPEC]  microframe          125 us   =  ~575.000 ciclos
   [DATO]  un humano rapido    < 20 pulsaciones/s
```

★ **El presupuesto de este camino lo pone el bus, no el CPU: 884 ticks son el
0,15% de un microframe.** Aqui no se optimizan ciclos **jamas**; se cumplen las
cinco reglas R-USB1..5, que son de CORRECCION, y ahi es donde este camino ha
sangrado siempre.

### P8 -- EL ARRANQUE | TACHADO

```
   Ultra_kernel_x86-64/faggin/s1_cpu/, ring0/cpu_vendor/, ring0/plat/madt.rs
   ring0/core/ (init), ring0/cabina/
```

Ocurre **una vez**. Aqui no se optimiza: **aqui se comprueba**. Con una sola
excepcion, y esta ya hecha: `init_pat` decide el rendimiento de todo lo demas
(el framebuffer en WC en vez de UC), asi que **una decision del arranque es
la duena del eje de otro camino**.

### P9 -- EL OCIO | eje ENERGIA | componentes C10, C12

```
   el bucle ocioso, ring0/cpu/power.rs (RAPL), AXION / smp
```

Metro puesto (milivatios de paquete y nucleo). Falta **un antes y un despues de
`smp stop`**, que es lo unico que convierte R-PWR1 en un hecho. Encender nucleos
sigue faltando (MWAIT).

### P10 -- EL TOOLCHAIN | TACHADO como consumidor

```
   toolchain/lang/{c,cobol,ada}, toolchain/forge/, toolchain/tools/
```

No corre en el camino caliente de la maquina. **Pero es el PRODUCTOR de todos
los demas**: el codigo maquina que emite decide el tamano y los ciclos de P1..P9.

★ Por eso el toolchain no tiene eje propio y **tiene el de su salida**: una
decision de codegen se juzga por lo que le hace al camino que ejecuta ese
binario. Es lo que ya paso con `rep movsb` --un `memcpy` byte a byte porque **el
emulador no tenia la instruccion**: una casilla que faltaba en el banco de
pruebas eligiendo el codigo maquina que corre en el Ryzen--.

---

## 2b. ★ EMPEZAMOS POR LA CPU: P1 pieza a pieza, ordenado por uso

Aplicando la regla del multiplicador (`META-KERNEL_HARD.md`, seccion 2): para el
eje CICLOS se ordena **por veces por segundo**. Todo lo que sigue se recorre en
**el 100% de las puertas** salvo donde se diga.

Y al lado va la **generacion** (ley L7), porque es lo que dice que puede y que
no puede saber cada trozo:

| # | pieza | veces | coste | generacion | fila |
|---|---|---|---|---|---|
| 1 | `syscall/entry.rs` -- el stub | **100%** | **785-839 de 884 (89-91%)** | **abuelo** -- no sabe que operacion se pidio | ★ **NINGUNA** |
| 2 | `syscall/mod.rs::dispatch` | 100% | 87 (C) / 104 (Rust) | **padre** -- sabe que puerta, no que objeto | `DISPATCH` 105/60 |
| 3 | `syscall/meter.rs` start+stop | 100% | **69-107 de esos 87-104** | el instrumento | -- |
| 4 | `trap::registrar_publicacion` | 100% | **sin medir** | diagnostico | -- |
| 5 | `invoke()` -- match de 2 brazos | 100% | trivial | padre | -- |
| 6 | `obj/cap.rs::resolve` | **44%** (32 de 72 ops) | **+166** | **hijo** -- sabe que handle | `HANDLE` 355/80 |
| 7 | `obj/*.rs` -- el objeto contesta | segun la op | clase A/B/C/D | **nieto** -- el unico que sabe que significa | -- |

### Las tres cosas que dice esta tabla

★ **1. El 89% del coste esta en el trozo con CERO filas y la generacion mas
baja.** El abuelo es el que mas se usa (100% de las puertas) y del que menos se
sabe: `dispatch` tiene fila, la capability tiene fila, **el stub no**. Y no es
por descuido -- los cuatro sellos que lo partian existieron, contestaron
*(guardar 30, resto 1254, devolver 30)* y **se retiraron el mismo dia por costar
el 17%**. El cableado hasta Ring 3 sigue puesto y `coste.bex` dice `NO MEDIDO`
en vez de imprimir ceros.

★★ **2. EL TERMOMETRO ES DEL TAMANO DEL ENFERMO, y la tanda del 16-08 lo dejo
sin discusion.** Los dos testigos midieron el coste de un `rdtsc` suelto y **no
coinciden**:

```
   [MEDIDO]  Rust   111 - 4  (su bucle de 4)  = 107
   [MEDIDO]  C      112 - 43 (su bucle de 43) =  69
```

No es que uno mida mal: **el CPU es fuera de orden**. En un bucle de 43 ticks el
`rdtsc` se solapa con el trabajo de al lado y sale barato; en uno de 4 no tiene
donde esconderse y sale entero. *"Lo que cuesta una instruccion"* no es una
constante -- **depende de lo que tenga alrededor**, y por eso se dan los dos
numeros y no una media que no describe ninguno de los dos casos.

Consecuencia, y hay que decirla en voz alta: `dispatch` mide **87-104** y su
propio instrumento cuesta **69-107**. O sea que **el trabajo real de Rust dentro
de la puerta esta enterrado debajo de su propio termometro**, y la fila
`DISPATCH` --104 contra techo 105-- esta a UN tick de gritar REGRESION por algo
que no es el codigo.

★ **La fila DISPATCH, hoy, no se puede leer.** La meta de 60 no se alcanza
afinando un `match` de dos brazos: se alcanza **sacando el metro con un `cfg`**.

★ **3. La jerarquia es lo que hace FALSABLE la anomalia viva.** *"Los ~246
ciclos no pueden estar en el stub"* no es una intuicion: el stub es el **abuelo**
y por construccion no sabe que operacion se pidio, asi que un coste que dependa
de la operacion no puede aparecer ahi. Sin L7 eso seria un numero raro; con L7
es una contradiccion, y una contradiccion se puede resolver con una sonda.

### Lo que falta para ordenar de verdad: el histograma por CLASE

Hoy se sabe lo que cuesta cada clase y **no se sabe cuantas veces se pide cada
una**. Sin eso, "donde se usa mas" es una suposicion (R-CENSO3).

```
   [MEDIDO 16-08, sonda de una variable a la vez]
   clase A   pseudo-cap, respuesta inmediata      890    PID, TID, YIELD, MI_PADRE
   clase B   pseudo-cap, camina una tabla         958    INFO, KLOG, CABINA, AUTOPSIA
   clase C   resuelve capability real            1124    las 32 de handle
   clase D   hace trabajo de verdad           ~2,2 M     CONSOLE_WRITE, EJECUTAR, ABRIR

   fila B - fila A =  +68   lo que cuesta una operacion mas gorda
   fila C - fila B = +166   lo que cuesta un HANDLE de verdad
```

[!] **Y esto corrige una cifra de la tanda anterior.** Se habia escrito que el
handle costaba **+217** contra +33 de la operacion, o sea 6,5 veces. Con las tres
filas de la sonda --que cambian **una variable cada vez**-- sale **+166 contra
+68: 2,4 veces**. El handle sigue mandando, pero menos. La resta del testigo C
(1230 - 926 = 304) NO sirve para esto: cambia capability **y** operacion a la
vez, que es el defecto que la sonda de tres filas existe para arreglar.

★ **Cuatro casillas, no setenta y dos.** Un contador por operacion seria peso en
el camino caliente y ademas obligaria al que cuenta a saber que operacion es --
que es justo lo que L7 prohibe. Un contador por **clase** es un `array[clase]++`
de dos o tres ciclos, y contesta la pregunta entera: **si la clase D domina, la
puerta es irrelevante y el trabajo esta en la consola**; si domina la C, la
capability es lo siguiente.

---

## 3. Lo que el censo REORDENA

Puestos en la misma unidad --ciclos por segundo-- y con la misma maquina:

```
   pintar la pantalla entera a 60 fps      ~5.900 M ciclos/s   > un nucleo
   un printf por linea en un bucle de 1k   ~2.200 M ciclos/s   medio nucleo
   la puerta, suponiendo 100.000/s            ~87 M ciclos/s   1,9%
   el cambio de contexto                       ~2 M ciclos/s   0,04%
```

★★ **Tres ordenes de magnitud entre lo mas caro y lo que se estaba midiendo.**

Y hay que decir las dos cosas, no una:

1. **El trabajo en la puerta fue correcto** y no se toca: es el suelo que paga
   toda operacion, es la superficie que BMO-X promete a quien programe encima, y
   es **el unico camino del arbol con juez** -- o sea que ademas de bajar 2618 a
   884, es lo que enseno a medir. Sin el, este censo no se podria escribir.
2. **Pero no es donde estan los ciclos.** Si el objetivo es *"la maquina va
   rapida"*, el orden es P2, P3 y luego todo lo demas.

---

## 4. La tabla de TACHADOS -- lo que ya no hay que mirar

| tachado | por que numero | vuelve al censo si... |
|---|---|---|
| cambio de contexto (P4) | 0,04% de un nucleo | el tick sube de 1.000 Hz, o el cambio se hace mas de 50.000 veces/s |
| la entrada (P7) | 884 ticks = 0,19% de un microframe | jamas; el bus no va a acelerar |
| el arranque (P8) | ocurre una vez | nunca, salvo que el arranque pase a ser interactivo |
| el toolchain (P10) | 0 ciclos en la maquina | nunca como consumidor; siempre como productor |
| el RTC, `bmo-hash` en carga, el censo de extensiones | una vez o casi | si entran en un bucle |
| formatos (BEF, ESTRATOS, particiones) | son declaraciones, no trabajo | si alguien mete calculo en un formato |

★ **Esta tabla es el producto principal del censo.** Seis areas del arbol
quedan fuera de toda discusion de rendimiento **con su numero al lado**, y eso
es lo que hace que la lista de lo que si importa sea corta.

---

## 5. Lo que queda VIVO, en orden

```
   0  CONTAR LAS PUERTAS POR SEGUNDO        el contador ya existe; falta leerlo dos veces
                                            -> convierte P1 en un porcentaje. Media hora

   1  P2 EL PINTADO                         ventana limpia + fila + techo/meta
                                            el unico camino que se come un nucleo entero

   2  P3 LA CONSOLA                         2,2 M por escritura, ya medido y sin fila

   3  C2 LA SONDA DE CACHE                  4 numeros (L1/L2/L3/DRAM) con el metro que hay
                                            desbloquea R-CACHE1 para P2 y P3

   4  P5 EL TRINQUETE DE TAMANO             marco maximo por funcion + techo del .bex

   5  P9 EL ANTES/DESPUES DE `smp stop`     el metro de RAPL ya existe

   6  P6 EL DISCO                           el mas caro de instrumentar, y el que menos
                                            duele hoy
```

---

## 6. Como se marca en el arbol, y como se repite el censo

Cada fichero de un camino **vivo** declara en su cabecera:

```rust
//! [eje]     THROUGHPUT -- paga LATENCIA individual y MEMORIA
//! [camino]  P2 el pintado
//! [exige]   R-FB1, R-FB2, R-CACHE2, R-BUS1
```

Un fichero de un camino **tachado** declara una sola linea, y es igual de util:

```rust
//! [eje]     NINGUNO -- P8, ocurre una vez. Aqui se comprueba, no se optimiza.
```

★ **Declarar el tachado es tan importante como declarar el eje**: sin esa linea,
dentro de seis meses alguien --yo incluido-- va a "optimizar" el arranque.

**Repetir el censo** es rehacer la tabla de la seccion 2 cuando (a) aparezca un
camino nuevo, (b) cambie una frecuencia de disparo, o (c) un `[ESTIMADO]` pase a
`[MEDIDO]`. Los tres son eventos concretos, no un calendario.

---

*Ver `META-KERNEL_HARD.md` para la ley de cada componente y las reglas
`R-<COMP><n>` citadas aqui; `presupuesto.rs` para las tres filas ya vivas de P1.*
