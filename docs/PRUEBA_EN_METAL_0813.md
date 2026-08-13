# PRUEBA EN METAL -- el arranque del 2026-08-13

> ## ✅ EL ARRANQUE DE LAS 09:38 YA CONTESTO -- SEGUNDA VUELTA ABAJO
>
> **1. DOOM pasa `W_Init: Init WADfiles.` y abre su WAD** (`arch: archivo
> REFLEJADO para leer =4.0 MiB` dos veces). El arreglo de `&c->defaults[i]`
> funciono en metal.
>
> **2. El grafo con curvas Bezier salio**, con sus puntas de flecha. Escalones
> 0-2 verificados.
>
> **3. `caja.bex` y `cobol/1/hola.bex` corren igual** -- la regresion del codegen
> queda descartada.
>
> [!] **Y aparecieron dos cosas nuevas, las dos ya arregladas:**
>
> - `saving config in apps/doom1.wad` -- DOOM iba a escribir su configuracion
>   **encima del WAD** al salir. Causa: **`!(-6)` valia -256** en BMO C, asi que
>   `if (!strcasecmp(...))` acertaba con cualquier resultado negativo. El WAD se
>   comprobo y esta intacto (4.196.020 B).
> - **`ray.bex` da `#PF` escribiendo**: a la franja de pared le faltaban dos de
>   los cuatro topes.
>
> **Lo que sigue abajo es la primera vuelta, y se conserva.** Lo que hay que
> hacer AHORA esta en la seccion final.

Tres commits, y **ninguno ha visto un CPU**. La guia anterior queda en
`PRUEBA_EN_METAL.md`; de ahi siguen abiertas las seis preguntas de su segunda
vuelta, que **no se repiten aqui** -- se pueden contestar en el mismo arranque.

> Lo de hoy son tres preguntas, y una de ellas se contesta con una sola linea de
> texto en pantalla.

```powershell
Ultra_kernel_x86-64\build.ps1 -Flash -Drive A -Data A
```

`-BuildOnly` ya paso entero: ASCII limpio, contrato de syscalls con 49
operaciones y 42 campos de INFO, y los tres binarios enlazados.

---

# 1 -- DOOM: LA LINEA QUE LO CAMBIA TODO

`run apps/doom.bex` **desde el escritorio** (el icono, o `Ejecutar`), no desde el
shell de Ring 0. Ver el punto 4 sobre por que.

Lo ultimo que se supo: DOOM moria imprimiendo

```text
   Unknown configuration variable: 'use_joystick'
```

que **no era un aviso sino la causa de muerte** -- `I_Error` imprime y llama a
`exit`. La causa estaba en el compilador: `&c->defaults[i]` valia CERO.

## Lo que tiene que salir ahora

```text
   M_LoadDefaults: Load system defaults.
   W_Init: Init WADfiles.            <- ESTA. Es toda la prueba.
```

**Si aparece `W_Init`, el arreglo del codegen funciono en metal**: DOOM paso la
tabla de configuracion, encontro su WAD y esta abriendolo. Lo que falle DESPUES
es territorio nuevo -- y se sabra, porque ya no hay muertes mudas.

