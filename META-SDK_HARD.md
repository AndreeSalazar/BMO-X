# META-SDK HARD

> **La ley de REX** -- la libreria con la que se escribe una app de BMO-X.
>
> No la firma quien la escribe: la firma **la ley de una app**, que a su vez la
> firmo la superficie, que la firmo el silicio.
>
> Escrito el **2026-08-18**, el mismo dia que su hermana mayor, y por un motivo
> concreto: al preguntar *"que le falta a BMO-X para que entre gente de fuera"*
> aparecio que **el SDK ya existia** --2.360 lineas en diez cabeceras-- y que
> no tenia ni nombre ni indice. Vivia dentro del directorio de tablas del
> ensamblador. Ahora se llama **REX**.
>
> Tercera de [`META-KERNEL_HARD.md`](META-KERNEL_HARD.md) y
> [`META-APP_HARD.md`](META-APP_HARD.md), con la misma forma: aqui una regla
> solo existe si al lado tiene **de donde sale** y **que se rompe** si no se
> cumple.

---

## 0. LA CADENA, AHORA CON TRES ESLABONES

```
   el SILICIO         firma la ley del kernel      META-KERNEL_HARD.md
   la SUPERFICIE      firma la ley de una app      META-APP_HARD.md
   la LEY DE UNA APP  firma la ley de REX          este documento
```

★ **REX no tiene ley propia, y eso es la propiedad y no la carencia.** Existe
para una sola cosa: que cumplir las siete reglas de la Meta-App no cueste
escribirlas siete veces, una por app. El dia que REX exija algo que la Meta-App
no exige, el que esta mal es REX.

---

## 1. QUE ES REX, EN UNA FRASE

★★ **REX abstrae en tiempo de COMPILACION, nunca en tiempo de ejecucion.**

Eso no es una preferencia de estilo: es lo unico que deja la raiz congelada. La
comodidad sube a REX, REX baja al `.bex`, y **la app sigue dependiendo de dos
puertas y de nada mas**.

Ya estaba escrito, en la primera linea de
`toolchain/forge/sem-asm/tables/bmo/bmo.h`:

> *"En Linux o en Windows, `#include <unistd.h>` promete un `libc.so` que el
> cargador resolvera mas tarde. Aqui no hay cargador que resuelva nada [...]
> Asi que la cabecera **trae el cuerpo**, y lo que hay dentro del cuerpo baja a
> la instruccion en la misma linea."*

Ese comentario era doctrina escondida en un fichero. Esto lo sube a ley.

### 1.1 El desahogo de la raiz -- por que la superficie aguanta en dos puertas

Un sistema sin REX tiene una sola respuesta para *"esto es incomodo"*: una
operacion nueva. Asi es como una superficie de 2 llega a 350 sin que nadie tome
jamas una mala decision -- se toman trescientas cincuenta decisiones razonables.

Con REX hay **dos respuestas distintas**, y elegir bien entre ellas es la
disciplina entera:

```
   ahorra TRABAJO      ->  es una cabecera de REX      la superficie no se toca
   concede AUTORIDAD   ->  es una operacion nueva      y hay que justificarla
```

*De donde sale*: el tercer syscall que se fue. `CHANNEL_KICK` se retiro el
2026-08-10 --*"resolvia un handle y avisaba a su consumidor, o sea una OPERACION
con numero de syscall propio"*-- y su numero quedo **reservado y sin reciclar**,
para que un binario viejo falle diciendolo. La puerta se estrecho mientras el
sistema crecia.

*Y el numero de hoy, medido y no supuesto*, con el mismo `grep` sobre
`platform/abi/bmo-abi/src/syscalls/surface/` en tres fechas del arbol:

```
   2026-08-11   cf878698    69 operaciones
   2026-08-14   324f7654    73
   2026-08-18              88
   2026-08-19   HEAD        92      <- 44 TASK, 9 ARCH, 6 INPUT, 4 AUDIO, 3 TAREA...
```

⚠ **Los documentos decian 39 y 40.** `docs/identidad/QUE_DESBLOQUEA.md` y
`docs/maestro/AUTOCURACION_MAESTRO.md` llevaban una cifra vieja, y el segundo la
usaba para prometer que *"la superficie cabe en la cabeza"*. Sigue cabiendo --88
operaciones detras de dos puertas se auditan en una tarde-- pero la promesa
tiene que decir el numero de hoy. **Dos puertas es la forma; 88 es el tamano.**
Confundirlas es lo que hace que una ley suene mejor de lo que es.

