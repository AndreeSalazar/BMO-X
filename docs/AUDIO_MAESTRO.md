# AUDIO MAESTRO -- del silencio a los gatitos, sin inventar un driver

> Escrito el **2026-08-12**, ANTES de escribir una linea del camino de sonido.
> Es lo mismo que se hizo con `RED_MAESTRO.md` y salio bien: el plan primero, y
> despues el driver contra el plan en vez de contra la intuicion.

**La prueba de aceptacion de este documento es una sola frase, y es del dueno:**

> *"cuando llegue al sonido real quiero escuchar mis gatitos"*

No es una broma dentro del plan: es el criterio. Un tono de prueba lo produce
cualquier cosa que oscile. Un maullido grabado exige **frecuencia correcta,
canales correctos, muestras en el orden correcto y sin huecos** -- si algo de eso
falla, un gato no suena como un gato y se nota sin instrumentos. Es el mismo
truco que la MAC predicha de la red: una respuesta que se puede comprobar de un
vistazo vale mas que un contador.

---

# 1. EL LIMITE QUE IMPORTA NO ES EL QUE PARECE

Con la red el limite era la LATENCIA y no el ancho de banda. Aqui es distinto y
conviene decirlo antes de optimizar nada:

**El audio no es rapido, es PUNTUAL.**

Un audifono USB a 48 kHz, 16 bits, 2 canales son **192.000 B/s**. Eso no es
ancho de banda: es lo que cabe en una lectura de disco de las que ya se hacen. El
problema es otro -- ese aparato pide su trama **cada milisegundo, siempre**, y si
una vez no hay muestras preparadas no se oye "un poco peor": se oye un CLIC.

```
   modelo CAUDAL      mover N bytes por segundo        <- lo que NO es
   modelo METRONOMO   tener 192 bytes listos CADA ms   <- lo que SI es
```

De ahi salen dos consecuencias que ordenan todo lo demas:

1. **Alguien tiene que alimentar el endpoint sin que nadie se lo pida.** Eso
   existe desde el 2026-08-12: el **hilo del bus** (`ring0/dev/usb/bus.rs`).
   Antes de el, esta pagina no se podia escribir.
2. **Las muestras no pueden viajar por syscalls.** Un `escribir(192 bytes)` cada
   milisegundo es mil cruces de anillo por segundo para mover lo que cabe en una
   linea de cache. Ver la parte 4.

---

# 2. LO QUE DICE EL SILICIO QUE HAY DEBAJO

Medido en el Windows de esta misma maquina, no supuesto:

- **El audifono del dueno es USB Audio Class 1.0** -- `VID_1B3F&PID_2008`,
  `USB\Class_01&SubClass_01`, driver generico `wdma_usb.inf`. Es el **unico**
  endpoint de salida activo de la maquina.
- El codec de la placa es un **Realtek ALC897**, y sus endpoints
  "Auriculares"/"Altavoces" estan `DeviceState=4` (no presente). No es
  concluyente --el driver generico no hace deteccion de jack-- pero **si no hay
  nada enchufado al jack verde, un driver HDA no produciria un solo sonido
  audible**.

## La consecuencia que reordena el plan

Escribir un driver **HD Audio** seria escribir un driver entero para un aparato
que no esta conectado. El camino corto pasa por el USB, que **ya funciona en
metal**: enumera, hace control transfers y tiene quien lo bombee.

[!] Y el zumbador del PC (`AUDIO_OP_BEEP`, puerto 0x61) **no cuenta**: el puerto
existe en todo x86, el cabezal SPKR de una MSI A320M puede venir sin conectar, y
ademas esa operacion **bloquea el nucleo** mientras suena (de ahi su tope de 250
ms). Es una campana, no una tarjeta de sonido.

---

# 3. EL REPARTO: DONDE VIVE CADA COSA

El mismo de siempre, y por las mismas razones:

```
   Ring 0                              Ring 3
   ------                              ------
   KIND_AUDIO                          WAV, MP3, mezcla, volumen por app
   el ENDPOINT y su metronomo          los FORMATOS, que tienen versiones
   el anillo de tramas                 y por tanto se equivocan
```

**El kernel no sabe lo que es un MP3.** Un decodificador es codigo de terceros
lleno de tablas y desplazamientos; aqui puede morirse sin llevarse la maquina.

Y hay una razon extra que no es filosofica: los formatos **se acumulan**. Meter
uno en Ring 0 es abrir la puerta a los quince siguientes.

---

# 4. LAS MUESTRAS SE PRESTAN, NO SE COPIAN

Es la regla de la casa (`LA_RAM.md`) aplicada al sonido, y hay que decidirla
**antes** de escribir el primer `write`, porque despues cuesta deshacerla.