| Sintoma | Que significa |
|---|---|
| sale `W_Init` y sigue | **el arreglo funciono.** Apuntar donde muere ahora |
| vuelve `use_joystick` | el `.bex` desplegado es el viejo: el flash no llego |
| muere antes, en otra linea | otro defecto delante. La linea nueva ES el dato |
| `IWAD file ... not found` | ahora si es el WAD. Estaba en `A:\apps\` con 4.196.020 B |

★ **Y si arranca**: `A:\apps\doom1.wad` esta en el disco y la partida deberia
empezar. Eso ya no es una prueba de sistema, es jugar.

---

# 2 -- EL GRAFO DE ESTRATOS, CON CURVAS DE VERDAD

`F12` -> `TAB` -> pestana `nodos`.

Antes: una espina vertical con codos, todo rectangulos de un pixel. Ahora **una
curva Bezier por hijo**, del punto de salida del padre a la entrada de cada caja,
con punta de flecha.

## Que mirar, y es todo a ojo

1. **Que las curvas se vean curvas y no poligonales.** El numero de tramos se
   estima del tamano; si se ven las esquinas del troceado, el sospechoso es
   `tramos()` en `dibujo/curva.rs` y se sube el divisor.
2. **Que la punta de flecha toque la caja** y no se quede a un pixel ni la pise.
3. **Que las curvas no se crucen con las cajas.** Viven en el canal de 44 px
   entre las dos columnas; si alguna pasa por encima de un nombre, el tirante es
   demasiado largo.
4. ★ **ENCOGER LA VENTANA por la esquina.** Es la prueba del recorte, que hasta
   hoy no existia en esta ventana: **ni un pixel de arista puede salirse del
   marco**. Con codos no podia pasar por construccion; con curvas si.
5. Y que **arrastrar la ventana no deje rastro** de curvas viejas.

Vuelta atras: `git revert 82bb94ea`.

[!] `gui.bex` paso de 319.528 a **337.192 B**. Si el escritorio no arranca, es
esto antes que nada.

---

# 3 -- LA REGRESION QUE HAY QUE DESCARTAR: TODOS LOS `.bex` DE C

El commit del codegen (`1a48cbd2`) **toca todos los programas de C**, no solo
DOOM. Tres ordenes y se descarta entero:

```text
   run c/caja.bex        sus cuatro lineas y sus dos recursos
   run c/ray.bex         el laberinto, con paredes
   run cobol/1/hola.bex  que COBOL no se entero de nada
```

Si alguno deja de arrancar o pinta basura donde antes pintaba bien:
`git revert 1a48cbd2`.

★ Lo que **no** puede pasar es que un programa compile y haga otra cosa en
silencio: el brazo que rellenaba de ceros ahora es un error de compilacion. Si el
build hubiera encontrado un caso sin cubrir, `-BuildOnly` habria parado -- y no
paro, con los 12 ejemplos y las 56.465 lineas de DOOM delante.

---

# 4 -- ESTRATOS: `generacion 2`, Y LA PARTE BONITA ES DESPUES DE REINICIAR

En la caja de `Ejecutar`:

```text
   estratos sellar
```

Son **dos palabras a proposito**: `sellar` a secas se teclea sin querer, y esto
**escribe en el disco**.

Lo que hace: una transaccion **sin datos**. No reserva un bloque ni toca un
objeto -- commitea apuntando al mismo estrato que ya habia, y lo unico que cambia
es el numero de generacion, escrito en **la copia del superbloque que NO manda**.

★ Por eso es la primera: recorre el camino ENTERO --reservar, cerrar, `FLUSH
CACHE`, barrera, commit, superbloque alterno-- y **no puede perder un dato aunque
salga mal**. Si falla antes del commit, el volumen es el de antes; si falla
escribiendo el superbloque nuevo, se estropea la copia que no manda y el volumen
monta igual.

```text
   COMMIT. generacion 2
