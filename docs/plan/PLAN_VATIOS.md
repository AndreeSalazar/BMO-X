# PLAN_VATIOS -- lo que gasta el CPU en reposo, y por que

> Escrito el **2026-09-11** a partir de la pregunta del dueno: *"analiza todo
> pero enfocado en CPU en consumo de watts, inspirado en MS-DOS que consume
> DRASTICAMENTE bajo; se puede? analiza pero la base claro."*
>
> La respuesta corta: **se puede, y BMO-X ya tiene el instrumento** -- lo que
> le falta es dormir. Hoy la maquina en reposo gasta como si trabajara, y
> este documento dice QUIEN la mantiene despierta, con la linea de codigo.

```text
   [ ]  pendiente        [~]  a medias, y se dice cuanto        [x]  hecho, con fecha
```

---

# 0. LA BASE, sin adornos

## 0.1 Lo que MS-DOS hacia de verdad

MS-DOS **no ahorraba energia**. `COMMAND.COM` esperando una tecla giraba sobre
`INT 16h`; el CPU iba al 100 % sin hacer nada. Lo que ahorraba era **la maquina
de su epoca**: un 386 gastaba 3 W girando o parado. El ahorro de MS-DOS es una
propiedad del silicio de 1990, no del sistema.

Lo que si tenia MS-DOS, y es lo que vale copiar, son DOS cosas:

```text
   1. NO HAY NADIE DETRAS      cero demonios, cero servicios, cero "telemetria".
                               Cuando el usuario no hace nada, NO PASA NADA.
   2. DESPERTAR POR EVENTO     la tecla es una interrupcion. Nadie pregunta
                               mil veces por segundo "hay tecla?": el hardware
                               avisa cuando la hay.
```

Y a partir de DOS 6 (`POWER.EXE`) y Windows 3.x, la tercera: **`HLT` cuando no
hay nada que hacer**. Es el `INT 28h` de reposo: el CPU se para y lo despierta
la siguiente interrupcion.

** BMO-X ya tiene la primera **por construccion**: dos syscalls, sin demonios,
sin nada corriendo que el dueno no haya lanzado. Y NO tiene la segunda ni la
tercera bien hechas. Eso es lo que este plan mide y arregla.

## 0.2 El numero que ya existe, y es el que hay que bajar

`cpu/power.rs` lee los contadores RAPL del Zen 3 (`CORE_ENERGY_STAT`,
`PKG_ENERGY_STAT`, via `cpu_vendor/ryzen_5_5600x/energia.rs`) y `consumo` /
`save` los ensenan. **Se midio dos veces**:

```text
   24-08   12 en pie, once girando (`pause`)   57,7 W paquete   9,9 W nucleo (BSP)
   26-08   `save` en el escritorio             58,5 W           4495 MHz medidos
```

*** **58 W en reposo, sin que nadie haga nada.** Ese es el numero.

[!] **CORREGIDO el 11-09 por la tarde.** Aqui ponia que `4495 MHz en reposo` era
*"el sintoma de que nadie esta en reposo"*. **Falso**: es MPERF/APERF, o sea la
frecuencia MIENTRAS el nucleo ejecuta, y no dice nada del tiempo que duerme.
Windows marca 118 % con un 85 % ocioso. Ver `EFICIENCIA_MAESTRO.md`, 2.1.

[!] Desde el 10-09 los once obreros duermen con `MWAITX` en el C-state mas
profundo que enumera el CPUID (`smp/dormir.rs`). **No se ha medido despues.**
La primera linea del proximo `consumo` vale mas que todo lo que sigue.

## 0.3 Windows es la INSPIRACION, no la vara

[!] **CORREGIDO el 11-09 por la tarde, a peticion del dueno.** Aqui ponia que
Windows en reposo era *"el suelo"* y que BMO-X *"no puede bajar de ahi"*. Las
dos cosas eran falsas:

```text
   el suelo    lo pone el SILICIO (y la placa y el firmware), no un sistema
   Windows     tiene decenas de servicios despiertos. BMO-X no tiene NADA
               detras (0.1), asi que puede quedar POR DEBAJO de Windows quieto
```

** Lo que Windows SI da es la **prueba de que se puede**: en este mismo Ryzen
pasa el 83 % del tiempo ocioso en C2 (medido el 11-09). Eso es inspiracion, no
meta. **La meta es ir tan abajo como deje el silicio**, y el numero que manda es
el que mida el propio BMO-X con todo dormido. HWiNFO sigue valiendo para una
cosa que BMO-X no ve: la grafica.

```text
   [ ] W0a  REFERENCIA, no suelo: Windows quieto 2 minutos, Package Power = ___ W
   [ ] W0b  BMO-X, shell de Ring 0, `consumo` dos veces seguidas:  ___ W
   [ ] W0c  BMO-X, escritorio, `save` dos veces seguidas:          ___ W
```

Tres numeros y una resta cada uno. Sin ellos, lo de abajo es opinion.

---

# 1. QUIEN MANTIENE DESPIERTO AL CPU, con la linea

Leido en el arbol el 11-09. Ordenado por lo que cuesta cada uno.

## 1.1 *** El BSP en reposo hace `sti; hlt`, o sea C1 y nada mas

`task/scheduler/roja.rs:1167`: la tarea idle se aparca con `sti; hlt`. `HLT`
es **C1**: el nucleo deja de ejecutar pero sigue encendido, con reloj y con
sus caches calientes. Los C-states profundos (CC6: el nucleo se APAGA y se
guarda su estado) no se entran con `hlt`; se entran con **`MWAIT` y una
pista**, o por el puerto de E/S que declara ACPI en `_CST` -- y `_CST` es AML,
que esta casa no interpreta (`bmo-placa-firmware`).

** La buena noticia: **el codigo ya existe**. `smp/dormir.rs` enumera los
C-states por `CPUID 5 EDX`, elige el mas profundo que exista y hace `MWAITX`
con plazo. Lo usan los once obreros. **El BSP -- el unico nucleo que de verdad
trabaja y el unico que de verdad descansa -- no lo usa.**

## 1.2 *** El tick es PERIODICO a 1 kHz, haya o no haya nadie

`faggin/s2_mem/src/main.rs:408`: LAPIC en modo periodico, `hz / 1000`, o sea
un tick cada milisegundo. `plat/timer.rs` lo atiende y llama a `on_timer`
**mil veces por segundo, con la maquina vacia**.

Un nucleo en CC6 que recibe una interrupcion cada milisegundo **no esta en
CC6**: entrar y salir de un C-state profundo cuesta decenas de microsegundos y
un pico de corriente, y hacerlo mil veces por segundo es lo que `dormir.rs`
ya llamo *"comer electricidad sin sentido -- con mas pasos"* cuando le paso a
los obreros con su plazo de 0,27 ms. Al BSP le pasa lo mismo y nadie lo ha
dicho.

Lo que el tick hace hoy y habria que seguir haciendo SIN tick:

```text
   on_timer         el quantum del planificador       solo hace falta si hay >= 2 LISTOS
   wait_deadline    los plazos de WAIT                 se sabe CUANDO vence el proximo
   LATIDO 1 kHz     el reloj del escritorio            ver 1.3: es el que sobra
   TICKS            `timer::ticks()` para timeouts     se calcula del TSC
```

Ninguna de las cuatro necesita que el reloj suene cuando nadie lo espera. Es
lo que Linux llama *tickless* desde 2007: **el LAPIC en modo one-shot, armado
al PROXIMO instante en que alguien tenga algo que hacer**, y si no lo hay, no
se arma.

## 1.3 *** El escritorio da MIL vueltas por segundo

`director/src/main.rs:982`: *"el bucle va montado en el LATIDO... PIDE 1.000
vueltas por segundo"*. Cada vuelta son unas nueve puertas (entrada, superficies,
foco, tick...) a 969 ciclos la puerta: ~9 millones de ciclos por segundo, que
en CPU es el 0,2 % de un nucleo -- **pero en vatios es que el BSP despierta mil
veces por segundo aunque el dueno se haya ido a dormir**.

