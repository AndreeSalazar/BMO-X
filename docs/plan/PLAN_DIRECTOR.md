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

`Ultra_userspace/services/director/src/scene/surface.rs`. Una `Table` de cuatro
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

# ★ PASO 2b -- portar `ray.bex` a una ventana ✅ HECHO

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

## ★ HECHO el 2026-08-19 -- y con dos cosas que no estaban en la lista

`raycaster_C.c` compila a **19.394 bytes** con los dos caminos dentro: pide
ventana primero y **solo si no hay compositor** toma la pantalla entera.

1. ★★ **El orden de los cuatro pasos estaba al reves de como se escribio.** La
   superficie se pide ANTES de reclamar la pantalla, no despues: mientras el
   escritorio viva, `PANTALLA_RECLAMAR` contesta que no, asi que preguntar
   primero por ahi es preguntar por el camino que casi nunca esta abierto.

2. ★★ **En ventana NO se reclama la entrada, y eso es la casilla 4 en vivo.**
   `ENTRADA_RECLAMAR` es de la pantalla entera: pedirla desde dentro de una caja
   le quitaria el teclado al escritorio, que es el modelo viejo del que esto
   sale. Asi que `ray.bex` en ventana **se mira y no se toca** -- literalmente el
   estado que 2c existe para cambiar. Tampoco pinta las barras de "como se sale":
   la salida es el boton de cerrar del marco, que ya lo pone el DIRECTOR.

[!] Y el fallo de REX que destapo, porque es el primer programa que pidio
superficie sin pedir ficheros: **`superficie.h` leia `__bmo_bloque_cap` sin
traerlo** --lo declaraba `archivo.h`-- asi que una app que solo queria una
ventana no compilaba, y el error nombraba un simbolo con dos guiones bajos que
el programa no habia escrito nunca. De ahi sale `<bmo/bloque.h>`.

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

### ★ HECHA el 2026-08-19

`platform/shared/bmo-golpe` --nueve filas en verde-- y `Table::golpe` en el
compositor. Tres cosas que salieron al escribirla y no estaban previstas:

1. **Fuera no es (0,0): fuera es que no hay golpe.** La resta devuelve
   `Option`. Un saturado convertiria un clic de fuera en un clic en el borde, y
   cada app tendria que descubrir por su cuenta que ese cero era mentira.
2. **El recorte del golpe y el de los pixeles son el MISMO codigo.** `compose`
   pasa a usar la misma `visible()`. Dos copias de esa cuenta serian un borde
   donde se ve una cosa y se pulsa otra -- y eso no da error, da un numero.
3. **Se comprueban las DOS cajas**: la visible y la que la app declaro. Lo
   segundo es dato de otro proceso y se usa solo como tope, asi que la frontera
   de confianza no se reparte entre dos modulos.

★ Y vive fuera del `.bex` por L7b: es un **hijo** --relaciona la caja con el
punto y no sabe que significa el resultado-- y ahi se prueba en tres segundos en
vez de en una tanda de flasheo.

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

## ★★ 2c.4b -- EL ORDEN, y por que la calculadora va la ULTIMA

Cuatro pasos, y cada uno se puede comprobar **solo**:

```
   1  leer `endpoint.rs`     publicar un evento SIN conceder derecho de respuesta
   2  traducir el golpe      clic de pantalla -> clic de la app (una resta)
   3  `ray.bex` RECIBE       una tecla dentro de su ventana, sin llevarse la pantalla
   4  `calculadora.bex`      la mudanza
```

★ **El 3 es la prueba, y va antes que el 4 a proposito.** `ray.bex` ya esta
portado a superficie (paso 2b) y no tiene estado que perder: si recibe una tecla,
la entrada funciona. Si se hiciera al reves --mudar primero la calculadora-- un
fallo no diria si es de la entrada o de la mudanza, y se depuraria en dos sitios
a la vez.

Es la misma disciplina que la sonda de DOOM: **el instrumento antes que el
paciente**.

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

---

# ★★★ 2c.3 -- `ray.bex` RECIBE UNA TECLA EN SU VENTANA. HECHO el 2026-08-23

