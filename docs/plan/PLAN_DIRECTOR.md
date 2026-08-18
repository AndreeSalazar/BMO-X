# DIRECTOR -- de compositor a administrador

> Escrito el **2026-08-10**. `gui.bex` pasa a llamarse **DIRECTOR** cuando haga
> las cuatro cosas de abajo, **no antes**: un nombre describe algo hecho, no una
> intencion.
>
> El nombre es el de la orquesta. No toca ningun instrumento: **decide quien
> suena, cuando y cuanto.** Reparte la pantalla, da el marco, decide el foco y
> --con el paso 3-- decide quien corre antes.

## Lo que YA es verdad, para no reconstruirlo

| | |
|---|---|
| Aislamiento | Un fallo en Ring 3 mata la tarea y el sistema sigue |
| Memoria sin mezclar | Espacio de direcciones por proceso, `KIND_MEMORIA` propio, `revoke_all` al morir |
| Cadena de custodia | `fb::proceso_muerto` recupera la pantalla **y pinta las ultimas cuatro lineas del muerto** |
| El ultimo recurso | `Ctrl+Alt+ESC` en Ring 0, en el punto unico por donde pasan todas las teclas |
| El marco | `scene/chrome.rs`: minimizar, maximizar, cerrar, fichas y arrastre. En metal desde el 06-08 |
| Prestamo de memoria | `loan::offer` + `loan::take` + `loan::process_died` + **soltar**. Completo |
| El contrato de superficie | `<bmo/superficie.h>`, formato `BSUP`. **Hecho** (`de3a74b9`) |
| `TASK_OP_MI_PADRE = 0x26` | **Hecho** -- `ring0/task/familia.rs` + `scheduler::tid_de` |
| El lado del DIRECTOR | **Hecho** -- `scene/surface.rs`. Compila; **falta metal** |

---

# ~~PASO 1~~ -- `TASK_OP_MI_PADRE = 0x26` ✅ HECHO

Una superficie se le ofrece al DIRECTOR, y el programa **no tiene otra forma de
nombrarlo**. Donde quedo cada pieza:

1. **`ring0/task/familia.rs`** -- modulo hermano de `paquete.rs`, no dos columnas
   dentro. El motivo salio al escribirlo: `paquete::recordar` **se rinde cuando
   la ruta no cabe** en `RUTA_MAX`, y con razon. Compartiendo tabla, un programa
   con la ruta larga se quedaria ademas sin poder pintar en una ventana, y el
   sintoma --*"esta app no compone y las demas si"*-- no lleva a la causa.
   **Dos datos con motivos distintos para faltar son dos tablas.**
2. **Quien lo apunta: `syscall.rs`, en el brazo de `EJECUTAR`** -- y **no**
   `lanzar.rs`, como decia el plan. `lanzar::ruta` lo comparten el syscall y el
   shell del kernel: mirando `current_pid()` desde dentro, un `run` tecleado por
   el puerto serie le pondria de padre **a la tarea que estuviera corriendo**,
   tipicamente el compositor. Un dato falso es peor que ninguno.
3. **`scheduler::tid_de(pid)`** -- el inverso de `pid_de`. Y devolver `None` para
   un proceso muerto no es un hueco: es lo que convierte esta pregunta en **el
   detector de vida** del que tira `PRESTADO_OP_DUENO`.
4. **`syscall.rs`**: `0x26` y su brazo. El guardian de opcodes dice ahora
   **`38 opcodes, ninguno repetido`**, como estaba previsto.
5. **`bmo-abi/.../surface.rs`** y **`userland/src/lib.rs`**: el mismo id en los
   tres. Y de paso entraron en el ABI las cuatro operaciones del prestamo, que
   estaban solo en el kernel y en el userland.

**Devuelve `0` si no hay padre** --lanzado desde el shell de Ring 0-- y eso NO
es un error: es la respuesta correcta a "quien compone para mi".

---

# ~~PASO 2~~ -- El lado del DIRECTOR ✅ HECHO (falta metal)

