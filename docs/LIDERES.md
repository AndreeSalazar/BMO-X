# Los LIDERES: un servidor por aparato exclusivo

> Escrito el **2026-08-09**, el dia que `KIND_AUDIO` dejo de ser una idea y la
> ventana F10 tuvo que tomar y devolver el aparato para no dejar mudo a todo lo
> demas.
>
> `docs/PLAN_DOOM.md` dice **que falta para jugar**. Este dice **quien manda
> sobre cada aparato, y como reparte lo que le dieron.**

## La idea, en una frase

El kernel concede un aparato exclusivo a **UN** proceso. Ese proceso no lo usa
para si: lo **reparte**. Es un lider.

```
   kernel  --KIND_FRAMEBUFFER-->  gui.bex   --superficies-->  programas
   kernel  --KIND_AUDIO------->  audio.bex  --voces-------->  programas
```

Los dos siguen la misma forma y no por gusto: **es la misma forma.** Un aparato
que solo puede tener un dueno, y un trabajo de reparto que el kernel no debe
hacer.

## Por que el kernel NO hace este trabajo

Porque componer ventanas y mezclar canales son **politica**, no mecanismo.

La tentacion evidente seria `INVOKE(fb, DRAW_RECT, ...)` y `INVOKE(audio,
PLAY_SOUND, ...)`. Las dos serian mas faciles de escribir y las dos serian el
mismo error: cada pixel y cada muestra cruzando el anillo, el kernel acabando
con un motor de dibujo y un mezclador dentro, y BMO-X siendo un monolito con la
etiqueta de microkernel puesta encima.

Lo que hace el kernel es **decir de quien es**. Lo que se hace con ello es de
Ring 3, y ahi se puede reescribir, sustituir o matar sin tocar el anillo 0.

★ Y hay una consecuencia que se nota: **un lider que muere no se lleva la
maquina.** `cap::revoke_all` recupera el aparato, la autopsia dice si quedo algo
sin devolver, y se relanza. Un mezclador dentro del kernel no tiene esa salida.

## La regla que los define

**Un lider TOMA el aparato cuando empieza a repartirlo y lo DEVUELVE cuando
deja de hacerlo.**

No es una recomendacion de estilo: es lo unico que impide que el lider deje
mudos --o ciegos-- a todos los programas que lanza.

Ya se aprendio por las malas con la pantalla. `gui.bex` la reclamaba al arrancar
y **no la soltaba nunca**, asi que `ray.bex` --el ensayo general de DOOM-- se
llevaba un *"la pantalla ya tiene dueno"*. El compositor tenia razon en no
cederla a cualquiera que la pida; lo que pasaba es que **no podia cederla ni
queriendo**, porque `PANTALLA_SOLTAR` no existia. Se escribio despues, con el
fallo delante.

Por eso la ventana F10 se escribio ya de la otra forma: toma `KIND_AUDIO` al
abrirse y lo devuelve al cerrarse. **Es huesped del aparato, no su dueno.**

## Nomenclatura: las logicas en INGLES

No es estetica. Es que la superficie de `obj/` ya esta entera en ingles
--`claim` `release` `grant` `resolve` `revoke` `owner` `operation`
`process_died`-- y un modulo que mezcle idiomas obliga a recordar de cual es
cada nombre antes de escribirlo. Eso se paga en fallos, no en gusto.

La frontera es esta, y es la que ya seguia el repo sin tenerla escrita:

| Capa | Idioma | Por que |
|---|---|---|
| `obj/`, contratos, protocolo | **ingles** | es vocabulario de sistema, y se compara con `bmo-abi` linea a linea |
| presentacion, ventanas, texto en pantalla | **espanol** | lo lee el dueno de la maquina |
| comentarios y documentacion | **espanol**, ASCII | ver `bmo-idioma-y-ascii` |

★ `obj/audio.rs` entro el 08-08 con `pitido_kernel`, `aparatos` y `calibrar` --
los tres unicos nombres en espanol de todo `obj/`. Son `kernel_beep`, `devices`
y `calibrate`.

---

# LIDER 1 -- `gui.bex`, la pantalla

## Lo que YA hace

