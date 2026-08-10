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
| El marco | `escena/marco.rs`: minimizar, maximizar, cerrar, fichas y arrastre. En metal desde el 06-08 |
| Prestamo de memoria | `loan::offer` + `loan::take` + `loan::process_died`. **Completo** |
| El contrato de superficie | `<bmo/superficie.h>`, formato `BSUP`. **Hecho** (`de3a74b9`) |

---

# PASO 1 -- `TASK_OP_MI_PADRE = 0x26`

Una superficie se le ofrece al DIRECTOR, y el programa **no tiene otra forma de
nombrarlo**. `<bmo/superficie.h>` ya lo llama; mientras no exista,
`bmo_superficie_crear` devuelve 0 y el programa se cae al camino de la pantalla
exclusiva -- el degradado correcto.

**Los cinco sitios, en orden:**

1. `ring0/task/paquete.rs` es el patron a copiar: tablas paralelas indexadas por
   ranura, `MAX_VIVOS = 16`, y `process_died` que la suelta. Un modulo hermano
   --o dos columnas mas en ese mismo-- guardando **el tid del padre**.
2. Quien lo apunta: `lanzar.rs`, junto a `paquete::recordar(pid, path)`, que ya
   corre **despues** de que la admision haya ido bien. El padre es quien invoco
   `EJECUTAR`, o sea `scheduler::current_pid()` en ese instante.
3. `scheduler::tid_de(pid)` -- el inverso de `pid_de(tid)`, que ya existe. Hace
   falta porque Ring 3 solo conoce tids: `MEM_OP_OFRECER` recibe un tid y lo
   traduce con `pid_de`.
4. `syscall.rs`: la constante `0x26` y su brazo. **[!] Y el guardian de opcodes
   de `build.ps1` lo comprueba** -- hoy dice `37 opcodes, ninguno repetido`, y
   ese numero sube a 38. Listar los opcodes ordenados ANTES de elegir: el
   2026-08-02 `MEMORIA_PEDIR` se puso en `0x12`, que ya era `REINICIAR`, y pedir
   memoria habria reiniciado la maquina.
5. `bmo-abi/src/syscalls/surface.rs` y `Ultra_userspace/userland/src/lib.rs`:
   el mismo id en los tres, que es lo que el guardian compara.

**Devuelve `0` si no hay padre** --lanzado desde el shell de Ring 0-- y eso NO
es un error: es la respuesta correcta a "quien compone para mi".

---

# PASO 2 -- El lado del DIRECTOR

1. **Tomar**: `loan::take(pid, aspace)` devuelve la VA donde quedo mapeado lo
   que alguien ofrecio. Se llama una vez por fotograma; si no hay nada, `None`.
2. **Leer la cabecera** `BSUP` (32 bytes, ver `<bmo/superficie.h>`): magic,
   ancho, alto, stride, formato, **secuencia**.
3. **Pegar solo si la secuencia cambio.** Es la regla entera:
   > La app sube `secuencia` cuando el dibujo esta entero; el DIRECTOR repinta
   > cuando ve un numero distinto del que pego la ultima vez.
   Un fotograma a medias no cambia el numero, asi que no se pinta, y el peor
   caso es ensenar el anterior un fotograma mas.
   ** NO es un cerrojo y no debe serlo: un cerrojo dejaria al compositor
   esperando a una app colgada, y entonces una app rota se lleva el escritorio.
4. **Dentro del marco** que `escena/marco.rs` ya dibuja. Los tres botones salen
   gratis.
5. **Pantalla completa = no dibujar el borde.** Sigue componiendo: Alt+Tab
   sigue, el conmutador sigue, `Ctrl+Alt+ESC` sigue. Un juego colgado se cierra
   con el teclado y no con el boton de reset.

**La prueba**: portar `ray.bex` -- dibuja en la superficie en vez de en el
framebuffer, y aparece en una caja con sus tres botones.

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