`Ultra_userspace/services/gui/src/scene/surface.rs`. Una `Table` de cuatro
cajas, y en el bucle principal tres puntos: **recoger** al principio,
**componer** al final --justo antes del cursor del raton, que es lo unico que va
por encima-- y el raton en medio.

1. **Tomar**: `bmo::tomar_prestado_de()` devuelve `(handle, base, bytes)`. Una
   vez por vuelta; casi siempre dice que no.
2. **Leer la cabecera** `BSUP` y **no creersela**: ver el paso 2.5.
3. **Pegar solo si la secuencia cambio**, que es la regla entera. Un fotograma a
   medias no cambia el numero, asi que no se pinta, y el peor caso es ensenar el
   anterior un fotograma mas. ** NO es un cerrojo y no debe serlo.
4. **Dentro del marco** que `scene/chrome.rs` ya dibuja: `Chrome::for_content`
   --el unico constructor en pixeles, porque el tamano lo eligio la app-- y los
   tres botones salen gratis. **Ese fue el cobro de haber escrito `marco.rs`.**
5. **Pantalla completa = no dibujar el borde.** Sigue pendiente: hoy maximizar da
   el hueco bajo la barra, como las demas. Lo que ya es cierto es lo que
   importaba: **no se entrega el aparato**, asi que un juego colgado se cierra
   con el teclado.

## ★ 2.5 -- Los tres numeros que el DIRECTOR NO se cree

La cabecera la escribe **otro proceso**. Una app que declare `4000 x 4000` dentro
de un bloque de 1 MiB --por un fallo o a proposito-- leeria fuera del prestamo, y
**el fallo de pagina lo cobra el compositor**, no ella: una app rota se lleva el
escritorio, que es justo lo que este diseno existe para impedir.

`Cabecera::leer` comprueba que **lo que la cabecera declara cabe en los bytes que
el kernel dijo que presto**, en `u64` --en 32 bits `stride * alto * 4` se
desborda y da un total pequeno, o sea que la comprobacion pasaria justo en el
caso que tiene que parar--. Es la unica frontera de confianza del modulo y va
toda en una funcion a proposito.

## Y una app que MUERE

Se pregunta cada fotograma con `PRESTADO_OP_DUENO`, y no por prudencia: una app
muerta deja la secuencia **congelada**, que es indistinguible de una app
pensando. Sin esa pregunta, la ventana de un programa que ya no existe se
quedaria en pantalla con su ultimo fotograma y sus tres botones, como si fuera a
responder.

---

# ★ LO QUE FALTABA EN LA RAM, Y ERA MAS DE LO QUE PARECIA

`MEM_OP_OFRECER` + `TASK_OP_TOMAR` bastaban **para una**. El DIRECTOR necesita
una por ventana, y ahi aparecieron tres cosas que no estaban:

| Lo que faltaba | Por que |
|---|---|
| **Que dos prestamos no se pisen** | `take` mapeaba SIEMPRE en `PRESTAMO_VA_BASE`. El segundo caia encima del primero, y como la capability se concede con la VA como objeto, dos handles distintos contestaban lo del otro. **La segunda ventana ensenaria los pixeles de la primera y nada fallaria.** Ahora la direccion la decide la ranura: `BASE + ranura * 64 MiB` |
| **`PRESTADO_OP_DUENO` (0x03)** | preguntar si el que presto sigue vivo. Sin esto no hay forma de distinguir una app muerta de una app pensando |
| **`PRESTADO_OP_SOLTAR` (0x04)** | devolverlo. Sin esto, abrir y cerrar ventanas agota las ranuras y a partir de ahi ninguna app vuelve a tener caja hasta reiniciar |

Mas `MAX` de 8 a 16 ranuras, que es el mismo censo que `paquete` y `familia`.

## ★★ Y la decision que sostiene el modelo: **el prestamo sobrevive al que lo presto**