Es exactamente el `INT 16h` de `COMMAND.COM`: preguntar sin parar. Lo que
MS-DOS tenia y esto no: **la tecla LLEGA**. El escritorio deberia dormir en
`WAIT` sobre la ENTRADA -- el esperable existe, `WAIT` sabe bloquear sobre un
handle -- y despertar solo cuando hay una tecla, un movimiento de raton, una
superficie que subio su secuencia, o el plazo del reloj de la barra (que puede
ser de 250 ms, no de 1).

Lo que se paga: el testigo `~1000/s` del ritmo del planificador, que hoy es la
medida de si el turno se reparte bien, deja de existir. Es un instrumento
util; se cambia por otro (cuantas vueltas POR EVENTO, que es mas exacto).

## 1.4 ** El teclado y el raton se SONDEAN, no interrumpen

`obj/input.rs:233` y `dev/usb/mod.rs:390`: cada `INPUT_OP_*` **sondea el anillo
de eventos del xHC** dentro del syscall. No hay MSI, no hay interrupcion del
xHC. Consecuencia: **una tecla solo se ve cuando alguien pregunta**, y por eso
el escritorio TIENE que dar mil vueltas por segundo -- si dejara de preguntar,
el teclado dejaria de existir.

*** Este es el nudo. 1.3 no se puede arreglar sin 1.4: dormir sobre la entrada
exige que la entrada sepa despertar, y hoy no sabe. Es la misma dependencia
que `bmo-latencia-mano-pixel` ya vio por el otro lado (los milisegundos de
latencia estan en los extremos, y el `bInterval` del raton se lee y se ignora).

## 1.5 Las apps que giran

```text
   DOOM (pantalla entera)   `DG_SleepMs` gira sobre `bmo_ceder()` hasta el TSC   100 % de un nucleo
   DOOM (ventana, 11-09)    `bmo_dormir(ms)`: BLOQUEADA                            arreglado
   raycaster (pantalla)     `bmo_ceder()` por fotograma                            100 %
   raycaster (ventana)      `bmo_dormir(16 ms)`                                    bien
   `presta`/`lend_screen`   `wait(0,0,20 ms)`: bloqueada, 50 despertares/s          bien
```

`bmo_ceder()` no duerme: vuelve a la cola LISTO. Un programa que "espera"
cediendo es un programa al 100 %. Ya esta dicho en `<bmo/bmo.h>` y en el
raycaster; lo que falta es que **no haya forma de escribirlo mal**: un
`bmo_esperar_tsc(fin)` que bloquee, y `bmo_ceder` reservado para "tengo
trabajo y le dejo el turno a otro".

## 1.6 La red se sondea (`net::rx_poll`)

`op_maquina.rs:568`. Solo cuando alguien pregunta; hoy nadie pregunta en
reposo. No cuesta vatios hasta que haya una app de red viva. Se apunta y no se
toca.

## 1.7 Lo que YA esta bien, para no volver a mirarlo

```text
   once obreros          MWAITX, C-state mas profundo, plazo 27 ms   `smp/dormir.rs` 09-10  (sin medir)
   AXION `smp stop`      apaga nucleos de verdad                     `bmo-axion-nucleos`
   `lend_screen`         `wait` con plazo, no `yield`                 `main.rs:485`
   el arranque           `hlt` en los bucles de espera                `desktop.rs:239`, `entry.rs`
   nada detras           cero demonios: la propiedad de MS-DOS       por construccion
```

---

# 2. LAS PALANCAS, en orden, y lo que cuesta cada una

Cada una trae su medida ANTES y DESPUES con `consumo`. Una palanca sin las dos
cifras no se da por hecha (L3, LEY 24).

## [x] W1 -- El BSP duerme como los obreros: `MWAIT` con pista -- CODIGO 2026-09-11, metal pendiente

