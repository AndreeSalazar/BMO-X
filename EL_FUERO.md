# EL FUERO DE BMO-X

> **Lo que BMO-X le concede a quien quiera construir sobre el, y lo que le
> exige a cambio.** Un solo documento, para que nadie tenga que reunirlo.
>
> Escrito el **2026-08-19**, cuando el dueno pregunto como se llama todo esto
> junto: *"algo parecido a lo que aplican las empresas, pero basado en mi BMO-X,
> que su naturaleza es unica"*.
>
> No se llama SDK, y el apartado 0 explica por que la palabra se queda corta.

---

## 0. ★★ POR QUE NO SE LLAMA SDK

### Lo que dan las empresas

```
   Microsoft   Windows SDK, WDK      cabeceras + herramientas + documentacion de API
   Google      Android SDK, NDK      lo mismo, con un runtime obligatorio debajo
   Apple       los frameworks        lo mismo, y la capa ES el contrato
```

Los tres reparten la misma cosa: **un kit para desarrollar CONTRA una API**. Lo
que se entrega es la superficie de llamada y el modo de empleo. Lo que **no** se
entrega es por que esa superficie es asi, que esta prohibido, ni que pasa el dia
que la version siguiente rompa la anterior.

### Lo que da BMO-X, que es otra cosa

Aqui no hay una API a la que llamar: hay **una maquina que se concede con
condiciones escritas**. La frase fundacional del sistema es de
[`EL_CONTRATO_DE_CARGA.md`](docs/identidad/EL_CONTRATO_DE_CARGA.md) y no ha
cambiado:

> **El programa DECLARA, el sistema CONCEDE, el kernel solo COMPRUEBA.**

Un programa no pide permiso en ejecucion: dice de antemano lo que necesita, y
**lo que no declaro no existe para el**. No hay `root` al que escalar porque no
hay `root`. Eso no es una caracteristica de un kit: es un regimen.

### Por eso: un FUERO

Un *fuero* es la carta que una autoridad **concede** a un pueblo: la lista
escrita de lo que sus gentes pueden hacer, con lo que se les exige a cambio, y
que **no se puede exceder**. Es, palabra por palabra, lo que hace una
capability.

★★ Y la fila que ningun SDK del mundo trae: **las leyes**. Microsoft te da la
API y te deja adivinar el motivo; aqui se entrega **por que** esta prohibida
cada cosa, con su numero y con lo que ya se pago por incumplirla. Un fuero sin
sus razones seria un peaje.

---

## 1. LA CADENA ENTERA, DE UNA VEZ

```
   el SILICIO         firma la ley del kernel      META-KERNEL_HARD.md
   la SUPERFICIE      firma la ley de una app      META-APP_HARD.md
   la LEY DE UNA APP  firma la ley de REX          META-SDK_HARD.md
   ------------------------------------------------------------------
   y este documento no firma nada: REPARTE lo anterior y lo INVENTARIA
```

★ El FUERO **no es una cuarta ley**. Las leyes dicen que esta prohibido y por
que; esto dice **que hay, donde esta y por donde se empieza**. Si alguna vez
contradice a una ley, la ley gana y este fichero esta viejo.

---

## 2. LO QUE SE CONCEDE

### 2.1 La superficie -- dos puertas

```
   INVOKE(cap, operacion, a0, a1, a2)     haz esto AHORA
   WAIT(esperable, visto, timeout_ns)     despiertame CUANDO
```

Congeladas. Todo lo demas --abrir un fichero, leer el raton, reclamar la
pantalla-- es una **operacion sobre una capability**, y hoy son **93**
(medidas el 19-08 sobre `platform/abi/bmo-abi/src/syscalls/surface/`; eran 69 el
11-08 y 88 el 18-08).

⚠ **Dos puertas es la FORMA; 93 es el TAMANO.** Confundirlas hace que la ley
suene mejor de lo que es. Lo que impide que 93 se conviertan en 350 es la regla
`R-REX3`: *comodidad es cabecera, autoridad es operacion*.

★ Las cinco que entraron el 19-08 --`TASK_OP_HIJO` y las cuatro de `KIND_TAREA`--
son el ejemplo de esa regla funcionando **a favor** de crecer: cerrar un proceso
es AUTORIDAD, no comodidad, asi que le tocaba operacion. La regla no existe para
que la superficie no crezca nunca: existe para que cuando crezca se sepa por que.

Un tercer syscall existio y se retiro el 2026-08-10 --`CHANNEL_KICK` era una
operacion con numero propio-- y **su numero no se recicla**: un binario viejo
que lo llame falla diciendolo.