Cuando muere el dueno de algo ya tomado, lo tentador es quitarselo al que lo tomo
--tenemos su `cr3` con `scheduler::cr3_de_pid`-- y es justo lo que no se puede
hacer: **el que lo tomo es el DIRECTOR, y esta componiendo**. Desmapearle paginas
por debajo mientras las recorre es un fallo de pagina en el compositor, o sea que
**una app que se cierra se lleva el escritorio**.

Asi que la oferta queda **huerfana**: los marcos siguen siendo validos
--`destroy_address_space` libera las tablas de paginas, no las hojas-- y el
DIRECTOR lo suelta cuando quiere, avisado por `OP_DUENO`. Al lado de una ventana
congelada un fotograma de mas, no hay debate.

## Lo que sigue pendiente de la RAM, y no bloquea esto

Los escalones 2 a 7 de `LA_RAM.md`, con uno que si roza a las apps en caja:
**`lanzar.rs::con_buffer` lee el fichero entero a un estatico de 4 MiB**, asi que
una app grande sigue sin arrancar -- tenga ventana o no. Y del lado del programa,
la superficie sale del monton: `BMO_MONTON_BYTES` (1 MiB por defecto) tiene que
declararse bastante para la imagen, o `malloc` devuelve 0 y no hay ventana.

---

# ★ PASO 2b -- LO SIGUIENTE: portar `ray.bex`

La prueba de que esto vale. Dibuja en la superficie en vez de en el framebuffer y
aparece en una caja con sus tres botones. Cuatro cosas, en orden:

1. `#include <stdlib.h>` y `#define BMO_MONTON_BYTES` con sitio para la imagen.
2. `bmo_superficie_crear(ancho, alto)`. **Si devuelve 0, no es un fallo**: nadie
   compone, y se sigue por el camino de la pantalla exclusiva que ya funciona.
3. Pintar en `bmo_superficie_pixeles(s)` con el `ancho` de la superficie como
   stride, en vez de en el panel.
4. `bmo_superficie_lista(s)` **despues del ultimo pixel**, y `bmo_ceder()`.

⚠ Y hay una trampa apuntada de antes que aplica aqui: **el mapa del raycaster
valia CERO**, asi que `ray.bex` va a dibujar otro laberinto del que se recuerda.
Que la ventana ensene algo distinto no quiere decir que la superficie falle.

---

# ★★ PASO 2c -- LA ENTRADA: hoy una app puede ENSENAR, no la puedes TOCAR

> Anadido el **2026-08-18**, al preguntar el dueno por que la calculadora no
> puede ser `apps/calculadora.bex` con su icono. La respuesta salio de leer este
> mismo plan: **los pasos 1, 2, 2b, 3, 4 y 5 hablan todos de PIXELES**. Ninguno
> manda un clic hacia dentro.

```
   HECHO     la app dibuja en su memoria  ->  el DIRECTOR la pega en un marco
   FALTA     el dedo del usuario          ->  la app
```

Por eso DOOM funciona y una calculadora en una ventana no puede: DOOM se lleva
la pantalla ENTERA y con ella el teclado. Es el modelo viejo --relevo, no caja--
y no sirve para nada que quiera compartir el escritorio.

★ **Esto no desbloquea la calculadora: desbloquea la primera app normal de
BMO-X, sea cual sea.** La calculadora solo es el mejor primer cliente que hay,
porque lo caro ya esta hecho y probado: 20 rects, una isla y una tabla de golpeo
que sale del mismo arbol que se pinta.

## 2c.1 -- Traducir el golpe. **No la bloquea nada.**

El DIRECTOR sabe donde pego cada superficie, asi que un clic en `(px, py)` de
pantalla es un clic en `(px - ox, py - oy)` de la app. Es una resta, y es la
misma que `calc_gen::golpe` ya hace dentro de la calculadora.

⚠ Y la regla que se hereda del paso 2.5: **las coordenadas que salen tienen que
caer dentro de la superficie que la app declaro**. Mandarle un clic en `(5000,
5000)` a una app de 322x446 es darle un numero que no puede significar nada.

**Como se sabe que quedo hecha**: el DIRECTOR sabe decir *"este clic es de la
superficie 2, en su pixel (81, 210)"* sin que la app exista todavia.

