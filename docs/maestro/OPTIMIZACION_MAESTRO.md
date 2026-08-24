# OPTIMIZACION MAESTRO -- la regla es que es LO ULTIMO, y el respeto es saber por que

> Escrito el 2026-08-24, a peticion del dueno: *"vamos a optimizar TODO en CPU en
> ciclo, en Red, en teclado y mouse sus ciclos propios que necesita tiempo --
> cada uno existe por la doc de reglas de optimizacion y respeto, y cada uno es
> EL ULTIMO para aplicar."*
>
> Este documento existe porque el principio ya estaba escrito **en un solo
> componente** --`EL_TECLADO_EXIGE.md`-- y ahi valia solo para un aparato. Es una
> ley, no una nota de un fichero.

---

# LA LEY 0: CORRECTO, MEDIDO, RAPIDO -- Y EN ESE ORDEN

```text
   1. CORRECTO   hace lo que dice, y hay una prueba que lo EJECUTA
   2. MEDIDO     hay un numero, y se sabe con que instrumento salio
   3. RAPIDO     y solo entonces, y solo donde el numero diga
```

** Los tres pasos no son consejos de estilo: **cada uno hace posible el
siguiente.** Optimizar algo que no es correcto es hacerlo mal mas rapido.
Optimizar algo que no esta medido es cambiarlo a ciegas y llamarlo mejora.

*** **Y "lo ultimo" quiere decir LO ULTIMO.** No penultimo, no "en paralelo si
hay tiempo". Toda optimizacion que entro antes de su turno en este proyecto
costo mas de lo que ahorro, y hay dos escritas con nombre en `BITACORA.md`:
la ley 10 (el write-combining sin `sfence` **no era rapido, era incorrecto**) y
la ley 12 (cambiar el tipo de memoria de una region **es cambiar un contrato**).

---

# 1. LA PREGUNTA QUE VA ANTES DE TODAS: QUIEN PONE EL PRESUPUESTO?

Antes de tocar una linea por rendimiento hay que contestar **quien manda sobre
el tiempo de ese camino**. Y casi nunca es el CPU.

| camino | quien pone el presupuesto | y entonces |
|---|---|---|
| calculo puro | **el CPU** | ciclos. Aqui SI se optimiza |
| teclado y raton | **el BUS USB** (microframe de 125 us) | ★ **jamas se optimizan ciclos** |
| pantalla | **el ESCANER** (16,7 ms a 60 Hz) | bytes por fotograma |
| red | **el ENLACE** y la ida y vuelta | microsegundos, no ciclos |
| disco | **el APARATO** | no esperar es lo que se optimiza, no calcular menos |
| una app respondiendo | **EL HUMANO** | ~100 ms para parecer instantaneo |

*** **Optimizar un camino cuyo presupuesto lo pone otro es optimizar nada.** Y
es peor que nada: se paga con complejidad, se cobra en cero, y el codigo queda
mas dificil de arreglar el dia que falle de verdad.

---

# 2. EL RESPETO: los caminos que NECESITAN TIEMPO

Esta es la mitad del titulo que no habla de velocidad, y es la que se olvida.

## 2.1 -- El teclado y el raton

`EL_TECLADO_EXIGE.md` ya lo tenia escrito, con sus tres numeros:

```text
   [SPEC]   un microframe                125 us  =  ~462.000 ticks de TSC
   [MEDIDO] una puerta del sistema             884 ticks  =  0,19% de un microframe
   [DATO]   un humano rapido              < 20 pulsaciones por segundo
```

> **El presupuesto de este camino lo pone el bus, no el CPU. Aqui no se
> optimizan ciclos jamas.**

** Un teclado USB **no avisa: contesta cuando le preguntan**, cada `Interval`. Si
nadie pregunta, no hay teclas -- aunque el aparato este perfecto. Asi que lo que
hay que cuidar no es la velocidad del camino: es **que la bomba no se pare.**

*** Y ahi esta el fallo real de este componente, que nunca fue de ciclos:
**perder un evento de un endpoint de interrupcion no pierde una pulsacion: PARA
LA BOMBA**, y el teclado se queda mudo hasta que alguien reinicia. Optimizar ese
camino un 30% no habria evitado ni uno de los fallos que costaron dias.

## 2.2 -- Lo que "respeto" significa, dicho como regla

```text
   Un aparato que espera NO es un aparato lento.
   Un camino cuyo presupuesto pone un humano NO se acelera: se hace FIABLE.
```

