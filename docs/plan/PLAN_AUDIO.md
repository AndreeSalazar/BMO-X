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
| **1** -- `SET_INTERFACE` | poner el alt que trae el endpoint | ⛔ **falta** |
| **2** -- el TRB isocrono | mandar silencio | ✅ **escrito 25-08**, sin ejecutar |
| **3** -- WAV | PCM en un sobre. Cero decodificador | ✅ **HECHO 25-08**, 12 pruebas |
| **4** -- el bufer prestado | `MEM_OP_OFRECER` y dos indices | ⛔ falta |
| **5** -- MP3 | encima del mismo tubo | ⛔ falta, **y va el ultimo** |

★★ **Y la fila que importa: hoy BMO-X CONTROLA EL VOLUMEN Y NO PUEDE EMITIR UNA
MUESTRA.** Lo que falta para que suene no es un driver -- es el paso 1 y un
bucle que alimente el tubo.

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

## ⛔ A1 -- `SET_INTERFACE`: EL BLOQUEANTE DE VERDAD

**Que falta**: pedirle al aparato el alt setting que trae el endpoint isocrono
(`SET_INTERFACE`, request 0x0B) y configurar ese endpoint en el xHC.

*** **Este es el que separa "escrito" de "suena".** El paso 0 sabe cual es el
alt; el paso 2 sabe encolar una trama. **Nadie le ha dicho al aparato que se
ponga en ese alt**, asi que el endpoint no existe todavia.

**Que la bloquea**: nada tecnico. `bmo-xhci` ya sabe mandar peticiones de
control --las usa el volumen-- y ya sabe configurar endpoints.

**Como se sabe que quedo hecha**: `cabina` dice que el endpoint quedo
configurado, y su DCI coincide con el que declaro el descriptor.

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

## ⛔ A4 -- el bufer prestado

`MEM_OP_OFRECER` y dos indices. Puede hacerse a la vez que el A5; lo que no puede
es hacerse mucho despues, o el bucle nacera copiando.

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
   A1  SET_INTERFACE                   *** LO UNICO QUE SEPARA DE QUE SUENE
   A4  el bufer prestado               a la vez que A5, no despues
   A5  MP3                             el ultimo, y sin rehacer nada
```

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