## 2c.2 -- ★★ POR DONDE VIAJA UN EVENTO. Aqui hay que ELEGIR.

Y no por la consola, que es lo que acaba de costar un fallo mudo -- ver
`docs/maestro/IPC_MAESTRO.md`. Dos caminos, los dos con piezas ya construidas:

### A. Por un ENDPOINT (`TASK_OP_ENDPOINT_CREATE` / `_CONNECT`)

Existe, y **sus tres guardias ya se probaron en hardware** con
`toolchain/tools/rpc-demo`. Es Ring 3 contra Ring 3, que es exactamente esta
conversacion.

- **a favor**: no hay nada que inventar, y el kernel garantiza la entrega.
- **en contra**: **969 ciclos por evento** (ver `docs/componente/LA_PUERTA_POR_DENTRO.md`).
  Para una calculadora eso es gratis --un clic cada varios segundos--; para algo
  que siga al raton, es el precio equivocado.

### B. Por un ANILLO ESCRITO DIRECTAMENTE, sin pasar por el kernel

Cero syscalls por evento: el DIRECTOR escribiria en la pagina de canal de la app
con un `mov`, igual que hoy LEE sus pixeles con un `mov`. Es el prestamo de
memoria del paso 2 **espejado**.

- **a favor**: gratis por evento, y el enmarcado sigue siendo imposible de
  romper -- una ranura ES un mensaje.
- **en contra**: la pagina del canal de la app tendria que estar mapeada en el
  DIRECTOR, y eso es autoridad nueva sobre un proceso ajeno. No es un
  renombrado: es una frontera de confianza mas.

## ★★ 2c.2b -- CORRECCION DEL MISMO DIA: A **YA ESTA CONSTRUIDO**

Lo de arriba se escribio diciendo que habia que *"ver si `bmo-channel` sirve
entre dos Ring 3"*. Se miro, y la pregunta estaba mal hecha.

★★ **`bmo-channel` NUNCA fue "Ring 0 contra Ring 3": ese nombre dice quien
ESCRIBE la pagina, no de donde viene el mensaje.** Su propio `ring0_complete` lo
tiene escrito desde que se escribio:

> *"Existe para Endpoint RPC: una llamada de otro proceso no entra por el anillo
> de submissions de ESTE servidor --el que la hizo tiene el suyo--, pero se le
> entrega por el mismo camino por el que ya lee todo lo demas. **Asi el servidor
> no necesita un segundo mecanismo de recepcion.**"*

Y el kernel ya lo hace: `ring0/obj/endpoint.rs`, funcion `publish` -- escribe en
el anillo de completions del servidor **por el physmap**, y el servidor lo
recoge con el mismo `ring3_poll` que usa para todo.

```
   proceso A llama  ->  el kernel PUBLICA en el anillo de B  ->  B hace ring3_poll
```

**Eso es Ring 3 contra Ring 3 y lleva funcionando desde `rpc-demo`.** El camino A
no hay que construirlo: hay que USARLO.

### Lo que queda de diferencia, que es poco y concreto

1. **RPC es una llamada que espera respuesta**; un evento de entrada no espera
   nada. El endpoint concede un derecho de respuesta **one-shot en cada
   llamada** (`cap::grant(KIND_REPLY)`), y para un clic eso sobra. Publicar sin
   conceder la respuesta ya lo sabe hacer `ring0_complete`; lo que hay que ver
   es por donde se pide eso sin inventar una operacion nueva.
2. **El precio se queda**: sigue siendo una puerta por evento. Para una
   calculadora es gratis --un clic cada varios segundos--; a 60 fotogramas
   siguiendo al raton, no.

★ **Asi que la eleccion entre A y B deja de ser arquitectonica y pasa a ser un
NUMERO: cuantos eventos por segundo.** Y para el primer cliente --la
calculadora-- la respuesta es A, sin discusion y sin escribir una linea de
mecanismo nuevo.