> Pedido asi por el dueno: *"solo iniciar con el DOOM en Ring 3 tiene que ser
> como uso MUY GENERALES de app, ventanas, TODO ESO"*. Y con el remate que
> convirtio la casilla en algo que se puede mirar: *"dale con menu y
> configuracion con tecla"*.

## ★★ LA ELECCION ENTRE A Y B SE DESHIZO SOLA, Y NO POR UN NUMERO

La seccion 2c.2 dejaba dos caminos abiertos y le ponia a **B** un precio
concreto:

> *"la pagina del canal de la app tendria que estar mapeada en el DIRECTOR, y
> eso es autoridad nueva sobre un proceso ajeno. No es un renombrado: es una
> frontera de confianza mas."*

**Eso ya no era cierto cuando se escribio.** `loan::take` mapea el bloque
ofrecido y concede la capability con `RIGHT_READ | RIGHT_WRITE` -- o sea que
desde el paso 2, el DIRECTOR **ya podia escribir** en la memoria de la app. La
autoridad no hay que crearla: **la concede la propia app al ofrecer la
superficie**, y lo unico que faltaba era un sitio acordado donde dejar la tecla.

```text
   coste de B, como estaba escrito     una frontera de confianza mas
   coste de B, medido                  CERO. Ya estaba concedida.
```

★ Y con eso el numero que el plan decia que decidiria la eleccion --*"cuantos
eventos por segundo"*-- **deja de existir**: B no cuesta un syscall por evento,
asi que no hay frecuencia a partir de la cual deje de valer.

## EL BUZON: donde vive, y por que ahi

Dentro de la propia superficie, declarado en los dos `u32` que la cabecera
`BSUP` tenia **reservados a cero** desde el primer dia:

```text
   24..28   BUZON: donde empieza, en bytes desde el principio. 0 = no hay
   28..32   BUZON: cuantas ranuras (potencia de 2)

   y el buzon, detras de los pixeles:
    +0   CABEZA (u32)   la escribe el DIRECTOR
    +4   COLA   (u32)   la escribe la app
    +8   las ranuras, de 8 bytes: un evento CRUDO, el mismo `unsigned long
         long` que devuelve `bmo_entrada_evento`
```

Es la misma decision que la cabecera y que `BICO` dentro del paquete: **el dato
dice lo que es, y quien lo transporta no necesita entenderlo.** El kernel presta
bytes y no se entera de que ahora hay un buzon dentro.

★★ **Y ES OPCIONAL, que es lo que decide quien se queda las teclas.**
`bmo_superficie_crear` sigue sin pedirlo; hay que llamar a
`bmo_superficie_crear_con_buzon`. Una app que solo ensena --un reloj, un
medidor-- no lo declara, y entonces el DIRECTOR no le manda nada y el escritorio
conserva el teclado. **Pedirlo es decir "yo se leer".**

## DE QUIEN ES UNA TECLA: el orden, escrito una vez

La regla de la casa --*dos duenos para una tecla se resuelve con un ORDEN, nunca
con una heuristica*-- aplicada en `desktop::keys::app`:

```text
   1. las del ESCRITORIO      F1..F12 y cualquier cosa con Alt pulsado
   2. las de la APP con foco  todo lo demas, si declaro buzon
   3. nadie                   y entonces se descartan
```

⚠ La lista del 1 es corta y **cerrada**. Una app que se quedara tambien con
Alt+Tab y con las F seria el modelo viejo otra vez --el que entrega el aparato--
y de ese no se vuelve sin el boton de reset. `Ctrl+Alt+ESC` no esta en la lista
porque no le hace falta: vive en Ring 0.

★ Y hay una cosa que **se drena siempre**, tenga foco una app o no: la cola
cruda. Si solo se vaciara cuando hay a quien entregar, una racha tecleada contra
el escritorio se quedaria dentro y la app que ganara el foco despues recibiria
de golpe teclas viejas -- pulsaciones que el usuario dio a otra cosa. Una cola
que solo se vacia a veces es peor que no tenerla.

## ★ LAS DOS COLAS, que es lo que hace que esto no le quite nada al escritorio