### 2.2 REX -- las diez cabeceras

2.360 lineas en `toolchain/forge/sem-asm/tables/bmo/`, con su indice al lado:
`bmo.h` (las dos puertas), `archivo.h`, `entrada.h`, `monton.h`, `musica.h`,
`paquete.h`, `superficie.h`, `scroll.h`, `sonido.h`, `bloque.h`. Y `bmo-rt`
(922 lineas) como enlace de Rust.

★ **Una cabecera de REX trae el cuerpo.** No hay `libbmo.so` que alguien tenga
que resolver despues, porque no hay enlazado dinamico: lo que incluyes compila
hacia dentro de tu `.bex`. La ley entera esta en
[`META-SDK_HARD.md`](META-SDK_HARD.md).

### 2.3 Los lenguajes de la casa

| lenguaje | estado | ejemplos que corren |
|---|---|---|
| **BMO C** | 32/32 sondas del lenguaje; `<stdlib.h>` con monton de verdad | **13** |
| **BMO COBOL** | decimal exacto en centavos, File I/O | **12** |
| **Ada** | empezado, con plan escrito | 1 |
| C++ | ⏸ **aparcado**, con su motivo y su fecha | 0 |

Ninguno es obligatorio, y esa es la propiedad: una app depende de dos syscalls,
no del lenguaje en que se escribio.

### 2.4 La cara -- MAQUETA

La maquetacion se compila **en el anfitrion** y emite coordenadas: una app no
lleva dentro un motor de composicion. Cinco crates, uno por generacion
(`lex`, `node`, `cascade`, `layout`, `verdict`), y desde el 18-08 esa cadena
esta **probada** y no solo afirmada -- ver
[`herencia.py`](toolchain/tools/censo-modular/herencia.py).

### 2.5 El paquete -- un fichero

Un `.bex` con sus recursos **dentro** (seccion `Resources` 0x0B): icono `BICO`,
datos, lo que haga falta. Lo escribe `bmo-pack`. Una app no son varios ficheros
que se puedan separar, asi que no existe el fallo de que falte uno a mitad de
ejecucion.

### 2.6 Los guardianes -- seis, y cada uno ya rechazo algo

Corren dentro de `build.ps1`. Un guardian que hay que acordarse de invocar no
protege igual que el que se corre solo.

| guardian | dice NO a |
|---|---|
| `ascii-sweep` | un byte no-ASCII donde la regla no lo permite |
| `enlaces` | una cita a un documento que no existe |
| `censo-modular` (L6a) | un modulo nuevo por encima de 1.000 lineas, o uno que crecio |
| `herencia` (L7) | una generacion que depende de otra mas alta |
| contrato de Ring 0 | el kernel y `bmo-abi` diciendo cosas distintas |
| caras de MAQUETA | una cara generada que ya no corresponde a su `.maqueta` |

★ El de L6a es un **trinquete y no un muro**: no juzga los dieciocho ficheros
que ya incumplen --uno que grita sin motivo se apaga-- sino el delta contra su
linea base. El arbol solo puede mejorar.

### 2.7 Las leyes

Las tres, en la raiz, y son parte de lo que se entrega:
[`META-KERNEL_HARD.md`](META-KERNEL_HARD.md) (la maquina),
[`META-APP_HARD.md`](META-APP_HARD.md) (una app),
[`META-SDK_HARD.md`](META-SDK_HARD.md) (REX).

---

## 3. LO QUE SE EXIGE A CAMBIO

Las siete de [`META-APP_HARD.md`](META-APP_HARD.md), en una linea cada una. Cada
una alli trae **de donde sale** y **que se rompe** sin ella:

| | |
|---|---|
| `R-APP1` | es UN fichero |
| `R-APP2` | declara lo que necesita, y no puede mentir |
| `R-APP3` | dibuja en SU memoria y la ofrece; no toma la pantalla |
| `R-APP4` | sube su `sequence` cuando el dibujo esta ENTERO |
| `R-APP5` | nadie se cree sus numeros |
| `R-APP6` | muere sin llevarse a nadie |
| `R-APP7` | su cara es dato o codigo generado, nunca un motor |

★ Las siete se resumen en una: **una app rota no se lleva el escritorio.** Y
aqui el aislamiento no es una promesa de la libreria: es la frontera del
proceso.

---

## 4. LO QUE NO SE CONCEDE, DICHO A PROPOSITO