**Que la bloquea**: la pregunta 1, que es de leer `endpoint.rs`, no de disenar.

**Como se sabe que quedo hecha**: una app con superficie recibe un clic por su
anillo y no necesita un segundo mecanismo de recepcion -- que es literalmente lo
que el comentario de `ring0_complete` prometio.

**Como se sabe que quedo hecha**: `ray.bex` --ya portado en 2b-- reacciona a una
tecla dentro de su ventana sin llevarse la pantalla.

## 2c.3 -- De quien son las teclas, que ya esta contestado

No hace falta politica nueva: **`bmo_input::foco` ya decide quien tiene el
teclado**, y es lo mismo que gobierna las ventanas del escritorio. Una app con
superficie es una ventana mas.

★ Y la regla ya se escribio dos veces en esta casa --la consola de ESTRATOS y la
calculadora--: *dos duenos para una tecla se resuelve con un ORDEN, nunca con
una heuristica*. Aqui el orden lo da el foco.

## 2c.4 -- Y una app que no contesta

Un evento mandado a un proceso muerto no puede colgar al DIRECTOR. Ya hay con
que preguntarlo (`PRESTADO_OP_DUENO`, del paso 2), asi que es la misma pregunta
en el mismo sitio, no una nueva.

⚠ Con el camino A esto importa mas: una llamada bloqueante a una app que no
responde **para el escritorio entero**. Si se elige A, la entrega tiene que
poder rendirse.

## 2c.5 -- Y ENTONCES `apps/calculadora.bex`

Con 2c hecho, la app suelta es **mudanza, no invencion**:

```
   la cara        calc_gen.rs ya pinta contra un origen, no contra la pantalla
   el motor       cobol/calcgui.bex no se entera de nada
   el estado      scene/calc.rs sale del compositor tal cual
   el icono       BICO + bmo-pack, y el clic que lanza, ya funcionan
```

★ Lo unico de verdad nuevo seria que la calculadora **pinte en su superficie en
vez de en la `Pantalla`** -- y eso es cambiar a que le pasa el origen, porque el
emisor de MAQUETA nunca supo donde estaba la ventana.

---

# PASO 3 -- Cerrar sin ser root

*"opcion para cerrar fuerte"* suena a boton y es **autoridad**: matar un proceso
ajeno. Si el DIRECTOR puede matar a cualquiera *porque es el DIRECTOR*, eso es
`root` con otro nombre -- en el sistema cuya primera clausula dice que la
autoridad no se hereda.

**Lo correcto**: `EJECUTAR` devuelve un **handle sobre el hijo**, y matar es una
operacion de ese handle. El DIRECTOR cierra lo que el lanzo porque tiene su
handle, no porque sea especial.

★ Y eso da gratis el panel de la derecha: **la lista de apps activas ES la lista
de handles que el DIRECTOR tiene**. No hay que preguntarle al kernel quien
existe -- ya lo sabe, porque el los abrio.

`Ctrl+Alt+ESC` sigue siendo el que no se puede quitar, porque vive en Ring 0 y
no depende de que nadie este vivo.

---

# PASO 4 -- Prioridad por FOCO

> **La prioridad no es un atributo del proceso. Es una consecuencia de a donde
> mira el usuario.**

En Linux hay `nice()`, en Windows `SetPriorityClass`: un numero que el programa
**se pide a si mismo**, y por eso todo el mundo se pone alto y el numero deja de
significar nada.

Aqui no hace falta esa API. El DIRECTOR ya sabe quien tiene el foco
(`bmo_input::foco`), y el foco lo decide el usuario apuntando. **Una app no
puede subirse la prioridad porque no hay donde pedirla: la gana estando
delante.** Una operacion mas hacia el planificador, no un sistema nuevo.

---

# PASO 5 -- El rename

`sys/gui.bex` -> `sys/director.bex`. Al final, cuando los cuatro esten hechos.

Toca: `build.ps1` (el destino de `bex-link`), el arranque del kernel que lo
lanza, y `Ultra_userspace/services/gui/` -> `services/director/`.