`KIND_ENTRADA` tiene **dos** colas y se llenan del mismo sondeo: la de
CARACTERES --que el escritorio cocina para su linea de Ejecutar-- y la de
EVENTOS CRUDOS, que es la que se reenvia. **Leer una no le roba nada a la otra**,
asi que el escritorio sigue teniendo sus atajos mientras la app recibe la tecla
entera, con su flanco.

Y el flanco hacia falta: **un caracter no tiene SOLTAR.** Sin el, cada pulsacion
contaria dos veces o ninguna, y no se puede escribir nada que reaccione a una
tecla mantenida -- que es la mitad de lo que hace un juego.

## COMO QUEDO, pieza a pieza

```text
   <bmo/superficie.h>          `..._crear_con_buzon` y `..._evento`. REX.
   scene::surface::Header      el buzon pasa por la MISMA aduana que el resto
                               de la cabecera: si no cabe en lo prestado, no
                               existe -- y una app sin buzon no recibe nada
   scene::surface::publicar    cuatro escrituras a memoria. Cero puertas
   desktop::keys::app          el orden de quien es cada tecla, y el drenado
   bmo::Entrada::evento()      la cola cruda, que en Rust no estaba expuesta
   raycaster_C.c               el primer cliente: menu y ajustes
```

★★ **La aduana se queda donde ya estaba.** `Header::read` era *"la unica
frontera de confianza del modulo, y va toda en una funcion a proposito"*, y el
buzon se valida ahi dentro y no donde se escribe. Meter una segunda
comprobacion en otro sitio seria abrir una segunda puerta que alguien tendria
que acordarse de cerrar.

Y la asimetria es deliberada en los dos sentidos: **ninguno de los dos se cree
el indice del otro.** El DIRECTOR escribe con la CABEZA, que es suya y enmascara;
la app lee con la COLA, que es suya y enmascara. Un indice con basura puede
hacer que se pierda o se repita un evento --y solo le duele a quien lo escribio
mal-- pero nunca que se lea o se escriba fuera del prestamo.

## EL PRIMER CLIENTE: `ray.bex`, con menu y ajustes

`M` abre el menu; flechas o `WASD` navegan y cambian; `ESC` lo cierra. Tres
ajustes, y los tres se ven en el mismo fotograma:

```text
   campo de vision   estrecho / normal / ancho   toca el plano de camara
   velocidad         lenta / normal / rapida     toca el paso
   tema              noche / normal / claro      toca techo y suelo
```

★ No hay texto: **son barras**. REX no trae fuente para una app de C, y
dibujarla aqui seria meter una fuente en un ejemplo. Una fila por ajuste, tantos
segmentos encendidos como vale, y la fila senalada con su marca. Se lee de un
vistazo y no promete un idioma que este programa no sabe escribir.

★★ **Y lo que de verdad prueba el menu no es el menu**: es que una app en una
caja tiene ESTADO que cambia con el teclado y se ve cambiar. Una tecla que mueve
al personaje podria ser un movimiento inercial; una opcion que se queda puesta
solo puede venir de una tecla que llego, se entendio y se guardo.

⚠ Y `ESC` **no cierra la ventana**: solo sale cuando el programa tiene la
pantalla entera. En una caja la salida es el boton del marco, que lo pone el
DIRECTOR -- una app que decidiera cuando se la puede cerrar seria el modelo
viejo.

```text
   aprobado:  `ray.bex` en una ventana, `M` abre el menu, las flechas cambian
              un ajuste y se ve en el mismo fotograma; y el escritorio sigue
              respondiendo a F7 y a Alt+Tab mientras tanto.
```

⚠ **Nada de esto lo ha visto un CPU.** Compila --`ray.bex` pasa de 19.437 a
27.415 bytes-- y el DIRECTOR enlaza, pero un buzon entre dos procesos sobre
memoria compartida es exactamente la clase de cosa que un emulador no prueba.
Ver `../metal/PRUEBA_EN_METAL_0823.md`.

## EL RATON, el mismo dia y por el mismo camino

