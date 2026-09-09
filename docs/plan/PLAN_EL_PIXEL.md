# PLAN EL PIXEL -- las reglas de lo que se pinta, y donde estan los milisegundos

> Escrito el **2026-09-09**, el dia que se rompio el cuello de botella del
> volcado y quedo claro que **el compositor ya no era el problema**.
>
> El dueno lo pidio asi: *"que sea mas como real time, que sea en streaming,
> 0 ms posible, y nuevas reglas de que eso es por pixeles"*.
>
> Este documento es las dos cosas: **donde estan de verdad los milisegundos**, y
> **las reglas que rigen un pixel** en esta casa.

---

# 0. LO PRIMERO, PORQUE CAMBIA LA PREGUNTA

Hasta el 09-09 la pregunta era *"por que tarda tanto en pintar"*. Se contesto:
`volcar` y `rect` movian pixeles con `write_volatile`, que le **prohibe** al
compilador convertir el bucle en `rep movsb`; y `glifo` gastaba **68 veces mas
memoria en apuntar el trabajo que en hacerlo**. Ep. 52 de `BITACORA.md`.

★★ Con eso arreglado, **el compositor aporta menos de un milisegundo** a un
fotograma normal. Y entonces la pregunta correcta deja de ser *cuanto tarda en
pintar* y pasa a ser **cuanto tarda desde que muevo la mano hasta que lo veo**.

Son preguntas distintas: la primera es CAUDAL, la segunda es LATENCIA. Un motor
grafico se juzga por la segunda.

---

# 1. ★★★ EL PRESUPUESTO DE LA MANO AL PIXEL

```text
   [1] el raton se mueve
        |
        |  el aparato postea su informe cuando quiere: su `bInterval`
        |  (muchos ratones piden 1 ms) -- y el xHC lo recoge en su anillo
        v
   [2] el HILO DEL BUS drena el anillo            <= 4 ms      BUS_PERIOD_MS
        |
        v
   [3] el compositor se entera                    <= 1 ms      el LATIDO, 1 kHz
        |
        v
   [4] pinta y vuelca                             < 1 ms       (desde el 09-09)
        |
        v
   [5] el ESCANER de video lo ensena              <= 16,7 ms   y sin V-Sync
```

## Lo que dice este presupuesto, y es incomodo

```text
   lo que pone BMO-X de su parte    < 1 ms   de ~22
   lo que ponen los DOS EXTREMOS    ~21 ms
```

★★ **Optimizar mas el compositor ya no compra nada medible.** Los milisegundos
estan en el aparato de entrada y en el escaner de video, y ninguno de los dos es
codigo del compositor.

> El compositor dejo de ser el cuello el 09-09. Seguir apretandolo es apretar la
> pieza que ya no aprieta.

---

# 2. ★★ QUE QUIERE DECIR "0 ms", DICHO SIN VENDER NADA

**Cero es imposible y hay que decirlo**: un pixel tiene que viajar por PCIe y el
panel se refresca a su ritmo. Lo alcanzable es **que BMO-X no anada nada a lo que
el hardware ya cuesta**. Eso tiene tres nombres concretos:

```text
   [2] -> 1 ms     que el hilo del bus lata al ritmo que pide el APARATO
   [3] -> ~0       que el compositor despierte por el EVENTO y no por el reloj
   [5] -> ~0       V-Sync de verdad: volcar cuando el escaner no esta mirando
```

## Y "streaming" tiene un significado exacto aqui

No es *"ir mas rapido"*: es **no acumular**. Un sistema que va en streaming no
tiene una cola de fotogramas esperando; produce uno cuando hace falta, y el
siguiente evento entra en el siguiente fotograma y no en el tercero.

★ **Eso BMO-X ya lo hace por construccion, y es su mejor propiedad de tiempo
real**: no hay cola de comandos, no hay triple bufer, no hay compositor de
terceros en medio. Se pinta y se vuelca en la misma vuelta. **La latencia que
tiene es la del hardware, no la de una tuberia.** Un escritorio moderno pone
dos o tres fotogramas de cola sobre eso.

---

# 3. LAS REGLAS DEL PIXEL

Cada una con su estado real, y el estado importa mas que la regla.

### X1 -- un pixel que se pinta con el color que ya tenia es DESPERDICIO

`[ ] PENDIENTE, y probablemente no se hace nunca.` Comprobarlo cuesta una
LECTURA por pixel, y leer es lo caro. La regla se cumple **por arriba** --no
repintando regiones enteras, X2-- y no pixel a pixel. Queda escrita para que
nadie la implemente por el lado malo.

### X2 -- lo que no cambia no se marca

`[x] HECHO el 09-09`, y con pieza: `scene/huella.rs`. Era una **costumbre de un
fichero** --`testigo` lo hacia desde agosto-- y sus dos vecinos de barra, escritos
despues, repintaban 650 px de ancho en cada fotograma sin que nadie lo decidiera.

★★ Y trae su excepcion, que es lo que la convierte en regla por **L4**: **el
pulso NO la lleva.** Su aguja es la prueba de vida del bucle, y un instrumento de
vida que se calla cuando no cambia nada se calla justo cuando el bucle se muere.

### X3 -- el papeleo de un pixel no puede costar mas que el pixel

`[x] HECHO el 09-09.` `glifo` marcaba pixel a pixel y `marcar` copia `Sucias`
(136 B) dos veces por llamada: **19,3 KiB pintados contra 1.315 KiB de papeleo,
68 a 1**. Ahora marca la celda una vez.

★ La leccion que deja: **un carril VERDE no es un carril barato.** El color dice
lo que arriesgas al TOCARLO, no lo que cuesta EJECUTARLO.

### X4 -- una copia obligada cuesta UNA instruccion