Un raton que responde en 1 ms y uno que responde en 8 se sienten **igual**. Uno
que se queda quieto una vez al dia se siente **roto**. La segunda propiedad vale
mas que la primera y no se compra con ciclos.

---

# 3. LA BUROCRACIA: lo que se paga por el proceso y no por el trabajo

Esta es la unica clase de lentitud que este sistema persigue por su cuenta, sin
esperar a que un numero lo pida.

## 3.1 -- El numero que la nombra

Una llamada al sistema, medida en el Ryzen 5 5600X con **dos instrumentos
distintos que coinciden en un 1,7%**:

```text
   la puerta pelada             969 ciclos       (792 ticks = 214 ns)
   resolver el handle         + 221 ciclos       (la sonda que aisla la variable)
   ------------------------------------------------------------------
   antes de hacer NADA        ~1.190 ciclos
   el trabajo de OP_INFO           36 ciclos     <- lo que de verdad se pidio
```

*** **El trabajo es el 3% y la burocracia el 97%.** Ese es el numero que hay que
tener delante cada vez que alguien proponga una operacion nueva.

## 3.2 -- ** Y LA BUROCRACIA NO SE LIMA: SE EVITA

La reaccion natural es optimizar la puerta. **Es el trabajo equivocado**, y la
aritmetica lo dice sola:

```text
   la via rapida son 58 instrucciones de ensamblador
   un Zen 3 retira hasta 6 por ciclo
   ni contandolas a UNA POR CICLO se llega a 969
```

Lo que cuesta no son las instrucciones: son **las dos transiciones de
privilegio**, y esas no se liman. Se pagan o no se pagan.

> **La regla:** cuando el coste fijo domina, la mejora no es hacer la operacion
> mas barata -- es **hacer menos operaciones**. Una llamada que trae diez cosas
> en vez de una paga la burocracia una vez.

[!] Y esto es lo contrario de lo que parece un microoptimizador haciendo su
trabajo. Quitar tres instrucciones del prologo se ve, se mide, y **no cambia
nada**.

## 3.3 -- Donde SI vale la pena mirar

| sitio | por que |
|---|---|
| operaciones que se llaman en bucle | pagan la burocracia N veces |
| datos que cruzan la puerta byte a byte | el buzon en memoria propia ya existe para eso |
| copias que podrian ser reflejos | `LA_RAM.md`: reflejar en vez de copiar |

---

# 4. EL CPU: donde SI se cuentan ciclos, y como

## 4.1 -- Las unidades, y la trampa que ya costo un 22%

```text
   1 tick de TSC  =  1,22 ciclos de nucleo  =  0,27 ns   (en este Ryzen)
```

[!] **El metro llevaba desde el primer dia imprimiendo `ciclos/op` donde media
TICKS**: un error del 22%, y es el patron 2 de la casa -- *el campo que viene en
otra unidad*. Corregido el 17-08.

*** Y los **presupuestos siguen en ticks a proposito**: el TSC es invariante y
los ciclos no. Convertir antes de comparar contra el techo moveria el trinquete
cada vez que el CPU cambia de frecuencia, **que es justo lo que un trinquete no
puede hacer**.

## 4.2 -- Dos testigos, y no se promedian

Los 969 ciclos salieron de dos instrumentos con compiladores y bucles distintos,
y coinciden en 1,7%. **Eso no valida la puerta: valida el INSTRUMENTO**, que es
lo que permite creerse el resto.

[!] Y donde NO coinciden --el handle: 221 contra 383-- **no se promedian**. La
sonda de 221 aisla la variable; la de 383 compara dos filas que se diferencian
en dos cosas a la vez. **La buena es 221; la otra mide otra pregunta.**

> Promediar dos numeros que miden cosas distintas produce un tercero que no mide
> ninguna.

## 4.3 -- Lo que INTI cuesta, y por que se puede leer

El manifiesto del `.bex` trae los numeros: cuantas comprobaciones anti-UB se
emitieron, cuantos bloques `crudo`, cuantas instrucciones de la maquina se
tocan. **El precio del "sin comportamiento indefinido" es un numero que se
puede mirar**, no una promesa.

Y una llamada de funcion en INTI son **20 ciclos**, medidos. Ese es el dato con
el que se decide si algo se mete en linea, cuando llegue su turno.

---

# 5. LA PANTALLA: el presupuesto lo pone el escaner

```text
   [MEDIDO]  volcar la pantalla entera        27,6 ms
   [SPEC]    un frame de video a 60 Hz        16,7 ms
   [DERIVADO] lo que CABE en un frame          5,0 MB  =  60%
```