`Table::golpe` llevaba desde el 19-08 escrito y **sin llamante**, con su
`#[allow(dead_code)]` y el motivo al lado: *"2c.1 se entrega SOLA para que su
fallo no se confunda con el del transporte"*. El transporte llego cuatro dias
despues y **no hubo que tocar una linea de esa funcion**.

```text
   bit 63       1 = raton, 0 = tecla
   bit 8        HAY, en los dos
   bit 9        PULSADA: la tecla baja, o el boton baja
   bits 0..7    el scancode, o la mascara de BOTONES
   bits 16..31  x dentro de la app, en pixeles suyos
   bits 32..47  y
```

★★ **Una tecla sigue siendo, bit a bit, lo que devuelve `bmo_entrada_evento`**
--el kernel nunca enciende el 63-- asi que el codigo que ya sabia leer teclas
vale sin tocar una coma. Y por eso mismo el bit **tiene que preguntarse**: en un
evento de raton el byte bajo son los BOTONES, y leerlo como scancode haria que
cada clic pareciera la tecla numero 1. Un fallo que no da error: da un
movimiento. De ahi que REX traiga `bmo_sup_es_raton` en vez de dejar al llamante
acordarse de una mascara.

### ★ Y una tecla y un clic NO se reparten igual

```text
   una TECLA  va a quien tiene el FOCO
   un CLIC    va a DONDE SE PULSO
```

Preguntarle al foco por un clic seria mandarselo a una ventana distinta de la
que el dedo estaba tocando. Por eso `keys::app::raton` no mira el foco y
`keys::app::reenviar` si.

★ Y el recorte sale gratis: `Table::golpe` contesta `None` fuera del contenido,
que es **la misma funcion que recorta los pixeles**. Un clic en la barra de
titulo no llega a la app sin que haga falta una segunda comprobacion -- y por
eso no puede haber un borde donde se ve una cosa y se pulsa otra.

⚠ **Hoy solo viaja el clic (el boton BAJANDO).** Soltar no se publica, asi que
dentro de una app no se puede arrastrar. Esta dicho en la cabecera de REX y aqui:
media promesa contada entera es una limitacion; contada a medias es un fallo.

### El cliente: las casillas del menu de `ray.bex` se pulsan

Y la geometria del menu paso a vivir en cuatro funciones --`menu_px`,
`menu_py`, `menu_fila_y`, `menu_seg_x`-- que usan **el pintado y el golpe**. No
es limpieza: si las dos cuentas no dan lo mismo hay un borde donde se ve una
casilla y se pulsa la de al lado, y eso no da error, da un numero equivocado.
Es la misma decision que el DIRECTOR tomo al recortar el golpe con la misma
funcion que recorta los pixeles.

## Lo que esto deja abierto, dicho por delante

1. **Soltar el boton no viaja**, asi que no hay arrastre dentro de una app. Y el
   MOVIMIENTO del puntero tampoco: un anillo de eventos es la forma equivocada
   de contar una posicion que cambia sesenta veces por segundo --se llenaria y
   se descartarian los nuevos, o sea que la app leeria posiciones VIEJAS--. Lo
   que quiere el raton es un CAMPO DE ESTADO en la cabecera, no una ranura.
2. **El foco se le da a cualquier app**, tenga buzon o no. Una app que solo
   ensena y se lleva el foco deja la linea de Ejecutar muda mientras este
   delante. Se sabe como se arregla --preguntarle al buzon-- y no se hizo aqui
   porque el foco significa DOS cosas a la vez (quien tiene las teclas, y quien
   esta delante para Alt+Tab) y separarlas es otra casilla.
3. **La app ve tambien los atajos del escritorio que no estan en la lista
   cerrada.** Es el precio de que las dos colas sean independientes: no hay
   forma de saber que caracter salio de que scancode.
4. ⚠ **`desktop/mouse.rs` esta en 995 lineas**, a cinco de que L6a lo rechace --
   y es un `GIGANTE` (2 funciones, media 497), o sea la especie mas cara de
   partir: el estado local tiene que volverse un struct antes de mover nada.
   **El proximo trabajo que toque el raton parte ese fichero primero.**