---

## 2. LAS DOS PRUEBAS -- las que separan una libreria de un framework

[`META-APP_HARD.md`](META-APP_HARD.md), apartado 6, prohibe **un framework** y
**un runtime obligatorio**. REX podria ser las dos cosas sin querer. Estas son
las dos preguntas que lo impiden, y se contestan antes de escribir codigo:

### La PRUEBA DEL FRAMEWORK

> *Si REX desapareciera manana, la app se podria reescribir contra la superficie
> **sin cambiar el sistema**?*

- **Si** -> es una **libreria**: entra en el `.bex` como codigo, y el contrato
  sigue siendo la superficie.
- **No** -> es un **framework**: el contrato se mudo. Ese es el dia en que
  BMO-X hereda las deudas de otro.

### La PRUEBA DEL RUNTIME

> *Algo tiene que estar VIVO para que la app arranque?*

Si la respuesta es si, eso es un punto unico de fallo que la ley del kernel no
contempla. Una cabecera que trae el cuerpo, si. Un servicio obligatorio, no.

★ Las diez cabeceras de hoy pasan las dos limpiamente. Escribirlo aqui no es
felicitarse: es dejar el criterio puesto **antes** de que se escriba la once --
y `bloque.h`, la decima, se escribio DESPUES de esta ley y paso las dos.

---

## 3. LAS SIETE REGLAS

Cada una con de donde sale y que se rompe sin ella, igual que las de una app.

### R-REX1 -- Compila hacia dentro, no enlaza hacia fuera

Una cabecera de REX **trae el cuerpo**. No hay simbolo que alguien vaya a
rellenar despues, porque no hay enlazado dinamico y un `.bex` es una imagen
entera.

**Sin esto**: REX se convierte en un `libbmo.so`, y entonces la version de esa
libreria pasa a ser parte del contrato de toda app que exista.

### R-REX2 -- Nada de REX tiene que estar vivo

Ni un servidor, ni un demonio, ni un proceso que haya que arrancar antes.

**Sin esto**: una app no puede arrancar sola, y `R-APP6` --*muere sin llevarse a
nadie*-- deja de poder cumplirse hacia arriba.

### R-REX3 -- Comodidad es cabecera; autoridad es operacion

La regla del apartado 1.1, hecha regla. Una peticion de la forma *"seria comodo
que el sistema hiciera X"* se contesta con una cabecera **salvo que X conceda un
derecho que hoy no se puede conceder**.

**Sin esto**: la superficie crece por comodidad, que es como crecen todas.

### R-REX4 -- Una sola fuente de verdad, N enlaces

Los creadores de BMO-X escriben en cuatro lenguajes de la casa. Si REX es una
implementacion por lenguaje escrita a mano, las 88 operaciones derivan.

*De donde sale*: ya existe el guardian --
`platform/abi/bmo-abi/tests/bmo_h_cruza_de_lenguaje.rs`-- que comprueba que
`bmo.h` y el ABI de Rust dicen lo mismo. Y ya existe el precedente de la forma
correcta: los 62 intrinsecos de sem-asm viven en **una tabla TOML**, no en miles
de lineas repetidas cuatro veces.

**Sin esto**: el patron de fallo numero uno de este arbol -- dos listas de lo
mismo que derivan en silencio, exactamente lo que le paso a `ORDENES` en el
Ep. 39 de [`BITACORA.md`](BITACORA.md).

### R-REX5 -- Se EXTRAE de dos apps, no se disena para diez

Una pieza entra en REX cuando **dos apps ya la escribieron por separado**. No
antes.

*De donde sale*: [`META-APP_HARD.md`](META-APP_HARD.md) seccion 6 -- *"una API
general antes de tener clientes es coste sin comprador"*. Y el precedente
ajeno: SDL no se diseno en una pizarra, se **extrajo** de los ports de juegos
reales.

**Sin esto**: REX crece con funciones que nadie llamo nunca, y cada una hay que
mantenerla contra una superficie que se mueve.

