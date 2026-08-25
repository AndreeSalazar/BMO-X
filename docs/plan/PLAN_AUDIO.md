# PLAN AUDIO -- las casillas de su MAESTRO, medidas contra el codigo

> Escrito el **2026-08-25**, cuando `AUDIO_MAESTRO.md` llego a codigo. Lo pide el
> indice de esta carpeta por su propia regla:
>
> > *"El par MAESTRO + PLAN es la simetria de esta carpeta (...) **el dia que un
> > maestro llegue a codigo, su plan va aqui y con este nombre**."*
>
> El **por que** de cada eleccion --y las cinco cosas que ese documento se niega
> a prometer-- vive en [`AUDIO_MAESTRO.md`](../maestro/AUDIO_MAESTRO.md). Aqui
> solo hay casillas.

---

# 0. EL ESTADO, EN UNA TABLA

| paso del maestro | que es | estado |
|---|---|---|
| **0** -- el aparato dice quien es | parsear AudioStreaming | ✅ **HECHO Y CABLEADO**, sin foto |
| **1** -- `SET_INTERFACE` | poner el alt que trae el endpoint | ✅ **HECHO 25-08**, sin ejecutar |
| **2** -- el TRB isocrono | mandar silencio | ✅ **escrito 25-08**, sin ejecutar |
| **3** -- WAV | PCM en un sobre. Cero decodificador | ✅ **HECHO 25-08**, 12 pruebas |
| **4** -- el bufer prestado | dos indices, y CERO copias | ✅ **HECHO 25-08**, sin ejecutar |
| **5** -- MP3 | encima del mismo tubo | ⛔ falta, **y va el ultimo** |

★★ **Y la fila que importa cambio el 25-08: el camino entero esta escrito.**
Descriptor, endpoint, alt, frecuencia, TRB isocrono y el bucle que alimenta.

⚠ **Lo que separa de que suene ya no es codigo: es un ARRANQUE.** Nada de esto ha
corrido nunca en el Ryzen, y el primer numero que lo dira es `encoladas` subiendo
con `tarde` en cero.

---

# 1. LAS CASILLAS

## [X] A0 -- el aparato dice quien es -- **YA ESTABA, y nadie lo habia visto**

Al ir a hacer el paso 0 resulto que **estaba hecho y cableado**:
`bmo_uaudio::stream::find_playback` son 509 lineas con **25 pruebas**, y
`dev/usb/audio.rs` ya lo llama y apunta los cuatro numeros por CABINA.

⚠ **Lo unico que le falta es un arranque.** Es la cuarta vez este mes que
aparece codigo escrito, compilado, probado y **sin ejecutar** -- ver `banda`,
`cabina` y `red rx`. La diferencia es que este si esta cableado: solo falta
mirarlo.

**Como se sabe que quedo hecha**: la foto, y esta **predicha** en el maestro:

```text
   audio: interfaz AudioStreaming, alt      =1
   audio: canales                           =2
   audio: bits por muestra                  =16
   audio: bytes por trama (wMaxPacketSize)  =192
   audio: frecuencia elegida                =48000
   audio: el endpoint isocrono es el DCI    =...
```

## [X] A1 -- `SET_INTERFACE` -- **HECHO el 25-08**

**Que era**: pedirle al aparato el alt setting que trae el endpoint isocrono
(`SET_INTERFACE`, request 0x0B) y configurar ese endpoint en el xHC.

*** **Era el que separaba "escrito" de "suena".** El paso 0 sabia cual era el
alt; el paso 2 sabia encolar una trama. Y **nadie le habia dicho al aparato que
se pusiera en ese alt**, asi que su endpoint no existia.

**Como quedo**: `dev/usb/audio.rs::abrir`, y son tres pasos **en este orden**:

```text
   1. configurar el endpoint en el xHC   el HOST se prepara
   2. SET_INTERFACE (0x0B)               el APARATO empieza su reloj
   3. SET_CUR frecuencia                 solo si declara mas de una
```

★★ **El orden es una decision.** Se prepara el host antes de que el aparato
arranque su reloj: al reves hay una ventana en la que el aparato ya espera datos
en cada microtrama y el xHC no tiene ni anillo donde ponerlos. Con `OUT` eso no
rompe nada --recibe silencio-- pero **cuenta como tramas tarde**, y entonces el
primer numero que se mira al depurar estaria sucio desde antes de empezar.

** Y el paso 3 solo se manda **si hay mas de una frecuencia que elegir**: un
aparato de una sola puede contestar STALL, con razon, y un error que sale en cada
arranque deja de ser un error.

[!] `EP_ISOCH_OUT = 1`, y la tabla del xHCI **no es la del USB**: aqui `1` es
Isoch OUT, `5` Isoch IN y `7` Interrupt IN --el del teclado--. Meter el numero
del USB da un endpoint del tipo equivocado, y eso no falla al configurarlo:
falla al primer TRB.

## [X] A2b -- EL SILENCIO, y se pide a proposito