`[x] HECHO el 09-09.` `rep movsb` y `rep stosd`, verificado en el desensamblado:
2 y 172 sitios, todos con su `cld`.

### X5 -- el escritorio EN REPOSO no mueve ni un byte a la pantalla

`[~] CASI.` Con X2 puesta, un fotograma sin novedades no ensucia nada y `volcar`
vuelve por su `sucias.vacia()`. Lo que queda fuera es el pulso (X2, excepcion) y
el parpadeo del cursor de escritura. **Y esta bien que queden**: los dos son
prueba de vida. Un escritorio literalmente inmovil no se distingue de uno muerto.

### X6 -- la latencia se DECLARA, no se descubre

`[ ] PENDIENTE, y es el escalon P1 de` [`PLAN_EL_PLAZO`](PLAN_EL_PLAZO.md). Hoy
no hay ni un plazo escrito en ningun sitio: hay un presupuesto (seccion 1) y
nadie lo juzga. **Un eje sin juez es prosa** -- L0, la primera ley de la casa.

### X7 -- ningun instrumento del camino del pixel vive donde no se mira

`[x] ARREGLADO el 09-09, y era el SEPTIMO.` `ritmo()` y `peor_trabajo()` median
el hilo del bus desde hacia semanas y **los leia solo `cabina/cockpit.rs`, una
pantalla de Ring 0** -- de donde no se vuelve. Ahora suben por `INFO_USB_RITMO` y
se ven en la barra (`scene/entrada.rs`).

```text
   CABINA               se veia solo con una tecla
   el testigo del USB   igual
   el pulso             solo dentro de la ventana de CPU
   el volcado           solo con la orden `mem` del shell
   el modo del lienzo   solo por la consola del arranque, que se tapa
   `cuerpo`             medido y sin leer
   el ritmo del bus     solo en Ring 0            <- el septimo
```

> Un instrumento al que hay que ir no se mira. El que esta delante, si.

---

# 4. LA ESCALERA, Y AHORA VA POR LOS EXTREMOS

## Y1 -- que el bus lata al ritmo que pide el aparato `[ ]`

`BUS_PERIOD_MS = 4` es una constante, y su comentario razona sobre un **teclado**
boot (*"pide que se le sondee cada 8-10 ms"*). El aparato que decide la latencia
que se NOTA es el **raton**, y muchos piden 1 ms.

★★ Y lo contraintuitivo: **el `bInterval` ya se lee.** `uhid/enumera.rs` lo saca
del descriptor, se lo pasa al Endpoint Context del xHC y lo escribe en el log. El
controlador esta programado al ritmo del aparato; **el hilo que drena el anillo
no se ha enterado.**

```text
   [ ] Y1.1  subir el `bInterval` del raton a Ring 0 y a Ring 3. Sin ese
             numero, bajar el periodo es adivinar -- y LEY 24 dice que el
             hardware se PERFILA
   [ ] Y1.2  que `BUS_PERIOD_MS` salga del minimo de los aparatos vivos y no
             de una constante
```

⚠ **El sacrificio, y se mide antes**: a 1 ms la vuelta se hace **cuatro veces
mas**, con sus cinco trabajos dentro. Por eso `INFO_USB_RITMO` sube tambien el
PEOR de esos cinco: si uno solo se come el 80% del periodo, el hilo no puede
sostener su ritmo. Es `C/T` acercandose a 1 -- la misma cuenta de
[`PLAN_EL_COMPAS`](PLAN_EL_COMPAS.md), aplicada al hilo que hoy decide la
latencia de todo el sistema.

## Y2 -- que el compositor despierte por el EVENTO `[ ]`

Hoy duerme sobre el LATIDO (1 kHz) y al despertar mira su buzon. Falta un
esperable de ENTRADA: `KIND_INPUT` con `RIGHT_WAIT`, y un `wake_by_key` desde el
bus. Con eso el compositor deja de tener ritmo propio: **despierta cuando pasa
algo, y no cuando toca mirar.**

★ Es la pieza que convierte `WAIT` en lo que promete, y quita el <=1 ms del paso
[3] sin acelerar nada.

## Y3 -- V-Sync de verdad `[ ]`

El paso [5] es la mitad del presupuesto y **no se puede tocar sin driver de
pantalla**: hace falta leer por donde va el haz, o una interrupcion de VBlank.
Bloque P3 de `PLAN_EL_PLAZO`, y el mismo bloqueante de siempre -- tras
`ExitBootServices` el GOP no existe.

## Y4 -- page flip: el unico ZERO COPY de verdad `[ ]`

No mover nada y cambiar la direccion que lee el escaner. Escalon 8 de
`docs/identidad/LA_RAM.md`. **El dia que exista, `sin_gpu/` se borra entera** --
eso ya esta escrito en su cabecera desde agosto.

---

# 5. LO QUE ESTE PLAN NO PROMETE

```text
   [ ] no promete 0 ms, y llamarlo asi seria vender humo: un pixel viaja por
       PCIe y un panel se refresca a su ritmo. Lo alcanzable es que BMO-X no
       ANADA nada encima
   [ ] no promete que Y1 mejore nada: puede salir que 4 ms es lo que el raton
       pide. El plan es MEDIRLO, y ese instrumento ya esta puesto
   [ ] X1 probablemente no se hace nunca, y esta escrita para eso
   [ ] y nada de esto esta medido todavia en metal. Lo unico probado del 09-09
       es que la instruccion salio del compilador
```

> El compositor ya no es el cuello. Lo que queda son los dos extremos: el
> aparato que habla cuando quiere y el escaner que mira cuando quiere. **Un
> orquestador que no manda en sus dos extremos no orquesta: acompana.**