```

Y luego `F12` tiene que decir `generacion 2`.

★★ **LA QUE DE VERDAD PRUEBA ALGO: REINICIAR Y VOLVER A MIRAR.** Si sigue
diciendo `2`, llego al plato y no se quedo en la cache del SSD. **Eso es lo unico
que separa una barrera que funciona de una que se cree.**

Si sale `el sellado NO se hizo`, el motivo esta en F11 con nombre:
`SinVolumen`, `SinBarrera` (el `FLUSH CACHE` fallo, y entonces NO se commiteo) o
`Rechazada`.

---

# QUE TRAER DE VUELTA

1. **`A:\datos\salida.txt`** -- se llena solo con lo que se lanza desde
   `Ejecutar`, y `guarda` vuelca el historial entero. Vale mas que cualquier
   foto. **Y por eso DOOM va lanzado desde el escritorio**: desde el shell de
   Ring 0 su salida no la recoge nadie.
2. **La foto del grafo con curvas**, que es lo unico de esta tanda que no se
   puede contar con texto.
3. La linea de DOOM donde muera, si muere.
4. `generacion` antes y despues del reinicio.

---

# LOS TRES COMMITS

| commit | que toca | si algo falla |
|---|---|---|
| `1a48cbd2` | **el codegen de C**: todos los `.bex` | un programa de C deja de andar |
| `073d10f8` | solo documentos | nada |
| `82bb94ea` | **el pintado del grafo** + `userland` | el escritorio no arranca |

---

# SEGUNDA VUELTA -- lo que hay que mirar AHORA

Un commit mas (`6ae09699`), y **toca el codegen otra vez**: hay que reflashear.

## 1 -- DOOM, y las tres lineas que tienen que DESAPARECER

`run apps/doom.bex` desde el escritorio. Lo que ya NO puede salir:

```text
   Development mode ON.                 <- nadie paso -devparm
   turbo scale: 200%                    <- nadie paso -turbo
   saving config in apps/doom1.wad      <- la peligrosa
```

Las tres eran el mismo bug: `!(-6)` valia -256, o sea que
`if (!strcasecmp("-config", "-iwad"))` acertaba. Si alguna vuelve a salir, el
arreglo no llego.

★★ **Y lo que tiene que salir en su lugar**:

```text
   saving config in ./default.cfg       (o similar, pero NO el .wad)
```

Y detras de `W_Init`, territorio nuevo: `R_Init`, `P_Init`, `S_Init`,
`D_CheckNetGame`... **cada linea que salga es un paso que nunca se habia dado.**

[!] Si DOOM llega a pintar, la pantalla es suya entera y se vuelve con
`Ctrl+Alt+Esc`.

## 2 -- `ray.bex`, que ahora tiene sus cuatro topes

```text
   run c/ray.bex
```

Tiene que **dibujar el laberinto y no morir**: pasillo con paredes claras a los
lados, `W A S D` mueven, `Q E` andan de lado, `ESC` sale.

Si vuelve a dar `#PF`, `datos/fallos.txt` dice el `rip` y la direccion -- y con
los topes puestos, un fallo ahi ya no puede ser la franja de pared.

## 3 -- Lo de la primera vuelta que sigue pendiente

`estratos sellar` -> `generacion 2`, **y que siga en 2 despues de reiniciar**.
Es lo unico que separa una barrera que funciona de una que se cree.

## Que traer

1. `A:\datos\salida.txt` -- vale mas que las fotos.
2. `A:\datos\fallos.txt` si algo revienta.
3. La linea mas lejana que alcance DOOM.

---

# TERCERA VUELTA -- lo del arranque de las 10:55

## Ya contestado, y no se repite

| | |
|---|---|
| Las tres lineas de mas de DOOM | **DESAPARECIERON**. El `!` estaba arreglado |
| El WAD | intacto, y ya no lo nombra la configuracion |
| `carpetas`, la pestana nueva | OK -- columnas, seleccion y `S sella` en el pie |
| Las curvas del grafo | OK, con sus puntas de flecha |
| **ESTRATOS ESCRIBIO** | `SELLADO. generacion 3` |

## 1 -- La que queda de ayer y no cuesta codigo: REINICIAR

`F12` -> `numeros`. Si sigue diciendo **generacion 3**, la escritura llego al
plato y no se quedo en la cache del SSD. **Es lo unico que separa una barrera que
funciona de una que se cree**, y es un arranque, no una sesion de trabajo.

## 2 -- DOOM, otra vez, con `p++` arreglado

Murio entre `M_LoadDefaults` y `saving config in %s`, y entre esos dos prints hay
UNA llamada: `M_StringJoin(configdir, "default.cfg", NULL)` -- variadica, con un
bucle de `va_arg`. Y `va_arg` **es** un `*p++`, que avanzaba un byte.

Lo que tiene que salir ahora:

```text
   saving config in ./default.cfg        <- y NO el .wad
   W_Init: Init WADfiles.
   adding apps/doom1.wad
```

