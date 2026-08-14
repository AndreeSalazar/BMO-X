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
