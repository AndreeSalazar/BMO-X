# EL TECLADO EXIGE

> Capitulo de componente, en la forma de `META-KERNEL_HARD.md`: no *"que hace
> BMO-X con el teclado"* sino **que exige el teclado de quien quiera leerlo**.
>
> Escrito el **2026-08-17** con el sintoma delante, dicho por el dueno:
> *"funciona un rato y se muere, y arranco desde Windows; el teclado sufre mas,
> el raton no"*.

---

## 0. Por que este componente tiene capitulo propio

Porque es el unico que puede dejar la maquina **sin dueno**. El escritorio no
tiene salida --al shell de Ring 0 no se vuelve-- asi que un teclado mudo no es
un periferico averiado: es una maquina que ya no es de nadie.

### ** Y ESTO NO ES UN PROBLEMA DE CICLOS. Hay que decirlo primero

El censo TACHO este camino (P7) y con su numero:

```
   [SPEC]   microframe                125 us  =  ~462.000 ticks de TSC
   [MEDIDO] una puerta del sistema        884 ticks  =  0,19% de un microframe
   [DATO]   un humano rapido              < 20 pulsaciones por segundo
```

**El presupuesto de este camino lo pone el bus, no el CPU.** Aqui no se optimizan
ciclos jamas; lo que se cumple son las seis exigencias de abajo, que son de
CORRECCION. Y es donde este componente ha sangrado siempre.

---

## 1. Lo que ES un teclado USB, y de ahi sale todo lo demas

Cinco hechos del aparato y del controlador. Ninguno es opinable:

1. **Es un endpoint de INTERRUPCION.** No avisa cuando pasa algo: **contesta
   cuando le preguntan**, cada `Interval`. Si nadie pregunta, no hay teclas --
   aunque el aparato este perfecto y encendido.
2. **El `Interval` es un EXPONENTE**, `2^(n-1) x 125 us`, no milisegundos. Es el
   campo que costo un teclado programado a **35 minutos** entre sondeos, con
   Configure Endpoint devolviendo EXITO.
3. ★★ **El evento ES EL PERMISO para volver a encolar.** Perder un evento de un
   endpoint de interrupcion **no pierde una pulsacion: para la bomba PARA
   SIEMPRE**, dejando el endpoint en `Running` y sin un solo error.
4. **El anillo de eventos es UNO para todo el controlador.** Compleciones de
   comando, informes del teclado, del raton y cambios de puerto salen por el
   mismo sitio. Lo que uno saca y no es suyo, se lo quita a su dueno.
5. **Un endpoint puede quedarse PARADO** (`Halted`) por un error del bus, y
   **el xHC ignora el timbre de un endpoint parado**. Desde fuera se ve
   exactamente igual que un aparato desenchufado.

---

## 2. Las seis exigencias

Cada una: **que exige** / **que pasa si no** / **como esta hoy** / **el numero
que lo dice**.

### E1 -- Que alguien pregunte SIEMPRE, aunque nadie escriba

**Exige:** una bomba que no dependa de quien tenga la entrada.

**Si no:** el bus solo avanza cuando un programa pide una tecla. En cuanto ese
programa se entretiene --pintar un frame gordo, cargar un WAD-- el teclado
enmudece, y parece la maquina colgada. No estaba colgada: **esperaba a que el
secuestrador preguntara la hora.**

**Hoy: HECHO.** `dev/usb/bus.rs` -- hilo de kernel propio, prioridad 2, una
vuelta cada **4 ms (250 Hz)**, durmiendo entre vueltas y no girando.

[!] **Con una condicion previa que hay que conocer**: el hilo solo arranca si
`PRESENT`, y `PRESENT` solo se pone si la enumeracion encontro algo. Si el xHCI
falla al arrancar, **no hay hilo** y el teclado vuelve al bug de arriba.

**El numero:** `bus_stats().0` subiendo. Vigilado por `cabina/watch.rs`, que
grita `el hilo del bus DEJO DE LATIR`.

### E2 -- Que su evento no se lo quede otro

**Exige:** que de la cola compartida, lo que no es mio **se aparque, jamas se
tire**.

**Si no:** ver el hecho 3. El endpoint enmudece sin un solo error.

**Hoy: HECHO.** Aparcadero de **64 plazas** en `bmo-xhci`, con dos contadores.

**El numero:** `evt_park_stats()` -> `(aparcados, PERDIDOS, ahora)`. El propio
codigo lo deja escrito: *"lo segundo tiene que ser cero; si un dia no lo es, hay
que subir el tope"*.