Tiene `KIND_FRAMEBUFFER`, pinta con `mov` sin cruzar el anillo, lleva doble
bufer sobre `KIND_MEMORIA`, compone cuatro ventanas con foco y Z-order, y
**sabe prestar la pantalla** (`PANTALLA_SOLTAR` + `ENTRADA_SOLTAR`, que van
juntas: ceder la pantalla sin ceder la entrada es dejar a alguien pintando en
una habitacion cerrada).

## Lo que le falta para ser lider de verdad

| # | Casilla | Tam | Estado |
|---|---|---|---|
| 1.0 | ★ **Superficies**: un cliente pinta en SU buffer y el lider compone | XL | el mecanismo existe (`KIND_PRESTADO`), el protocolo no |
| 1.1 | `RESOLUTION`: preguntar los modos que hay | M | ⛔ hoy no hay lista: el modo lo fija el GOP al arrancar |
| 1.2 | `SET_MODE`: cambiarlo en caliente | L | ⛔ por 1.1, y necesita driver de GPU |
| 1.3 | Que un cambio de modo **rehaga** las cajas de todas las ventanas | M | por 1.2 |

### 1.1 y 1.2 -- la adaptacion de resolucion, y lo que de verdad la bloquea

Hoy la resolucion la fija el firmware: UEFI entrega un GOP con un modo ya
elegido, el kernel guarda `FB_WIDTH`/`FB_HEIGHT`/`FB_STRIDE` y **eso no se puede
cambiar mas**. `FB_OP_DIMS` es una lectura y no hay nada al lado que escriba.

Lo bueno: **el compositor ya no supone la resolucion.** Todas sus cajas se
calculan con `p.ancho`/`p.alto` y `.min()`, y la escala del texto sale de la
altura de la pantalla. Se reviso entero el 08-09 y **no hay ni un numero de
resolucion clavado** en `services/gui/`.

★ Lo malo esta en el KERNEL, y es lo unico que hay que arreglar antes de que
llegue la GPU:

```
ring0/dev/framebuffer.rs:182
    const BACKBUFFER_WIDTH:  usize = 1920;
    const BACKBUFFER_HEIGHT: usize = 1080;
    static mut BACKBUFFER_MEM: [u32; 1920*1080] = [0; ...];
```

Son **8 MiB de `static mut`** declarados con la resolucion clavada dentro, y
`get_backbuffer_fb` recorta con `.min()` si la pantalla es mayor: en 2560x1440
se veria **la esquina**, sin un solo error.

Y lo que lo convierte en un caso claro: **no lo llama NADIE.** El barrido del
08-09 no encontro un solo uso de `get_backbuffer_fb`, `backbuffer_ptr` ni
`present` fuera del propio fichero.

★ **Pero el numero desmiente el motivo, y hay que decirlo.** Se borro y se midio
el `.bss` antes y despues:

```
   .bss ANTES:    2.608.160 bytes
   .bss DESPUES:  2.608.160 bytes
   AHORRO:        0
```

**Cero.** El enlazador ya lo descartaba, y por el mismo motivo por el que
sobraba: nadie lo referenciaba, asi que ni el static ni sus seis funciones
llegaban a la imagen. Los 8 MiB nunca existieron en la maquina.

Asi que borrarlo no fue una optimizacion: fue **quitar una trampa**. Mientras el
codigo estuviera ahi, la primera llamada a `present()` habria traido de golpe 8
MiB de `.bss` y un recorte silencioso a 1080 lineas -- y quien la escribiera no
tendria por que saberlo. **Codigo muerto que es gratis hoy y caro el dia que
alguien lo despierta.**

Y deja una leccion medible para el resto de esta lista: *"esto ocupa"* es una
hipotesis hasta que se mira el `.bss`. La optimizacion de verdad de este kernel
no esta en lo que sobra --el enlazador ya se lo come-- sino en lo que se usa.

---

# LIDER 2 -- `audio.bex`, el sonido

## Lo que hay hoy

`KIND_AUDIO` (el contrato), `<bmo/sonido.h>` y `<bmo/musica.h>` (las librerias),
y la ventana F10. **No hay servidor**: hoy cada programa reclama el aparato
entero para el, y mientras lo tiene nadie mas suena.