**Mal**: `audio_escribir(&muestras)` -> el kernel copia 192 bytes a su anillo.
Mil veces por segundo, mil cruces de anillo y mil copias.

**Bien**: la app pide un bloque con `KIND_MEMORIA`, lo llena con PCM y lo
**ofrece** (`MEM_OP_OFRECER`). El kernel lee de ahi para alimentar el endpoint.
La app escribe donde el aparato va a leer. **Cero copias**, y es exactamente el
mismo mecanismo que ya sostiene el LIENZO del compositor.

Lo unico que cruza el anillo son dos numeros: *"voy por aqui"* y *"vas por
alla"* -- un anillo de productor/consumidor con dos indices.

[!] **Si esto se hace en el paso 3, nace bien. Si se hace despues, hay que
deshacer una copia** -- y esta casa ya tiene la lista de copias que costo quitar.

---

# 5. EL ESTADO REAL HOY, medido el 2026-08-12

No es "no hay nada". Es bastante mas de lo que parecia, y por eso se cuenta:

| Pieza | Estado |
|---|---|
| `control_transfer` | **funciona en metal** -- lo usa toda la enumeracion |
| `configure_endpoint(slot, dci, ep_type, max_pkt, interval)` | **el tipo es un PARAMETRO**, no esta clavado. Isoch OUT = `1`; hoy `uhid` pasa `7` (Interrupt IN) |
| `SET_INTERFACE` | ya se manda un `bRequest 0x0B` en `enumera.rs:181` (SET_PROTOCOL de HID). Cambiar el recipient es **una linea** |
| El metronomo | **el hilo del bus**, 250 Hz, desde el 2026-08-12 |
| `bmo-uaudio` | volumen, mute, rangos, y **cero I/O**: es protocolo puro y probado en el anfitrion |
| `KIND_AUDIO` | existe como CONTRATO (claim/release exclusivo, anti-UAF por generacion) con `BEEP`/`VOLUME`/`SILENCE`/`DEVICES` |

## Lo que falta de verdad son DOS cosas, no un driver

1. **El TRB isocrono.** `queue_interrupt_in` emite un TRB Normal (tipo 1); el
   isocrono es **tipo 5**, con su frame ID. Es una funcion al lado, no una
   reescritura.
2. **El lado AudioStreaming del descriptor.** `bmo-uaudio` lo dice el mismo en su
   linea 51: *"la interfaz que transporta muestras es AUDIOSTREAMING (0x02) y no
   se toca aqui"*. Hay que leer alt settings, endpoint isocrono, formato,
   frecuencia y `wMaxPacketSize`.

---

# 6. EL ORDEN QUE PROPONE ESTE DOCUMENTO

Cada paso deja el sistema funcionando, que es la regla de la casa.

### Paso 0 -- QUE EL APARATO DIGA QUIEN ES. Cero I/O.

Parsear el lado AudioStreaming en `bmo-uaudio`: interfaz, alt settings, endpoint
isocrono OUT, formato, frecuencia, `wMaxPacketSize`.

**Es descriptor puro, asi que se prueba en el anfitrion** como el resto de ese
crate -- y por eso va primero: es el unico paso que no puede romper nada.

La foto que se busca, y **se puede predecir mirando el Windows de esta misma
maquina antes de arrancar**, igual que se hizo con la MAC:

```text
   audio: interfaz AS hallada, alt          =1
   audio: PCM 16 bits, canales              =2
   audio: frecuencia                        =48000
   audio: bytes por trama (wMaxPacketSize)  =192
```

Si sale otra cosa, **el numero dice cual pregunta fallo**: `canales =0` es que se
leyo el descriptor equivocado; una frecuencia que no esta en la lista del aparato
es que se leyo el campo equivocado.

### Paso 1 -- SET_INTERFACE Y EL ENDPOINT CONFIGURADO. Todavia no suena.

`SET_INTERFACE` al alt que trae el endpoint (el alt 0 de una AS **no lleva
ninguno**, a proposito: es el modo "no gasto ancho de banda"), y
`configure_endpoint` con `ep_type = 1`.

**Lo que se prueba es que el xHC ACEPTA el endpoint**, no que se oiga nada. Si el
Configure Endpoint devuelve error, el problema es el descriptor del paso 0 y no
el sonido.

[!] Aqui reaparece la mina del teclado: **sin Max ESIT Payload el xHC asigna cero
ancho de banda al endpoint** y las tramas no completan jamas. Esta escrito en
`xhci/src/lib.rs:1044`. Para un isocrono ese numero es mas grande que para un
teclado y es el primero que hay que mirar si nada se mueve.

### Paso 2 -- EL TRB ISOCRONO, MANDANDO SILENCIO.

Ceros, en bucle, alimentados por el hilo del bus.