[!] **CORREGIDO el 11-09 por la tarde.** En este Ryzen el C-state mas hondo que
enumera el CPUID es **C1** (`CPUID 5 EDX = 0x11`, leido desde el mismo chip):
W1 no gano profundidad -- el BSP y los obreros duermen en C1, igual que con
`hlt`. Lo que si gano es el despertar por interrupcion y el plazo. La
profundidad de verdad es W6.

Hecho en `smp/dormir.rs::reposo()` y `scheduler/roja.rs::idle_thread`: `sti`,
`monitor` sobre una celda propia, `mwaitx` con `ECX = 3` (la interrupcion
despierta, `EBX` es el plazo de siempre) en el C-state mas profundo que
enumera el CPUID; `sti; hlt` de reserva si no hay `MONITORX`. Se cuenta
`reposos` y `ticks_reposo`, y salen en `consumo` (fila `bsp`) y en `save`
(`bsp dormido`, `bsp reposo`, `bsp siestas`). INFO `0x67`/`0x68`.

`scheduler/roja.rs:1167`, la tarea idle: en vez de `sti; hlt`, el mismo
`mwaitx` de `dormir.rs` con el C-state mas profundo enumerado, `ECX bit 0`
puesto (las interrupciones despiertan aunque esten enmascaradas) y `sti;
hlt` de reserva si no hay `MONITORX`. Es reusar 40 lineas que ya tienen su
semaforo.

```text
   cuesta   NADA nuevo: un camino que ya existe, en otro nucleo
   riesgo   RELOJ -- salir de CC6 tarda mas que salir de C1. El primer syscall
            tras un reposo largo llega ~50-100 us tarde. Se MIDE (perfil)
   gana     el unico nucleo que de verdad esta encendido pasa a apagarse
   mide     W0b antes / despues. Con el tick a 1 kHz la ganancia sera PARCIAL
            -- y ese parcial es la medida de cuanto vale W2
```

## [ ] W2 -- Tickless: el LAPIC en one-shot, armado al proximo plazo

[!] **BAJA AL FINAL, o se descarta (11-09 tarde).** Windows entra en C2 unas
24.000 veces por segundo en este chip y vive ahi: el tick de 1 kHz no es el
muro. Se mira solo si W6 deja un resto que se le pueda atribuir.

`s2_mem` deja el LAPIC en periodico; `plat/timer.rs` pasa a rearmarlo en
one-shot al salir de `on_timer`, con el minimo de: el fin del quantum si hay
>= 2 listos, el `wait_deadline` mas cercano, y el proximo latido pedido. Sin
ninguno de los tres: **no se arma**. `timer::ticks()` pasa a derivarse del
TSC (ya calibrado, `INFO_TSC_HZ`), que es lo que `DG_GetTicksMs` hace desde
agosto en Ring 3.

```text
   cuesta   TAREA: tocar el vector 48, `on_timer`, `ticks()` y los 16 sitios
            que suman ticks para un timeout
   riesgo   RELOJ y SILENCIO -- un plazo que no se arma es una tarea que no
            despierta NUNCA. El "cinturon" es un plazo maximo (250 ms) que
            se arma siempre que haya alguna tarea viva que no sea idle
   gana     el BSP en CC6 se queda en CC6
   pide     W1 primero: sin dormir profundo, quitar el tick no se nota
   mide     W0b antes / despues, y `consumo` dos minutos seguidos
```

## [ ] W3 -- El xHC interrumpe: MSI para el USB

`platform/drivers/usb/xhci`: habilitar MSI (o MSI-X) en el capability PCI del
xHC, un vector en la IDT, y el manejador **drena el anillo de eventos** al
mismo sitio donde hoy lo drena el sondeo. Los `INPUT_OP_*` pasan a leer una
cola ya llena en vez de ir al hardware.