### R-REX6 -- Tapable sin bifurcar

Un tercero tiene que poder sustituir una pieza de REX por la suya **sin editar
el repo**.

*De donde sale*: ya funciona. `toolchain/forge/bmo-mods/src/lib.rs` busca en
`$BMO_MODS` -> `mods/` -> `tables/`, *"y el primero que tenga el fichero gana
[...] para poder tapar una tabla del sistema sin editarla"*.

**Sin esto**: el que necesita cambiar una linea se lleva una bifurcacion entera,
y a partir de ese dia sus apps y las de la casa ya no son del mismo sistema.

### R-REX7 -- No esconde el numero

Una libc esconde lo que cuesta una llamada. REX no: si una operacion cruza la
puerta y eso son **969 ciclos** --dos testigos, 953 y 969, un 1,7% de
diferencia, en
[`LA_PUERTA_POR_DENTRO.md`](docs/componente/LA_PUERTA_POR_DENTRO.md)-- la
cabecera lo dice donde el que la usa lo va a leer.

**Sin esto**: alguien escribe un bucle de un millon de llamadas porque la
funcion parecia barata, y el sistema carga con la fama.

---

## 4. EL CENSO -- las diez piezas que REX ya tiene

Medido el 2026-08-18 sobre `toolchain/forge/sem-asm/tables/bmo/`:

| Pieza | Lineas | Que resuelve |
|---|---|---|
| `bmo.h` | 334 | las dos puertas, en C |
| `archivo.h` | 446 | ficheros de verdad, contra `KIND_ARCHIVO` |
| `entrada.h` | 335 | teclado y raton |
| `monton.h` | 284 | `malloc` sobre UN bloque del kernel |
| `musica.h` | 255 | notas, figuras y compas |
| `paquete.h` | 245 | los datos que viajan dentro del propio `.bex` |
| `superficie.h` | 194 | dibujar en TU memoria y ofrecerla |
| `scroll.h` | 126 | una ventana que se mueve sobre un historial |
| `sonido.h` | 103 | el sonido |
| `bloque.h` | 38 | que bloque del kernel es el del monton |
| | **2.360** | |

Y el enlace de Rust, aparte: `toolchain/lang/base/bmo-rt`, 922 lineas
(`crt0`, `syscall`, `heap`, `string`, `fmt`, `ffi`, `init`).

### 4.1 La coincidencia que vale como prueba

Puestas al lado de la descomposicion de SDL, sin haberla leido:

| SDL | REX |
|---|---|
| `SDL_video` / `SDL_Surface` | `superficie.h` |
| `SDL_events` | `entrada.h` |
| `SDL_audio` | `sonido.h` |
| `SDL_mixer` (libreria aparte) | `musica.h` |
| `SDL_RWops` | `archivo.h` |
| `SDL_malloc` (dlmalloc embarcado) | `monton.h` |
| `SDL_timer` | `INFO_TICKS` + `WAIT`, en `bmo.h` |
| `SDL_thread`, `SDL_cdrom`, `SDL_joystick`, dynapi | -- y esta bien que no |

★★ **Dos disenos que no se hablaron llegaron a las mismas siete cajas.** Eso no
es una casualidad bonita: es la mejor evidencia disponible de que el corte esta
bien hecho, porque el reparto de aqui no salio de copiar a nadie, salio de la
superficie.

---

## 5. POR QUE REX PUEDE SER UNA DECIMA PARTE DE SDL

La pregunta correcta no es *"cuanto le falta a REX para ser SDL"*. Es **por que
SDL es tan grande**:

> **SDL existe porque en Linux y en Windows una app NO PUEDE tocar el
> framebuffer.** SDL es el peaje que se paga a X11, a DirectX, a Wayland, a
> Cocoa. En BMO-X la app recibe la superficie **por prestamo** y compone sin una
> sola copia por fotograma.
>
> ★★ La capa por la que a SDL le pagan **aqui no tiene a quien cobrarle**.

De ahi sale el numero: **el tamano de SDL es el precio de la portabilidad, y
BMO-X no es portable -- es UN sistema.** Fuera los backends por plataforma,
fuera la seleccion de driver en ejecucion, fuera sus hilos, fuera su asignador,
fuera el bosque de `#ifdef`... y lo que queda es del orden de lo que ya esta
escrito aqui.