# PASO 3 -- Cerrar sin ser root ✅ HECHO el 2026-08-19

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

## Como quedo, y las dos cosas que salieron al escribirlo

```
   KIND_TAREA          el objeto es el TID del hijo
   se concede en       TASK_OP_EJECUTAR, a quien lanzo -- una vez
   se recupera con     TASK_OP_HIJO(tid), que solo BUSCA
   operaciones         TAREA_OP_VIVE, TAREA_OP_TID, TAREA_OP_CERRAR
```

★ **No hay comprobacion de parentesco, y es a proposito.** El permiso ES el
handle: `TASK_OP_HIJO` es un `cap::find`, igual que `CHANNEL_OPEN`, asi que a
quien no lanzo ese proceso no hay nada que darle. Anadir ademas un *"es tu
hijo?"* seria la misma regla en dos sitios, que es como se acaba con dos reglas
que no dicen lo mismo.

★ **El tid como objeto sale gratis**: `next_tid` solo sube, asi que un handle
viejo nunca acaba nombrando a otro proceso. Es la propiedad que `loan.rs` tuvo
que GANARSE revocando el handle al soltar --alli la direccion si se reutiliza--
y que aqui viene con el numero.

[!] **Y el fallo latente que destapo**, porque este es el primer caso en que
muere un proceso que NO es el que llama: `cap::revoke_all` le pasaba a
`loan::process_died` el **CR3 activo**. Con `TAREA_OP_CERRAR` el que llama es el
padre, asi que `undo` habria desmapeado paginas **del que cierra** en vez de las
del cerrado: compila, y se lleva por delante al DIRECTOR. Ahora se busca el
espacio del que muere con `cr3_de_pid`.

---

# PASO 4 -- Prioridad por FOCO ✅ HECHO el 2026-08-19

> **La prioridad no es un atributo del proceso. Es una consecuencia de a donde
> mira el usuario.**

En Linux hay `nice()`, en Windows `SetPriorityClass`: un numero que el programa
**se pide a si mismo**, y por eso todo el mundo se pone alto y el numero deja de
significar nada.

Aqui no hace falta esa API. El DIRECTOR ya sabe quien tiene el foco
(`bmo_input::foco`), y el foco lo decide el usuario apuntando. **Una app no
puede subirse la prioridad porque no hay donde pedirla: la gana estando
delante.** Una operacion mas hacia el planificador, no un sistema nuevo.

## ★★ Y AL ESCRIBIRLO: NO ES PRIORIDAD, ES QUANTUM

`TAREA_OP_DELANTE`, sobre el handle del hijo. Y **no toca `priority`**, por un
motivo que solo se ve mirando `choose_next`:

```rust
if task.state == Ready && (best.is_none() || task.priority > best_priority)
```

Es prioridad **estricta y sin envejecimiento**. Una tarea de prioridad 1 le gana
el turno a las de 0 siempre que este lista, y ceder no ayuda porque quien cede
sigue listo. O sea que subirle la prioridad a la app de delante **le ganaria el
turno al DIRECTOR**, que esta en 0 -- y sus pixeles dejarian de componerse.
**La ventana de delante seria la primera en dejar de refrescarse**: exactamente
lo contrario de lo que la regla busca.

El quantum no tiene ese modo de fallo. La rueda sigue dando la vuelta entera y
nadie se queda fuera; lo unico que cambia es cuanto dura cada parada.

```
   la PRIORIDAD es un ORDEN     y un orden estricto EXCLUYE
   el QUANTUM   es un REPARTO   y un reparto no deja a nadie fuera
```

★ **El foco no decide QUIEN corre. Decide CUANTO.** 4 ticks los demas, 8 la de
delante, y delante hay uno: ponerselo a una se lo quita a la anterior en la
misma pasada. Sin eso, cada cambio de foco dejaria una app mas con turno largo
y en diez minutos lo tendrian todas -- que es como `nice()` dejo de significar
nada en otros sistemas, solo que por descuido en vez de por pedirlo.