Y detras, territorio nunca pisado: `R_Init`, `P_Init`, `S_Init`,
`D_CheckNetGame`. **Cada linea que salga es un paso que nunca se habia dado.**

[!] Si vuelve a morir, `datos/fallos.txt` trae `rip` y direccion. El `rip` de la
vuelta anterior fue `0x4009D5F5`; si el nuevo es OTRO, es otro sitio y eso ya es
informacion.

## 3 -- La regresion, que ahora es mas ancha

`p++` lo usan **todos** los programas de C, no solo DOOM:

```text
   run c/caja.bex        sus cuatro lineas y sus dos recursos
   run c/ray.bex         el laberinto, y ahora sale con ESC o con la pantalla quitada
   run cobol/1/hola.bex  que COBOL no se entero
```

★ Y la que de verdad vigila: **un `int` tiene que seguir contando de uno**. Si
`i++` avanzara cuatro, todos los bucles del sistema irian de cuatro en cuatro.
Hay una prueba que lo fija, pero en metal se ve en que `ray.bex` ande normal.

Vuelta atras: `git revert 74657604`.

---

# CUARTA VUELTA -- la disposicion de los agregados, y el `memcpy`

> Esta tanda no sale de ver morir a DOOM: sale de **enumerar** un eje que nadie
> habia barrido. Por eso llega antes de que la maquina lo pida.

## 1 -- DOOM: donde tiene que morir ahora, si muere

El defecto: **el alineado de un miembro se deducia de su TAMANO**, y eso es
falso para todo lo que no sea un escalar -- un array se alinea como su
ELEMENTO. `char name[8]` mide ocho bytes igual que un `long` y se alinea a uno.

DOOM no parsea sus datos: **castea un struct encima de los bytes crudos del
WAD**. Asi que la disposicion no era una convencion interna, era una respuesta
que el formato del fichero ya tenia escrita desde 1993 -- y no coincidia:

| struct | lo que hacia BMO C | lo que dice el disco |
|---|---|---|
| `maptexture_t.patches` | offset 24 | **22** |
| `maplinedef_t` entero | 16 bytes | **14** |
| `mapsidedef_t` entero | 40 bytes | **30** |
| `mapnode_t` entero | 32 bytes | **28** |

Los dos que son el TAMANO son los peores: `p_setup.c` recorre el lump como un
array, o sea que **el primer registro del nivel sale bien y todos los demas
corridos**. Ese sintoma no se parece a un fallo de disposicion; se parece a un
nivel roto.

**Lo que hay que mirar, en orden:**

```text
   R_Init: Init DOOM refresh daemon -     <- pasa de aqui?
   P_Init: Init Playloop state.
   S_Init: Setting up sound.
   D_CheckNetGame: Checking network game status.
   HU_Init / ST_Init
```

| Sintoma | Que significa |
|---|---|
| llega a `D_DoomLoop` y **arranca solo el DEMO1** | esto ya es jugar |
| muere en `R_Init` | quedan mas casillas de disposicion, o texturas |
| muere en `P_Init` | idem, pero en las estructuras de nivel |
| `R_InitTextures: Missing patch in texture` | `patches` sigue mal colocado |

[!] **S_Init no puede fallar y conviene saberlo**: `FEATURE_SOUND` no esta
definido en este arbol, asi que `sound_modules[]` es `{NULL}` e `I_InitSound`
vuelve sin hacer nada. **DOOM va a jugarse en silencio.** Si muere ahi, el
sospechoso no es el sonido.

## 2 -- `memcpy` y `memset` son ahora UNA instruccion

`rep movsb` y `rep stosb`, con su `cld` delante. Antes eran bucles de seis
instrucciones **por byte**.

⚠ **Esto toca a TODOS los programas de C y de C++**, porque `memcpy` lo emite
el compilador. Si algo que copiaba deja de copiar, es esto.
Vuelta atras: el commit de esta tanda.

**LA PRUEBA, y son las de siempre**: `run c/caja.bex`, `run c/ray.bex`,
`run cobol/1/hola.bex`. Si el laberinto sale igual, la copia funciona -- el
raycaster copia filas enteras cada fotograma.