★ Lo que si conviene robarle a SDL, dicho para no tener que releerlo:
`SDL_Surface` como struct plano (es `BSUP`), `SDL_Event` como **una** union
etiquetada (*una ranura ES un mensaje*), `SDL_Init(flags)` declarando
subsistemas (es *el programa DECLARA*), `SDL_Flip` (es `R-APP4` exactamente) y
el callback de audio que **tira** en vez de empujar.

---

## 6. DONDE VIVE, Y POR QUE NO SE MUEVE

REX vive en `toolchain/forge/sem-asm/tables/bmo/`, que parece el sitio
equivocado y es el unico correcto: **`tables/` es la puerta de los terceros**
(`R-REX6`). Sacarlo a una carpeta bonita le quitaria a un creador la capacidad
de tapar una pieza sin bifurcar el repo.

★ El problema nunca fue el sitio: era que **no estaba en ningun indice**. Es el
Ep. 39 de [`BITACORA.md`](BITACORA.md) otra vez --*"una funcion que no se
anuncia no es discreta: no esta"*-- y alli el arreglo tampoco fue mover `ext`,
fue anunciarla en las dos listas. Aqui igual: REX tiene su indice al lado de los
ficheros y se anuncia desde la raiz, desde [`docs/README.md`](docs/README.md) y
desde su hermana.

---

## 7. LO QUE LE FALTA A REX HOY -- dicho para que nadie se lo prometa

| Hueco | Estado real | Que lo desbloquea |
|---|---|---|
| **el enlace de COBOL y de Ada** | REX es C (9 cabeceras) y Rust (`bmo-rt`). Los otros dos lenguajes de la casa no tienen enlace | `R-REX4`: la tabla y lo generado |
| **entrada dentro de una ventana** | `entrada.h` habla por relevo de pantalla entera | la casilla 4 de [`META-APP_HARD.md`](META-APP_HARD.md) |
| **sonido de verdad** | `sonido.h` y `musica.h` existen; debajo hay un contrato y el altavoz del PC. No hay driver HDA ni isocrono por USB | [`AUDIO_MAESTRO.md`](docs/maestro/AUDIO_MAESTRO.md) |
| **hilos** | no hay hilos de Ring 3, y `toolchain/lang/c/BRECHA.md` lo dice cuatro veces | SMP cableado, y no antes |
| **mas de un fichero por proyecto** | ★ el techo real del creador de fuera: hoy es **una sola unidad de traduccion** | compilacion separada |

★ El ultimo merece decirse claro, porque es el que de verdad frena a un tercero:
`R-APP1` --*es UN fichero*-- es el formato de **entrega**, y esta bien. Que
tambien sea el techo de **desarrollo** no esta bien, y no lo arregla ninguna
cabecera de REX.

---

## 8. LO QUE SERIA UN ERROR

- **Que REX crezca por si mismo.** Sin `R-REX5`, una libreria de sistema se
  llena de funciones que parecian utiles. Cada una hay que mantenerla contra 88
  operaciones que se mueven.
- **Que REX se vuelva obligatorio.** El dia que no se pueda escribir una app sin
  el, REX es el contrato y la superficie es decoracion.
- **Que una comodidad de REX suba a la superficie.** Es el camino por el que 2
  puertas se convierten en 350, y se recorre entero con buenas intenciones.
- **Prometer compatibilidad con SDL.** REX se le parece porque el problema es el
  mismo, no porque sea un clon. Una app de SDL no compila aqui, y decir lo
  contrario es vender una compatibilidad que no existe.

---

Ver [`EL_FUERO.md`](EL_FUERO.md) (el reparto entero: que se concede y que se
exige), [`META-KERNEL_HARD.md`](META-KERNEL_HARD.md) (la ley de la maquina),
[`META-APP_HARD.md`](META-APP_HARD.md) (la ley de una app),
[`QUE_DESBLOQUEA.md`](docs/identidad/QUE_DESBLOQUEA.md) (que desbloquea que, y
por que no es el lenguaje) y
[`LA_PUERTA_POR_DENTRO.md`](docs/componente/LA_PUERTA_POR_DENTRO.md) (lo que
cuesta cruzar).