★★ **Y el DIRECTOR no puede favorecerse a si mismo**, y no por prudencia: la
operacion va sobre el handle de un HIJO, y a el lo lanzo el kernel. No hay
handle suyo en manos de nadie, asi que no hay por donde pedirlo. La propiedad
que la hace no-`root` es estructural, no una promesa.

## ★ Y EL FOCO YA SABE NOMBRAR UNA APP -- mismo dia, y era el vocabulario

`Ventana::App(u8)`, el hueco de la mesa de superficies. Lo que rompio para
entrar es lo que lo hace valer: el enum era **C-like** y `id()` era
`self as u8`, o sea **seis ventanas decididas al compilar**. Una app no cabia
ahi, y por eso 2c.3 daba el foco por resuelto: la POLITICA si estaba en
`bmo_input::foco` --con sus veinte pruebas-- y lo que faltaba era el
vocabulario para nombrarla.

Ahora `id()` es un `match`, y un id puede salir de un dato en vez de una
constante. El compilador pidio la rama `App` en los **cuatro** sitios que
deciden algo por ventana, que es exactamente lo que `nombre()` prometia: *una
ventana nueva no arranca hasta que tiene nombre*.

```
   nace la caja      table.collect devuelve EL HUECO  ->  focus.open(App(i))
   se toca           focus.clic_en(App(i))
   se cierra         focus.close(App(i))
   MAX_VENTANAS      8 -> 10   seis fijas + las cuatro cajas
```

★★ **Y el turno se aplica en UN solo sitio**: una vez por vuelta, cuando el
foco CAMBIA. Ponerlo en el clic dejaba fuera a Alt+Tab --por ahi no pasa-- y
tener la misma regla en dos sitios es como se acaba con dos que no dicen lo
mismo. De paso evita sesenta cruces de puerta por segundo repitiendo algo que
ya era verdad.

[!] `Windows::abierta` contesta **`true`** para una app y no consulta nada: un
`App` solo entra en la lista del foco cuando nace su caja y sale cuando se
cierra, asi que la verdad la garantiza quien llama. Contestar `false` seria
peor que no contestar -- `top_now` caeria a `Run` y repintaria la terminal cada
vez que el foco estuviera en una app.

---

# PASO 5 -- El rename ✅ HECHO el 2026-08-19

Los cuatro estaban hechos, asi que el nombre ya estaba **cobrado**: *un nombre
describe algo hecho, no una intencion.*

```
   services/director/        ->  services/director/
   bmo-service-director      ->  bmo-service-director
   bin `compositor`     ->  bin `director`
   sys/gui.bex          ->  sys/d.bex
```

## ★★ Y EL FICHERO NO SE LLAMA `director.bex`, QUE ES LO INTERESANTE

`director` son **ocho caracteres exactos**: cabe en 8.3. O sea que el limite del
sistema de ficheros --el que decidio `gui` en su dia-- aqui no decide nada.

Lo que decide es del dueno: *"es para escritura en caso de que mi Ring 3 se caiga
y tenga que escribir"*. **Con el escritorio muerto esto se teclea a mano** desde
el shell de Ring 0, y ahi lo que cuenta son las letras.

★ Una letra no es una abreviatura: es una firma. Es la regla por la que `cc`,
`ld` y `sh` se llaman asi -- **lo que mas se usa lleva el nombre mas corto**. Y
no pierde nada, porque el NOMBRE y el ASA son dos cosas distintas y este arbol
ya las separaba: el crate, el binario y el fichero nunca se llamaron igual.

```
   el NOMBRE   DIRECTOR   lo dicen el arranque, CABINA y las tres leyes
   el ASA      d.bex      lo que se escribe cuando no queda escritorio
```

⚠ Y el 8.3 sigue mandando en todo lo demas: el driver FAT32 del kernel se niega
a recortar nombres, porque un nombre recortado abre otro archivo -- y en un
cargador de programas eso es ejecutar otro binario.


---

# ★★ EL RITMO DEL ESCRITORIO ERA UNA SUPOSICION -- 2026-08-23

> Lo trajo el dueno como una queja de uso: *"el DIRECTOR algo falla en darle
> autoridad en pantalla para que DOOM tome"*. La autoridad estaba bien. Lo que
> fallaba era **que el icono no llegaba a lanzar nada.**