```text
   cuesta   APARATO: es un aparato que hoy funciona y se va a tocar
   riesgo   AJENO -- el xHC es de MSI (la placa), y un MSI mal programado se
            ve como "el teclado dejo de existir". ESPEJO -- si el manejador y
            el sondeo drenan a la vez, se pierden eventos: el sondeo se QUITA
   gana     la tecla LLEGA. Es lo que hace posible W4, y de paso baja la
            latencia mano-pixel por el extremo que `bmo-latencia-mano-pixel`
            senalo
   mide     el testigo E6 del bus (`EL_TECLADO_EXIGE.md`), y que `bInterval`
            del raton por fin se respete
```

## [~] W4 -- El escritorio duerme sobre la ENTRADA, no sobre el reloj -- la mitad barata, 2026-09-11

[!] **Y LA MITAD BARATA NO ENTRA NUNCA (11-09 tarde).** `will_paint` incluye
`tick.quarter`, que se enciende cada 250 ms (`director/src/main.rs:781`), asi
que `quietas` vuelve a 0 cada ~250 vueltas y no llega nunca a las 500 del
reposo. Compila y no hace lo que dice.

```text
   [x] W4b  el cuarto de segundo pinta pero NO cuenta como actividad: `desktop/tick/roja.rs`
            CODIGO 2026-09-11 (`Tick::actividad`), metal pendiente: la barra
            tiene que decir `reposo` con todo quieto
```

**Hecho el REPOSO** (`desktop/tick/roja.rs::ceder`): tras 500 vueltas seguidas
sin nada que pintar (`will_paint` reune tecla, raton, superficie y el cuarto
de segundo), el bucle deja el latido y duerme 8 ms por vuelta: de 1.000 a
~125 vueltas por segundo. La primera vuelta que pinta lo devuelve a 1.000. La
barra dice `reposo` y no dispara la alarma de ritmo bajo. Es el principio que
pidio el dueno: *"si no hace nada, no consume; si esta activo, consume"*.

**Lo que falta** es la otra mitad: dormir SOBRE la entrada (cero vueltas
hasta que algo llegue), y esa pide W3. Con el reposo, la tecla tras un rato
quieto se ve como mucho 8 ms tarde; con W3+W4 se veria al instante y sin
vueltas.

`director/src/main.rs:982`: `dsk.tick.ceder()` deja de esperar el LATIDO y
pasa a `WAIT` sobre la entrada (o sobre un esperable que la entrada, las
superficies y el reloj de la barra senalen), con plazo de 250 ms para el
reloj y los testigos. **Mil vueltas por segundo pasan a ser cuatro, mas una
por cada cosa que de verdad pase.**

```text
   cuesta   TAREA en el DIRECTOR + DATO: el esperable de entrada tiene que
            SENALAR, y hoy la entrada no senala porque no interrumpe (W3)
   riesgo   RELOJ -- todo lo que hoy se repinta "en la vuelta" (cursor,
            testigos de la barra, animaciones) pasa a repintarse por evento
            o por el plazo de 250 ms. Lo que dependa de la vuelta se nota
   gana     el escritorio deja de ser el `INT 16h` de COMMAND.COM
   pide     W3
   mide     W0c antes / despues. *** Es la que deberia cerrar la distancia
```

## [x] W5 -- DOOM espera DORMIDO -- 2026-09-11 (`DG_SleepMs` sobre `bmo_dormir`, en todos los modos)

No hizo falta funcion nueva: `bmo_dormir` ya era `WAIT` con plazo. El bucle
de `bmo_ceder` sobre el TSC se fue; DOOM entre tic y tic ya no ocupa un
nucleo. El raycaster en pantalla entera sigue cediendo por fotograma, y esta
bien: esta DIBUJANDO, y lo que trabaja consume.

`<bmo/bmo.h>`: una funcion que BLOQUEA hasta un instante del TSC (sobre
`WAIT` con plazo), y DOOM/raycaster en pantalla entera la usan. Cierra 1.5.

```text
   cuesta   NADA: diez lineas de cabecera y dos sitios que las llaman
   riesgo   ninguno nuevo: es `bmo_dormir` con otro reloj
   mide     `consumo` con DOOM en pantalla entera parado en el menu
```