### E3 -- Que si se para, se le resucite

**Exige:** dos comandos, **en este orden** (xHCI 4.6.8):

```
   1. Reset Endpoint       Halted -> Stopped.  Sin esto, lo demas no vale.
   2. Set TR Dequeue       decirle POR DONDE seguir.
   3. <- el llamante encola y toca el timbre
```

**Si no:** `rearmar()` encola y toca el timbre para nada. El paso 2 es el que se
olvida y el que hace que *"el reset no sirviera de nada"*: resetear sin
recolocar deja el endpoint listo para leer TRBs viejos con el ciclo cambiado.

**Hoy: HECHO y cableado** -- `bmo_uhid` mira `cc_halta_endpoint(cc)` (Babble 3,
Transaction Error 4, Stall 6) y llama a `recuperar_endpoint`.

**El numero:** `(RECUPERACIONES, RECUPERACIONES_FALLIDAS)`.

### E4 -- Que se le pregunte a SU ritmo

**Exige:** el `Interval` del Endpoint Context escrito como exponente, no como el
`bInterval` crudo del descriptor --que en Low/Full Speed viene en milisegundos--.

**Hoy: HECHO.** Fue el bug de los 35 minutos.

**El numero:** el intervalo programado, que CABINA imprime al enumerar.

### E5 -- ★ Que se le pueda ENCHUFAR Y DESENCHUFAR

**Exige:** tres cosas distintas, y las tres hacen falta:

```
   a) enterarse       el aviso del propio bus (cambio de puerto)
   b) reparar         adoptar lo enchufado / SOLTAR lo desenchufado
   c) una RED         un barrido que compare puertos reales contra lo que el
                      driver cree, por si el aviso se perdio
```

**Si no:** un descubrimiento de UN SOLO INTENTO pierde lo que no estuviera
enganchado en ese instante **hasta el siguiente reinicio**. Y sin (b), un teclado
desenchufado **sigue contando como presente y no vuelve jamas**.

**Hoy: HECHO, las tres.** El aviso reactivo se atiende en el mismo bombeo en que
llega (4 ms); `atender_desenchufe` libera el puerto, **le devuelve los intentos**
y suelta el aparato; y el barrido de red corre cada **500 ms**.

[!] Y las dos guardas que no son opcionales, porque responder a un evento del
hardware con una accion sobre ese mismo hardware **regenera el evento**: no tocar
lo que ya funciona (estado) **y** un tope de intentos (contador). Una sola no
basta -- sin la primera se mata lo bueno, sin la segunda se gira para siempre.

**El numero:** `barrido_stats()` -> `(barridos, los que repararon algo)`. Si el
primero sube y el segundo no, el bus esta sano.

### E6 -- ★★ Que la averia se VEA

**Exige:** que cuando el teclado muera, el dueno lo sepa **sin buscarlo**.

**Si no:** las cinco exigencias de arriba pueden estar cumplidas, tener sus
contadores, ser vigiladas... y el dueno sigue viendo *"el teclado no responde"* y
nada mas.

**Hoy: HECHO, y sin verificar en metal** (2026-08-17). Los avisos que ya habia
siguen donde estaban --se dicen una vez, deduplicados, y hacen bien-- y encima de
ellos hay ahora un **estado**:

```
   dev/usb/salud.rs      la foto la saca el BOMBEO, 250 veces por segundo
   INFO_USB_SALUD        bits: hay xHCI / kbd adoptado / ENCOLADO / Running
                         + la EDAD DEL LATIDO del hilo, en ms, en 16..31
   INFO_USB_AVERIAS      los cuatro contadores que tienen que ser CERO
   scene/testigo.rs      la LUZ, en la barra, al lado de la ficha de CABINA
```

★ **La foto se saca en el bombeo y no al preguntar**, y no es un detalle de
implementacion: leer el estado de un endpoint recorre el Device Context y
`USBSTS` es MMIO, **y las dos cosas solo estan mapeadas en el PML4 del kernel**.
Un `OP_INFO` llega con el CR3 del que pregunta; mirar el hardware ahi seria un
`#PF`.

[!] **Y eso abre una trampa que hay que cerrar de frente**: una foto que se saca
en el bombeo **se congela si el bombeo muere**, y entonces el ultimo valor bueno
contestaria *"todo bien"* para siempre -- justo el dia malo. Por eso la edad del
latido viaja **pegada a los bits y se calcula al preguntar**: es lo unico de esa
palabra que envejece solo, o sea lo unico que puede delatar al que la escribe. Un
informe de salud que no puede caducar es un informe que miente.