## Lo que se encontro, y no era uno sino tres

El 19-08 (`cc5e6922`) un clic paso a SENALAR y el lanzamiento a **doble** clic.
El gesto se media contando `Tick::frames` contra una constante:

```text
   pub const DOBLE_CLIC: u32 = 24;   // "a los ~60 por segundo, unos 400 ms"
```

Ese comentario tenia dos afirmaciones y **ninguna se sostiene**:

1. **El contador no cuenta fotogramas: cuenta VUELTAS DEL BUCLE.** Sube una vez
   por pasada y el bucle no tiene freno --acaba en `yield_screen()` y vuelve--,
   asi que una vuelta muda son unas pocas puertas. Nadie lo habia contado nunca.
2. **Si hay reloj fino en Ring 3**, y se estaba usando trescientas lineas mas
   abajo en el mismo fichero: `lend_screen` construye su presupuesto de treinta
   segundos con `bmo::ciclos()` y `INFO_TSC_HZ`. El motivo escrito para contar
   fotogramas --*"en Ring 3 no hay reloj mas fino que el segundo"*-- era falso
   cuando se escribio.

★★★ **Y el aviso estaba escrito, en el sitio correcto, un mes antes.** El campo
`clic_frame` de la ventana de Datos lo decia con todas las letras:

> *"si el bucle del escritorio corre mas rapido, la ventana del doble clic se
> acorta sola. Es el precio de no tener un contador fino, y se paga
> sabiendolo."*

El riesgo estaba bien visto. Lo que estaba mal era la premisa de la que colgaba,
y **un riesgo aceptado sobre una premisa falsa no es una decision: es un fallo
con documentacion**.

## Los TRES sitios, porque la suposicion era la misma

```text
   doble clic de los iconos      scene/launcher.rs      -> no lanzaba
   doble clic de ESTRATOS        scene/data/mod.rs      -> no abria
   refresco de F7 / F8           desktop/paint.rs       -> `frames % 15`, y los
                                                          vatios son DIFERENCIAS
                                                          entre dos lecturas
   la luz del bus USB            scene/testigo.rs       -> distancia de 15 vueltas
```

Las dos ultimas no rompen nada visible: **refrescan de mas**. Los numeros del CPU
en F7 son diferencias entre dos lecturas, y con una ventana corta un vatio
tiembla en vez de asentarse -- que es exactamente lo que el comentario de
`paint.rs` temia y daba por evitado.

## Como quedo

```text
   scene/double_click.rs   NUEVO.  El gesto, en CICLOS y en un solo sitio.
                           400 ms, convertidos con INFO_TSC_HZ al usarlos.
   Tick::pulse()           cuenta la vuelta, levanta el flanco del cuarto de
                           segundo y CIERRA UN SEGUNDO: `loops_per_second`.
   Tick::frames            pasa a llamarse `loops`.  El nombre era la mentira.
   F7                      fila `escritorio`: N vueltas/s, del bucle, no
                           fotogramas.  La cifra deja de ser una suposicion.
```

★ **Y el fallback esta elegido en los dos sentidos, no por comodidad:** sin
`INFO_TSC_HZ` la ventana del gesto se abre entera --el segundo clic sobre lo
mismo abre, tarde lo que tarde-- y el espaciado de los refrescos vale `0`, o sea
*"mira siempre"*. De las dos formas de equivocarse sin reloj se coge la barata:
un raton sin reloj todavia sabe distinguir dos de uno, y una luz que no vuelve a
mirar miente sobre el bus.

```text
   aprobado:  doble clic en el icono de DOOM y arranca; doble clic en la
              rejilla de ESTRATOS y entra; y F7 dice cuantas vueltas da esto
              de verdad.  Ver `../metal/PRUEBA_EN_METAL_0823.md`.
```

⚠ **Nada de esto lo ha visto un CPU.** Compila, enlaza a `d.bex` (544.088 B) y
el banco del anfitrion sigue en 1.304 filas verdes -- pero el gesto es del raton
de una persona, y eso no se prueba en un emulador.