** Y la que de verdad importa, porque es la que no se puede probar aqui:
**DOOM tiene que ir mas fluido**. `DG_DrawFrame` mueve 1.024.000 bytes por
fotograma; con el bucle viejo eran ~6,1 millones de instrucciones de puro blit.

[!] **No es donde estaba el limite** y conviene no esperar un milagro: ~1-2 ms
de los 28,5 que tiene un fotograma a 35 Hz. DOOM daba 35 fps en un 486 a 66 MHz,
asi que en este Ryzen la velocidad no deberia ser el problema. Si DOOM va lento
**despues** de esto, el sospechoso es el renderizador compilado por BMO C, y
entonces hace falta el numero: `DG_GetTicksMs` ya existe.

## 3 -- Lo que ya NO hace falta traer de vuelta

Nada de esta tanda pide una foto. Las 12 casillas de la disposicion y las 28 del
lenguaje se contestan solas con `cargo test -p bmo-c-front sonda_`, y el "antes"
esta medido: con la regla vieja daban **nueve rotas de doce**.

Lo unico que el Ryzen puede contestar y el anfitrion no es **hasta donde llega
DOOM**. Una linea.

---

# QUINTA VUELTA -- tres ejes mas, y uno estaba roto

Se siguio enumerando en vez de esperar al arranque. Tres sondas nuevas, **39
casillas**, y el reparto es el dato:

| eje | casillas | resultado |
|---|---|---|
| **el ANCHO** -- estrechar/ensanchar (`SHORT(x)` de DOOM) | 16 | limpio a la primera |
| **las TABLAS** -- los datos que el `.bex` trae puestos | 11 | limpio a la primera |
| **el SIGNO** -- las operaciones que miran el bit alto | 16 | ** CUATRO ROTAS ** |

Que dos salgan limpios tambien vale: cuando `R_Init` falle, **no hay que
empezar por ahi**.

## Lo que estaba roto: las cuatro operaciones sin signo de 64 bits

```text
   (unsigned long)0x8000000000000000 >> 60   daba 18446744073709551608
   ...                               / 2     daba un negativo enorme
   ...                               % 10    idem
   ...                               > 1     daba 0
```

`>>` emitia `sar` donde tocaba `shr`, `/` y `%` emitian `cqo`+`idiv` donde
tocaba `xor rdx,rdx`+`div`, y `<`/`>` emitian `setl`/`setg` donde tocaba
`setb`/`seta`.

[!] **En 32 bits acertaba por casualidad**, y por eso llevaba ahi desde el
primer dia: el codegen calcula en `rax`, y un `unsigned int` llega extendido
con ceros -- el bit 63 vale 0 y `sar` da lo mismo que `shr`. Solo un
`unsigned long` lo destapa.

** Y el arm de `Shr` lo confesaba por escrito: *"un tipo sin signo querria
`shr`; hoy el codegen no arrastra esa distincion hasta aqui"*. Era falso -- la
distincion llegaba, faltaba preguntar. `expr_is_unsigned` esta calcada de
`expr_is_float`, que llevaba al lado todo el tiempo.

## Que mirar en metal

⚠ **Toca la aritmetica de TODOS los programas de C**: cambia que instruccion se
emite para `>>`, `/`, `%` y las cuatro comparaciones de orden. Si una cuenta
que salia bien empieza a salir mal, es esto.

```text
   run c/ray.bex         el laberinto (usa 16.16 con signo por todas partes)
   run c/caja.bex        sus cuatro lineas
   run cobol/1/hola.bex  que COBOL no se entero
```

★ Y **DOOM es la prueba de verdad**: `angle_t` es `unsigned int` y el
renderizador entero vive en aritmetica sin signo. Si antes hubiera llegado a
pintar, los angulos por encima de 180 grados habrian salido mal.

413 verdes en la suite de C, 1.126 en el workspace, y `build.ps1 -BuildOnly`
pasa entero.