*** **No hay V-Sync ni VBlank.** Asi que la regla no es "ir mas rapido": es **no
volcar mas del 60%**, porque por encima de ese umbral se dibuja mientras el
escaner ya paso, y eso no se ve como lentitud -- **se ve como una imagen
partida.**

** Y por eso la caja de sucio existe: recortar lo que se vuelca vale mas que
acelerar el volcado. Es la misma regla que la burocracia, en otro aparato:
**hacer menos, no hacerlo mas rapido.**

[!] Y por eso el deficit de DOOM a 1600x1000 es **el blit entero** y no el
raycaster. Optimizar el calculo de DOOM no habria movido un fotograma.

---

# 6. LA RED: latencia, y sin burocracia

El titular de `RED_MAESTRO.md` no es el ancho de banda:

> **Los microsegundos entre el aviso y la lectura.** Sin ese numero, "es rapido"
> es una opinion.

## 6.1 -- Lo que se mide desde el primer anillo

| numero | que delata si no es lo que debe |
|---|---|
| tramas recibidas / descartadas | un anillo que se llena sin que nadie lo vacie |
| **descriptores que el driver no devolvio** | la bomba parada, como el USB |
| avisos por MSI contra vueltas de sondeo | que la placa acepte MSI **y no lo enrute** |
| bytes que fueron DIRECTOS al anillo prestado | que el camino sin copia se este tomando |
| **microsegundos entre aviso y lectura** | la latencia, que es el titular |

## 6.2 -- La burocracia de la red, y donde NO esta

El enlace de esta maquina es de **100 Mbps**. A esa velocidad, el modelo "una
interrupcion por paquete" sobra de largo -- el problema aparece a gigabit.

*** **Asi que hoy la red NO tiene un problema de burocracia, y decirlo importa**:
optimizar el camino de recepcion antes de que exista el de transmision seria
optimizar un camino que todavia no lleva nada.

** El camino sin copia --el anillo prestado por `MEM_OP_OFRECER`-- ya esta
disenado, y lo que hace falta no es acelerarlo: es **contar si se esta tomando**.
Un camino rapido que nadie mide es un camino rapido que un dia deja de tomarse
en silencio.

## 6.3 -- Y la unica comparacion honesta

> **La latencia de ida y vuelta, en microsegundos, contra la que da Windows en
> la misma maquina y el mismo cable.**

No contra un objetivo inventado. Contra otro sistema, en el mismo silicio.

---

# 7. LAS REGLAS, JUNTAS

```text
   0   correcto, medido, rapido. En ese orden, y lo ultimo es LO ULTIMO
   1   antes de optimizar, decir QUIEN PONE EL PRESUPUESTO de ese camino
   2   un camino cuyo presupuesto pone un bus o un humano NO se acelera:
       se hace FIABLE
   3   cuando el coste fijo domina, hacer MENOS operaciones -- no operaciones
       mas baratas
   4   dos instrumentos que coinciden validan el METRO; dos que no coinciden
       NO se promedian
   5   los presupuestos van en la unidad que no se mueve (ticks, no ciclos)
   6   una optimizacion que cambia CUANDO se ve algo no esta terminada
       hasta que alguien decide cuando tiene que verse   [ley 10]
   7   cambiar el tipo de memoria de una region es cambiar un CONTRATO, y hay
       que ir a buscar a todos los que la usaban -- LOS LECTORES CUENTAN [ley 12]
   8   un camino rapido que nadie mide es un camino rapido que un dia deja
       de tomarse en silencio
```

---

# 8. LO QUE ESTE DOCUMENTO SE NIEGA A PROMETER

- **No hay optimizador en INTI, y hoy eso es una ventaja que nadie pidio.** El
  MMIO funciona porque no hay nada que elida las escrituras. **Eso es una
  suerte, no una garantia**: el dia que exista un optimizador, `volatil` tiene
  que existir antes -- y hoy no existe ni la palabra.
- **No se van a perseguir ciclos en caminos de aparato.** Esta escrito arriba y
  vale como respuesta a cualquier propuesta futura.
- **Ningun numero de aqui vale para otra maquina.** Son de un Ryzen 5 5600X
  concreto. Es la ley 24: **el hardware se perfila**, y una medida es un hecho
  sobre un chip.

---

# El resumen en una frase

> **Optimizar es lo ultimo, y respetar es saber que la mitad de los caminos de
> este sistema no se optimizan nunca: se hacen fiables.**