★ **Y la luz se pinta SIEMPRE, tambien en verde.** Una luz que solo aparece
cuando hay averia no se distingue de una luz que no funciona: si la primera vez
que se ve es el dia malo, lo que dice no se puede creer. Es la misma razon por la
que la ficha de CABINA esta siempre, escrita con otras palabras.

> ★★ **LA REGLA: UNA AVERIA VIVA ES UN ESTADO, NO UN EVENTO.**
>
> Un `fault()` informa a quien ya estaba mirando. Una averia que **sigue
> ocurriendo** necesita un indicador que siga encendido mientras dure, y en el
> sitio donde vive el dueno --el escritorio--, no en un log que hay que abrir.
>
> *"El bus no late"* no es una noticia: es una **condicion**. Y una condicion se
> pinta como una luz, no como un renglon que pasa.

Es el patron 33 con una vuelta mas: alli el motivo salia por un canal cerrado;
aqui sale por uno abierto **pero una sola vez**. Y no es solo del teclado --
`sin RAPL`, `disco no listo` y `fugas > 0` son estados contados como eventos.

---

## 3. La asimetria teclado/raton es un INSTRUMENTO, y es gratis

El dueno lo dijo asi: *"el teclado sufre mas, no mi raton"*. Eso no es una queja:
**es media busqueda hecha**, porque descarta todo lo que afectaria a los dos por
igual.

```
   descartado por la asimetria       el hilo del bus (bombea los dos)
                                     el CR3 del MMIO (mismo camino)
                                     la enumeracion (los dos enumeraron)
   compatible con la asimetria       algo POR ENDPOINT: parado, o su evento
                                     perdido
```

★ **Y hay un motivo fisico para que el que caiga sea el teclado y no el raton:**

```
   un raton moviendose   informa CADA intervalo -- cientos de eventos/segundo
   un teclado            informa cuando pulsas  -- unidades por segundo
```

Con un aparcadero de 64 plazas, **el que lo llena es el que mas habla**; el que
pierde su plaza es el que menos. Y como el evento ES el permiso para reencolar,
al teclado le basta perder **uno** para morir del todo, mientras el raton tiene
cientos detras para recuperarse.

[!] Eso es una **hipotesis con un numero que la confirma o la mata**, no una
conclusion -- este camino ya se ha razonado mal dos veces. La mata o la confirma
`evt_park_stats().1`.

---

## 4. El cuadro de mandos del teclado

Cinco numeros. Entre los cinco dicen **cual** de las seis exigencias fallo:

| numero | sano | si no lo esta |
|---|---|---|
| `bus_stats().0` | subiendo | E1: la bomba murio o nunca arranco |
| `evt_park_stats().1` (perdidos) | **cero** | E2: la cola se come eventos -> endpoint muerto |
| `RECUPERACIONES_FALLIDAS` | cero | E3: se intento resucitar y no salio |
| `RECUPERACIONES` | sube cuando muere | E3 funcionando: hay errores de bus pero se reparan |
| `barrido_stats().1` | cero en reposo | E5: la red esta reparando lo que el aviso perdio |

**Y la lectura combinada, que es lo que hay que hacer con el sintoma de hoy:**

```
   perdidos > 0                    -> E2. Subir el tope y drenar por bombeo
   RECUPERACIONES sube y sigue mudo -> E3: se resucita y se vuelve a parar
   RECUPERACIONES_FALLIDAS > 0     -> E3: la secuencia no completa
   todos limpios y sigue mudo      -> falta una septima exigencia. Escribirla aqui
```

### ★ Y los cinco se leen ya DESDE EL ESCRITORIO, que es donde vive el dueno

Desde el 2026-08-17 no hace falta el shell de Ring 0 --al que no se vuelve-- ni
que el teclado funcione para poder mirarlos:

| donde | que da | quien pinta |
|---|---|---|
| la **luz** de la barra | el veredicto en una palabra, encendido mientras dure | `scene/testigo.rs` |
| la orden **`info`** | los cinco numeros, con su E al lado, y **se puede guardar** | `commands/reports.rs`, seccion `teclado y raton` |