Eso basta para un pitido. No basta para un escritorio.

## Por que hace falta un lider, con el caso concreto

Hoy, si el escritorio abre F10 y a la vez corre `c/musica.bex`, **uno de los dos
se queda sin sonido** -- y el que pierda no puede hacer nada al respecto. Es el
mismo problema que la pantalla tenia antes del compositor: un aparato, muchos
que lo quieren, y ninguna politica que no sea "el primero que llegue".

## El protocolo, en ingles

Cuatro operaciones sobre un endpoint RPC (`KIND_ENDPOINT` ya existe), y ni una
mas hasta que haga falta:

| Operacion | Que hace |
|---|---|
| `voice_open(format) -> voice` | pide una voz. El servidor decide si la da |
| `voice_write(voice, buffer)` | entrega muestras por `KIND_PRESTADO` |
| `voice_gain(voice, gain)` | ganancia de ESA voz, no del aparato |
| `voice_close(voice)` | la devuelve |

★ **El cliente nunca toca el aparato.** Pide una voz y escribe muestras; el
servidor mezcla y es el unico que habla con `KIND_AUDIO`. Exactamente el trato
que tendran las superficies con la pantalla.

## Las casillas

| # | Casilla | Tam | Estado |
|---|---|---|---|
| 2.0 | `audio.bex` toma `KIND_AUDIO` y atiende un endpoint | M | libre |
| 2.1 | Arbitraje: quien suena cuando dos piden a la vez | S | por 2.0 |
| 2.2 | ★ El MEZCLADOR de verdad | L | ⛔ **por 2.4**: con el altavoz del PC no hay nada que mezclar |
| 2.3 | `voice_gain` + limitador `tanh` (el "boost" sin recorte) | S | por 2.2 |
| 2.4 | ⛔ **UN APARATO QUE ACEPTE MUESTRAS** | XL | es la casilla que decide la fase |

### ★ 2.4 -- y aqui la eleccion cambio

El plan de DOOM decia *"HD Audio, que en este Ryzen es HDA"*. El diagnostico del
08-08 lo corrige: **el aparato del dueno es USB**.

```
VID_1B3F&PID_2008    USB\Class_01&SubClass_01    = USB Audio Class 1.0
```

Y el codec de la placa (Realtek ALC897) tiene sus salidas analogicas sin nada
enchufado. O sea que **escribir el driver HDA seria una pieza XL para no oir
nada**.

| Camino | A favor | En contra |
|---|---|---|
| **HDA** | autocontenido; no toca el driver USB que tanto costo estabilizar | va a unos jacks donde no hay nada conectado |
| **USB Audio** | es el aparato que el dueno usa de verdad; xHCI **ya funciona** | pide transferencias **isocronas**, que hoy no existen (hay control e interrupt) |

⚠ **La decision esta tomada a favor de USB**, con una condicion que hay que
comprobar antes de escribir codigo: que no haya altavoces en el jack verde.

★ Y hay una casilla **anterior a las dos**, que se puede hacer ya:

> **El volumen de un aparato USB Audio es un `SET_CUR` sobre el Feature Unit de
> su interfaz de control: un CONTROL TRANSFER.** Y `bmo-xhci` ya sabe hacerlos.

O sea que **BMO-X puede mandar sobre el volumen del audifono antes de poder
reproducir una sola muestra**. Es pequena, es visible en el aparato fisico, y no
depende de 2.4.

## La cuenta

| Lider | Casillas | Faltan | Lo que bloquea |
|---|---|---|---|
| `gui.bex` | 4 | 4 | superficies (1.0); la resolucion espera a la GPU |
| `audio.bex` | 5 | 5 | **2.4**: un aparato que acepte muestras |

**Lo que se puede hacer sin esperar a nada**: el volumen por control transfer del
USB. La casilla 1.1 ya no esta en esta lista -- el backbuffer se borro el 08-09.

---

Ver [`PLAN_DOOM.md`](PLAN_DOOM.md) para el orden de DOOM, y
[`QUE_DESBLOQUEA.md`](QUE_DESBLOQUEA.md) para el censo de lo que falta.