`audio silencio` arma el empuje; `audio calla` lo para. El hilo del bus encola
**ocho tramas por latido** --cuatro para cubrir los 4 ms, mas cuatro de
colchon-- y toca el timbre **una vez**.

*** **Y no se enciende solo al arrancar.** Abrir el tubo es configuracion y es
seguro; empujar tramas es **trafico continuo a 250 latidos por segundo**, y eso
no debe pasar en cada arranque mientras no haya nada que reproducir. Es la regla
de las hojas de metal metida en el codigo.

** El timbre va FUERA del bucle: uno por trama serian 2.000 escrituras MMIO por
segundo para mover 192 bytes cada una -- **el aviso costaria mas que el dato**.

## [X] A2 -- el TRB isocrono -- **ESCRITO el 25-08**, y sin ejecutar

`queue_isoch_out` en `platform/drivers/usb/xhci/src/transferencia.rs`.

**Las cuatro diferencias con su hermano `queue_interrupt_in`**, que es donde se
equivoca quien copia la funcion de al lado:

```text
   1. el tipo es 5 (Isoch), no 1 (Normal)
   2. lleva FRAME ID: en que microtrama quiere el aparato estos bytes
   3. lleva SIA -- *Start Isoch ASAP*
   4. TBC/TLBC a cero, y cero no es "no aplica": es el valor
```

★ **Se usa `SIA` y no un frame id calculado**, y es una decision con precio:
calcularlo exige leer el `MFINDEX` y acertar antes de que el reloj avance, y
fallar el numero se **oye como un clic**. El maestro ya eligio por escrito:

> *"Primero que suene sin huecos. Un audio puntual con 40 ms de retardo es audio;
> uno con 5 ms y clics, no."*

** Y trae los dos contadores que el maestro pedia: `isoch_encoladas` --que tiene
que subir sola-- y **`isoch_tarde`**, que se apunta en el bucle de eventos
cuando el xHC contesta `Missed Service Error` (CC 10) o `Isoch Buffer Overrun`
(CC 31).

> **`tramas tarde` es la cifra de toda la pagina de audio.** Un audio que va bien
> y uno que chasquea se distinguen por ese contador y por nada mas -- a oido son
> *"suena raro"* y *"suena bien"*, que no es un diagnostico.

## [X] A3 -- WAV -- **HECHO el 25-08**, `platform/shared/bmo-sonido`

12 pruebas. Y **no es un formato de audio**: es PCM en un sobre, o sea
exactamente lo que come el endpoint. Cero decodificador.

### *** El fallo que este crate existe para impedir

Se lee mucho que *"un WAV son 44 bytes de cabecera"*. Es cierto en el caso comun
y **falso en general**: RIFF es una lista de trozos, y entre `fmt ` y `data`
puede haber un `LIST` con el titulo de la cancion.

> Un lector que salte 44 bytes a ciegas **entrega los metadatos como si fueran
> muestras**. Y eso no da un error: **da ruido blanco a todo volumen, en un
> oido.**

Por eso recorre los trozos, cuenta el byte de relleno de los impares, y comprueba
en `u64` que cada largo quepa -- que lo escribe el fichero, o sea otro.

### Y no convierte nada, a proposito

`cabe_en` contesta **si** o **no** con el motivo, y **no existe un `Casi`**: esa
variante habria sido la puerta por la que entra el resampler, que la parte 8 del
maestro rechaza por escrito.

## [X] A4 -- el bufer prestado -- **HECHO el 25-08, ANTES de MP3**

La parte 4 del maestro avisaba: *"si esto se hace en el paso 3, nace bien. Si se
hace despues, **hay que deshacer una copia**"*. Se hizo antes.

```text
   MAL   `audio_escribir(&muestras)` -> el kernel copia 192 bytes a su anillo.
         Mil veces por segundo, mil cruces de puerta y mil copias
   BIEN  la app pide un bloque, lo llena de PCM y lo OFRECE.
         **La app escribe donde el aparato va a leer**
```

### *** Y AQUI HAY ALGO QUE SOLO SE VE DESPUES DE SMAP

Desde el 25-08 Ring 0 **no puede tocar memoria de Ring 3**. Un diseno que hiciera
al kernel **leer** las muestras del bufer de la app estaria muerto desde esa
misma manana: `#PF` en la primera trama.

★★ **Este no lee nada.** El TRB isocrono lleva una direccion **FISICA**, y quien
va a buscar los bytes es **el xHC por DMA** -- no el CPU. El kernel solo traduce
una VA a su fisica **una vez**, al ofrecer.

> **El que lee no es el CPU, asi que SMAP no tiene nada que decir.**

[!] Y lo hace posible que `KIND_MEMORIA` entregue marcos **contiguos**: `Bloque`
guarda una `fisica` y los bytes van seguidos detras. Con paginas sueltas haria
falta un TRB por pagina y el corte no caeria en la frontera de una trama.

### Los dos numeros que cruzan, y ninguno mas

```text
   escrito   lo mueve la APP:   "he llenado hasta aqui"
   leido     lo mueve el TUBO:  "voy por alla"
```