La regla que hace util esta seccion: **un "no" con motivo escrito se puede
discutir; uno sin motivo es un agujero.**

| hueco | estado real hoy | que lo desbloquea |
|---|---|---|
| **mas de un fichero por proyecto** | ★ una sola unidad de traduccion. Es el techo que mas se nota viniendo de fuera | compilacion separada |
| **entrada dentro de una ventana** | una app puede ENSENAR; no la puedes TOCAR | la casilla 4 de META-APP |
| **sonido de verdad** | hay contrato (`KIND_AUDIO`) y el altavoz del PC; no hay driver HDA ni isocrono por USB | [`AUDIO_MAESTRO.md`](docs/maestro/AUDIO_MAESTRO.md) |
| **hilos** | no hay hilos de Ring 3, y `BRECHA.md` lo dice cuatro veces | SMP cableado, y no antes |
| **red** | no hay pila | [`RED_MAESTRO.md`](docs/maestro/RED_MAESTRO.md) |
| **COBOL y Ada contra REX** | REX es C y Rust; los otros dos no tienen enlace | `R-REX4`: la tabla y lo generado |

★ Y lo que **no va a caber**, para que nadie se lo prometa: un navegador, una
suite ofimatica moderna, nada que arrastre millones de lineas ajenas. No es un
problema de sistema operativo: es de plantilla. **Un sistema que promete
Photoshop es un sistema que no termina la calculadora.**

---

## 5. POR DONDE EMPIEZA UNO DE FUERA

Tres escalones, y cada uno tiene su fichero exacto:

```
   1  UN PROGRAMA         toolchain/lang/c/examples/hola_C.c
                          imprime, lee, y ya cruza las dos puertas

   2  UNA CARA            toolchain/lang/c/examples/raycaster_C.c
                          pixeles a una superficie propia

   3  UN PAQUETE          toolchain/lang/c/examples/caja_C.c  +  bmo-pack
                          un .bex con sus datos dentro y su icono
```

Y el buscador de cabeceras mira en este orden, **gana el primero que tenga el
fichero**:

```
   1  $BMO_MODS    una o varias rutas separadas por ';'   <- la puerta de los terceros
   2  mods/        en la raiz del repo, si existe
   3  tables/      las tablas del sistema
```

★★ Por eso un tercero puede **sustituir una pieza de REX por la suya sin
bifurcar el repo**. Es la mitad del fuero que suele faltar: no solo lo que se
concede, sino que se puede cambiar sin pedir permiso.

[!] Los guiones se ejecutan con el `cwd` en la **raiz del repo**: la busqueda de
raices sube desde el directorio actual, y desde el arbol de un proyecto de fuera
no encuentra `tables/`.

---

## 6. LO QUE EL FUERO NO ES

- **No es un framework.** En cuanto una app dependa de una capa que no sea la
  superficie, la superficie deja de ser el contrato. Las dos preguntas que lo
  impiden estan en [`META-SDK_HARD.md`](META-SDK_HARD.md) apartado 2.
- **No es un runtime.** Nada de aqui tiene que estar vivo para que una app
  arranque.
- **No es un manual de API.** Cada cabecera se explica a si misma en su
  cabeza; esto dice que hay y por donde se entra.
- **No promete compatibilidad con nadie.** Se parece a SDL porque el problema es
  el mismo, no porque sea un clon. Una app de SDL no compila aqui.

---

## 7. COMO CRECE ESTE DOCUMENTO

- **Una pieza entra cuando dos apps ya la escribieron por separado** (`R-REX5`).
  No antes. Una API general sin clientes es coste sin comprador.
- **Los numeros se vuelven a medir, no se copian.** Este fichero lleva 88
  operaciones y 2.316 lineas porque se contaron el 18-08. La cifra anterior
  --39-- sobrevivio meses en tres documentos porque nadie la volvio a medir y
  la fuente se habia mudado de sitio.
- **Un hueco del apartado 4 se tacha con su fecha**, no se borra. Lo que se
  quito de la lista dice tanto como lo que sigue en ella.

---

Ver [`META-KERNEL_HARD.md`](META-KERNEL_HARD.md),
[`META-APP_HARD.md`](META-APP_HARD.md),
[`META-SDK_HARD.md`](META-SDK_HARD.md),
[`EL_CONTRATO_DE_CARGA.md`](docs/identidad/EL_CONTRATO_DE_CARGA.md) (declarar y
conceder) y [`QUE_DESBLOQUEA.md`](docs/identidad/QUE_DESBLOQUEA.md) (que
desbloquea que).