**El silencio no puede sonar mal**, y esa es toda la idea: es la misma jugada que
`net rx` (recibir sin transmitir). Si el endpoint no se atasca y los contadores
avanzan, **el tubo esta vivo** sin haber arriesgado un solo ruido raro en los
oidos del dueno.

```text
   audio: tramas entregadas       =...   <- tiene que SUBIR sola
   audio: tramas que llegaron tarde =0   <- si sube, el metronomo no llega
```

### Paso 3 -- WAV. Y NO ES UN FORMATO DE AUDIO.

Un `.wav` es **PCM en un sobre**: 44 bytes de cabecera y detras las muestras
crudas, que es exactamente lo que come el endpoint. **Cero decodificador.**

Por eso el paso 3 es la BASE y no una etapa mas: si el aparato pide 48 kHz /
16 bits / 2 canales y el fichero viene asi, **el trabajo es leerlo y darlo**.

[!] Y si el fichero NO viene asi, este documento se niega a resamplear (ver la
parte 8). Se dice el formato que hace falta y se convierte fuera.

### Paso 4 -- EL BUFER PRESTADO. Cero copias.

`MEM_OP_OFRECER` y dos indices. Ver la parte 4. Puede hacerse a la vez que el 3;
lo que no puede es hacerse mucho despues.

### Paso 5 -- MP3, ENCIMA DEL MISMO TUBO.

`minimp3` es **un solo fichero**, o sea unity build por diseno -- igual que la
amalgamation de SQLite. Va en Ring 3, y **no toca nada de lo anterior**: entrega
PCM al mismo anillo del paso 4.

Va el ultimo a proposito. Empezar por aqui dejaria un decodificador en verde y
sin ejecutar mientras no hay donde soltar las muestras, que es exactamente la
cicatriz de los nueve tests de coma flotante del frontend de C.

---

# 7. LA CABINA DE AUDIO -- que tiene que confesar

Con el formato de unidades de `cabina-core::Fmt`, que ya existe:

| Linea | Unidad | Por que |
|---|---|---|
| `frecuencia` | `Count` | 48000, no `0xBB80` |
| `bytes por trama` | `Bytes` | el numero que hay que cuadrar con el aparato |
| `tramas entregadas` | `Count` | tiene que subir SOLA |
| `tramas tarde` | `Count` | **el numero que dice si hay clics**, y es el unico que importa cuando algo suena mal |
| `formato del endpoint` | `Bits` | los bits de `bmAttributes` deciden si es asincrono o adaptativo |

★ **`tramas tarde` es la fila de esta pagina.** Un audio que va bien y uno que
chasquea se distinguen por ese contador y por nada mas -- a oido son "suena raro"
y "suena bien", que no es un diagnostico.

---

# 8. LO QUE ESTE DOCUMENTO SE NIEGA A PROMETER

- **Resampleo.** Si el aparato pide 48 kHz y el fichero viene a 44,1, aqui **no
  se convierte**: se dice y se convierte fuera. Un resampler malo suena peor que
  no sonar, y uno bueno es un proyecto.
- **Mezclador de varias apps.** Un `KIND_AUDIO` es exclusivo, como la pantalla.
  Dos programas sonando a la vez es una politica de Ring 3, y llega cuando haya
  dos programas que suenen.
- **Grabar.** El microfono es otro endpoint, en el otro sentido, y no esta en
  ninguna de las frases del dueno.
- **HD Audio.** Ver la parte 2: seria un driver entero para un aparato que no
  esta conectado.
- **Latencia baja.** Primero que suene sin huecos. Un audio puntual con 40 ms de
  retardo es audio; uno con 5 ms y clics, no.

---

# LA PRUEBA FINAL, y es la que pidio el dueno

```text
   run apps/gatitos.bex        (o el reproductor que sea)
   -> se oyen los gatitos
```

[!] **Y hoy ese fichero NO EXISTE en el arbol**: `docs/arte/` tiene
`bmo-x-gato.jpg` --el logo, que es de donde sale el gato del splash-- y **ni un
solo fichero de audio en todo el repositorio**. Comprobado el 2026-08-12.

Asi que el paso 3 necesita que el dueno deje su grabacion, y este documento dice
**exactamente en que formato** para que no haya que adivinar ni resamplear:

```text
   PCM firmado de 16 bits, 2 canales, 48000 Hz, WAV
   -> y lo que diga el paso 0 manda sobre esto, porque lo dice el APARATO
```

El gato de BMO-X lleva desde el principio en el arranque sin decir nada -- *"el
gato no te juzga, no te dice nada, solo te protege"*. Que lo primero que suene en
este sistema sea un gato de verdad es una coincidencia demasiado buena para
desaprovecharla.

---

Ver `LA_RAM.md` (por que las muestras se prestan), `RED_MAESTRO.md` (el mismo
metodo, que ya salio bien) y `ARQUITECTURA.md`.