La pareja no es redundante: **la luz contesta SI, `info` contesta CUAL.** Un
testigo tiene que caber en una palabra o deja de leerse de un vistazo; el
diagnostico son cinco numeros. Y `info` se vuelca a `data\`, o sea que el dia
malo se puede mandar por escrito en vez de contar de memoria.

Lo que dice la luz, y en este orden --de fuera hacia dentro, porque **el orden ES
el diagnostico**--:

```
   SIN BUS USB      gris    no hay xHCI: no hay bus que mirar
   xHC MUERTO       rojo    USBSTS dice HSE/HCE. Lo demas es ruido
   BUS PARADO       rojo    E1: el hilo no late hace >100 ms (late cada 4)
   SIN TECLADO USB  rojo    E5: adoptado ya no esta. Desenchufado?
   TECLADO PARADO   rojo    sin encolar, o endpoint fuera de Running
   EVT PERDIDOS n   ambar   ** E2: LA HIPOTESIS DE ABAJO, CONFIRMADA
   NO RESUCITA n    ambar   E3
   REPARADO xn      ambar   E3: hay errores de bus, pero se reparan
   AVISOS PERD n    ambar   E5: el barrido esta tapando avisos perdidos
   TECLADO          verde   todo lo comprobable, comprobado
```

Mirar primero el teclado daria *"teclado parado"* cuando lo que se paro fue el
hilo -- un diagnostico que manda a mirar el aparato equivocado.

---

## 5. Lo que falta para que sea "como Windows"

El dueno lo pidio asi: *"SIEMPRE estar abierto para cuando desconecte mi teclado
o conecte con el USB, como Windows, para escribir basicamente"*.

De las seis, **las seis estan puestas** desde el 2026-08-17. Enchufar y
desenchufar ya funcionaba por diseno --aviso reactivo en 4 ms, red de 500 ms, y
el desenchufe libera puerto, intentos y aparato--; lo que faltaba era enterarse,
y eso es E6.

Queda **una consecuencia suya, sin comprobar**:

```
   que el propio escritorio sepa RE-RECLAMAR la entrada cuando el teclado
   vuelve. Hoy se suelta el aparato al desenchufarlo; queda comprobar que al
   volver, quien tenia la entrada la recupera sin que nadie relance nada.
```

[!] Esta **sin comprobar**, no dado por bueno. Es una fila escrita desde el punto
de vista del que mira la pantalla: *"desenchufar el teclado con el escritorio
abierto y volverlo a enchufar debe dejar escribir en la caja de Ejecutar sin
tocar nada mas"*.

### ★ Lo que hay que hacer en el Ryzen, y lo que descarta cada resultado

Nada de esto lo ha ejecutado un CPU todavia: **compila y esta razonado, y eso no
es lo mismo que funciona.**

```
   1. arrancar y mirar la barra, a la derecha
      TECLADO en verde       -> la luz vive y el bus esta sano AHORA
      cualquier otra cosa    -> ya esta dicho el problema, sin abrir nada

   2. escribir `info` y leer la seccion `teclado y raton`
      los cinco en su sitio  -> el cuadro de mandos entero es alcanzable
                                desde donde vive el dueno. Guardalo.

   3. DESENCHUFAR el teclado y mirar la luz (~1 s)
      SIN TECLADO USB        -> E5 (b) hace lo que dice, y la luz reacciona
      TECLADO en verde       -> el desenchufe no se atiende: mirar E5

   4. VOLVER A ENCHUFARLO
      vuelve a verde y ESCRIBE  -> las seis exigencias, cerradas
      vuelve a verde y NO escribe -> es la fila de arriba: el bus lo readopto
                                pero la ENTRADA no volvio a su dueno. Eso no
                                es del teclado: es del escritorio.

   5. usarlo hasta que se muera, y entonces mirar la luz SIN tocar nada
      EVT PERDIDOS n         -> ** la hipotesis de la seccion 3, CONFIRMADA
      TECLADO PARADO         -> E3: se paro y la resurreccion no lo trajo
      BUS PARADO             -> E1: el hilo del bus, no el teclado
      TECLADO en verde       -> ninguna de las seis. Falta la septima, y hay
                                que escribirla aqui.
```

Ese ultimo caso es el unico que deja el problema abierto -- y aun asi **habria
avanzado**: descartaria las seis de golpe, que hoy no se puede hacer.

---

## 6. Lo que este documento NO afirma

Que el teclado del 17-08 muera por E2. Eso lo dice `evt_park_stats().1`, y hasta
que ese numero se lea, **la causa esta sin determinar**. Lo que si afirma es que
las cinco primeras exigencias tienen su contador, y que entre los cinco no queda
sitio para una causa muda.

---

*Ver `META-KERNEL_HARD.md` C7 (USB) para las reglas R-USB1..5 que este documento
desarrolla, y `docs/CENSO_DE_EJES.md` P7 para por que este camino esta tachado
del eje de ciclos.*