** `escrito` se comprueba contra el tamano del bloque, y el tamano **lo dice el
bloque, no la app**: preguntarselo a ella seria dejar que declare un tamano que
no tiene. Y `fisica_de` busca en **sus** bloques y en ninguno mas, que es lo que
impide ofrecer la memoria de otro.

### *** MEDIA TRAMA NO SE MANDA

Si hay menos bytes de los que pide una trama, **no se entrega lo que hay
rellenando el resto**: sale una trama de silencio y se cuenta. Inventar muestras
no se ve -- **se oye**.

### Y el tercer contador, que separa dos culpas

```text
   tarde    el xHC no llego a su cita       -> el problema es del BUS
   huecos   nadie escribio la trama         -> el problema es de la APP
```

★ Los dos se oyen igual: un clic. **Sin separarlos, un audio que chasquea manda a
mirar el driver cuando la mitad de las veces el que llega tarde es quien
produce.** Los dos salen en `audio`, con su etiqueta al lado.

### Y se suelta al morir

`revoke_all` suelta el prestamo. Sin eso, **el aparato seguiria leyendo por DMA
marcos de un proceso que ya no existe** -- que es peor que un fallo: es un ruido
que no para y que no tiene dueno a quien pedirle que pare.

## ⛔ A5 -- MP3, y **por que va el ultimo aunque sea lo que se pidio**

El dueno lo pidio por nombre, y la respuesta honesta es el orden, no un no.

```text
   un .wav   ->  PCM              ->  el endpoint
   un .mp3   ->  DECODER -> PCM   ->  el MISMO endpoint
```

★★ **El decodificador no toca nada de lo anterior: entrega PCM al mismo sitio.**
Por eso puede ir el ultimo sin que nada se rehaga -- y por eso ir primero seria
caro:

> Empezar por aqui dejaria **un decodificador en verde y sin ejecutar** mientras
> no hay donde soltar las muestras. Es la cicatriz de los nueve tests de coma
> flotante del frontend de C, otra vez.

**Que trae**: `minimp3` es **un solo fichero**, o sea unity build por diseno --
como la amalgamation de SQLite. Va en Ring 3.

⚠ **Y hay un bloqueante que hay que mirar antes de traerlo**: `minimp3` usa coma
flotante, y el frontend de C de esta casa ya tiene historia ahi. Comprobarlo es
media tarde y se hace **antes** de traer 2.000 lineas, no despues.

[!] `bmo-sonido` ya **reconoce** un MP3 y contesta que no sabe tocarlo. Eso no es
soporte: es la diferencia entre *"esto no es audio"* y *"esto es un MP3 y aqui
todavia no hay decodificador"* -- dos respuestas que mandan a sitios distintos.

---

# 2. EL ORDEN, y por que es ese

```
   [X] A0  el aparato dice quien es    hecho y cableado. FALTA LA FOTO
   [X] A2  el TRB isocrono             escrito, sin ejecutar
   [X] A3  WAV                         hecho, 12 pruebas
   --------------------------------------------------------------------------
   [X] A1  SET_INTERFACE               hecho, sin ejecutar
   [X] A2b el silencio, a peticion     hecho, sin ejecutar
   [X] A4  el bufer prestado           hecho ANTES de MP3, que era el aviso
   --------------------------------------------------------------------------
   A5  MP3                             lo unico que queda, y sin rehacer nada
```

★★ **Lo que separa hoy de que suene ya no es codigo: es un ARRANQUE.** Todo el
camino --descriptor, endpoint, alt, frecuencia, TRB isocrono y el bucle que
alimenta-- esta escrito y **nada de ello ha corrido nunca**. El primer numero que
lo dira es `encoladas` subiendo con `tarde` en cero.

★ **A1 va primero de lo que queda y no es discutible**: con A0, A2 y A3 hechos,
lo unico que impide que salga un sonido es que **nadie le ha dicho al aparato que
se ponga en el alt que trae el endpoint.**

---

# 3. LA PRUEBA QUE PIDIO EL DUENO, y en que orden llega

```text
   1. el silencio        ceros en bucle. EL SILENCIO NO PUEDE SONAR MAL
   2. un tono            una onda generada, sin fichero
   3. un .wav            leer y dar. Cero decodificador
   4. un .mp3            y aqui ya no hay nada nuevo que inventar
```

★★ **El 1 es el equivalente de `net rx`**: si el endpoint no se atasca y
`isoch_encoladas` sube sola con `isoch_tarde` en cero, **el tubo esta vivo** sin
haber arriesgado un solo ruido raro en los oidos del dueno.

---

# 4. LO QUE ESTE PLAN NO PROMETE

Las cinco de la parte 8 del maestro, sin cambiar ninguna: **resampleo**,
**mezclador de varias apps**, **grabar**, **HD Audio** y **latencia baja**.

★ Y la que mas cuesta aceptar es la ultima, asi que va con su frase: *"Primero
que suene sin huecos. Un audio puntual con 40 ms de retardo es audio; uno con 5
ms y clics, no."*