## [ ] W6 -- C2/CC6 por el puerto de E/S del C-state: `C001_0073`, sin AML

La palanca grande, y no estaba en la primera version de este plan. MWAIT en este
Ryzen solo llega a C1; Windows vive en C2, y ese C2 sale del **puerto de E/S de
C-state** que la BIOS programa en el MSR `C001_0073` (CStateBaseAddr, segun el
PPR de AMD). Leer ese puerto duerme el nucleo en CC6. Y con los doce en CC6 el
paquete puede bajar a PC6, que es donde la RAM entra en autorrefresco.

```text
   cuesta   MAQUINA si el nucleo no vuelve. ARAT = 1 en este chip: el tick del
            APIC sigue contando en CC6, asi que el despertador no se pierde
   riesgo   SILENCIO -- una direccion mal leida no da fault: da un nucleo en C1
            creyendose en CC6. Por eso primero se LEE `C001_0073` y `C001_0296`
            en el metal, se apunta, y solo despues se usa
   gana     la unica profundidad que este silicio da
   sitio    `plat/smp/dormir.rs` (APAGA, L6h), y el MSR en el perfil del Ryzen
```

## [x] W7 -- Lo que no se ve, no se pinta: la VISTA en el buzon -- CODIGO 2026-09-11, metal pendiente

Idea del dueno. El DIRECTOR decide en cada vuelta si se ve cada ventana
(`bmo_golpe::vista`, con pruebas) y lo deja en el byte 2 del estado del buzon;
DOOM (`screenvisible`) y `ray.bex` se saltan el dibujo entero cuando no. La que
sigue pintando oculta se acusa. Regla: R-APP8 de `META-APP_HARD.md`.

```text
   [~] W7b  "tapada por otra ventana": HECHO el caso de PANTALLA COMPLETA
            (11-09, `Vista::Tapada`). El solape corriente sigue abierto, y
            pide geometria de verdad -- `scene/surface.rs`
   [ ] W7c  el metal: `consumo` con DOOM a la vista y minimizado -- `docs/metal/`
```

## Lo que NO se hace, y por que

```text
   un "governor" de frecuencia    el SMU del Zen 3 ya baja el reloj cuando los
                                  nucleos duermen de verdad. Primero dormir; si
                                  despues de W1-W4 el reloj sigue en 4,5 GHz en
                                  reposo, ENTONCES se mira `PstateCtl`. No antes
   leer `_CST` de ACPI            es AML. La pista de MWAIT sale del CPUID, que
                                  es estatico, y basta
   estimar un numero              LEY 24. Los tres numeros de W0 o nada
   tocar los obreros              ya duermen. Se MIDE lo que ganaron el 10-09
                                  y se deja en paz
```

---

# 3. EL ORDEN, y por que no es el de las ganancias

```text
   W0   medir tres veces           sin tocar nada     la base
   W1   MWAIT en el BSP            HECHO 11-09        barato, seguro, se nota
   W5   DOOM duerme                HECHO 11-09        barato, cierra las apps
   W4   reposo del escritorio      la mitad, 11-09    1000 -> 125 vueltas/s en vacio
   W3   MSI para el xHC            un aparato         *** el nudo: sin el, W4 no se cierra
   W4   escritorio por evento      el DIRECTOR        la que cierra la distancia
   W2   tickless                   el reloj           la ultima: es la mas ancha
                                                       y solo vale con todo lo demas
```

W2 va la ultima a proposito: quitar el tick con el escritorio dando mil
vueltas por segundo no ahorra nada, porque el escritorio ES un tick. Y W3
antes que W4 porque un escritorio que duerme sobre una entrada que no
despierta es un escritorio sin teclado.

> Un sistema no ahorra energia por tener pocas cosas. Ahorra cuando **lo poco
> que tiene sabe quedarse quieto** -- y hoy lo que tiene BMO-X pregunta mil
> veces por segundo si hay algo que hacer. MS-DOS tambien; lo que no tenia
> MS-DOS era un contador de julios para saberlo.
