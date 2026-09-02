# REX -- las cabeceras con las que se escribe una app de BMO-X

> **La ley esta en [`META-SDK_HARD.md`](../../../../../META-SDK_HARD.md)** y el
> reparto entero --lo que el sistema concede y lo que exige-- en
> [`EL_FUERO.md`](../../../../../EL_FUERO.md). Esto es el indice de REX: que
> hay, para que sirve cada pieza y por donde se empieza.

REX es lo que hay entre las **dos puertas congeladas** (`INVOKE` y `WAIT`) y un
programa. Diez cabeceras publicas, 3.015 lineas, y dos propiedades que conviene
saber antes de usarlas:

1. **No es un runtime.** Una cabecera de REX **trae el cuerpo**: no hay
   `libbmo.so` que alguien tenga que resolver despues, porque aqui no hay
   enlazado dinamico. Lo que incluyes, compila hacia dentro de tu `.bex`.
2. **Se puede tapar sin bifurcar el repo.** Estos ficheros son la ultima raiz
   que consulta `bmo-mods`: si dejas tu propia version en `$BMO_MODS` o en
   `mods/`, gana la tuya. Por eso REX vive aqui dentro y no en una carpeta mas
   bonita -- ver la seccion 6 de la ley.

---

## El semaforo: que arriesgas si tocas cada pieza

Desde el 2026-09-01 **cada cabecera lleva su color** (L6g), y las que tenian dos
masas con costes distintos estan partidas por dentro. La pregunta que contesta
el color no es *que hace*, que ya lo dice el nombre: es **voy a tocar esto, que
arrastro?**

| color | que dice | que exige antes de tocar |
|---|---|---|
| **ROJO** | puede corromper memoria o romper binarios que YA existen | leerlo entero |
| **AMARILLO** | si se equivoca no falla: **convence** | mirar quien lee lo mismo al otro lado |
| **VERDE** | normal y seguro, se puede jugar | nada |

**Fuera no cambia nada.** `#include <bmo/archivo.h>` sigue trayendo lo mismo:
las cabeceras partidas conservan nombre y sitio y son una **fachada**, igual que
un `mod.rs` que re-exporta. Incluir un carril suelto tambien vale.

## Las diez piezas

| Cabecera | Lineas | Color | Que resuelve | Ejemplo |
|---|---|---|---|---|
| [`bmo.h`](bmo.h) | 437 | ROJO | las dos puertas, en C. **Se empieza por aqui** | `examples/sonda_C.c` |
| &nbsp;&nbsp;[`bmo/roja.h`](bmo/roja.h) | 137 | ROJO | `INVOKE`, `WAIT` y los numeros de operacion | -- |
| &nbsp;&nbsp;[`bmo/verde.h`](bmo/verde.h) | 229 | VERDE | la tabla `INFO_*`: crece por filas | -- |
| [`archivo.h`](archivo.h) | 522 | ROJO | leer ficheros de verdad, contra `KIND_ARCHIVO` | `examples/leer_C.c` |
| &nbsp;&nbsp;[`archivo/roja.h`](archivo/roja.h) | 340 | ROJO | abrir, `fread`, `fwrite`, `fclose` | -- |
| &nbsp;&nbsp;[`archivo/amarilla.h`](archivo/amarilla.h) | 117 | AMARILLO | el cursor, que es un ESPEJO del del kernel | -- |
| [`entrada.h`](entrada.h) | 349 | AMARILLO | teclado y raton | ** **ninguno** |
| [`monton.h`](monton.h) | 351 | ROJO | `malloc`/`free`/`realloc`. Llega por `<stdlib.h>` | `examples/memoria_C.c` |
| &nbsp;&nbsp;[`monton/roja.h`](monton/roja.h) | 168 | ROJO | la arena y el reparto | -- |
| &nbsp;&nbsp;[`monton/verde.h`](monton/verde.h) | 74 | VERDE | cuanto queda y cuanto cabe | -- |
| [`musica.h`](musica.h) | 269 | VERDE | notas, figuras y compas, encima de `sonido.h` | `examples/vivaldi_C.c` |
| [`paquete.h`](paquete.h) | 261 | AMARILLO | leer los datos que viajan **dentro** del `.bex` | `examples/caja_C.c` |
| [`superficie.h`](superficie.h) | 515 | ROJO | dibujar en TU memoria y ofrecerla al DIRECTOR | `examples/raycaster_C.c` |
| &nbsp;&nbsp;[`superficie/roja.h`](superficie/roja.h) | 176 | ROJO | pedir el bloque y **ofrecerlo** | -- |
| &nbsp;&nbsp;[`superficie/amarilla.h`](superficie/amarilla.h) | 167 | AMARILLO | decodificar eventos y puntero | -- |
| [`scroll.h`](scroll.h) | 140 | VERDE | una ventana que se mueve sobre un historial | `examples/scroll_C.c` |
| [`sonido.h`](sonido.h) | 118 | AMARILLO | el sonido | `examples/sonido_C.c` |
| [`bloque.h`](bloque.h) | 53 | ROJO | que bloque del kernel es el del monton | -- |

Los ejemplos viven en `toolchain/lang/c/examples/`.

** **Lo comprueba una maquina**, no la buena voluntad: `contrato.py --check`,
reglas R11 y R12. R11 exige las tres etiquetas y **UNA sola clase de
`[cuesta]`** -- dos significa que el fichero esta mal cortado, y es la regla que
creo estas cuatro carpetas. R12 exige que la carpeta no mezcle y que **la
fachada traiga todos sus carriles**: uno que se quede fuera no da un `fichero no
encontrado`, da un simbolo sin declarar a nueve capas de distancia.

★ **Queda UN hueco, y es el que importa ahora**: `entrada.h` no tiene ejemplo.
Eran dos hasta el 2026-08-19, cuando `raycaster_C.c` se porto a ventana (paso 2b
de `PLAN_DIRECTOR.md`) y `superficie.h` gano el suyo. El de `entrada.h` llega con
el paso 2c: hoy una app en ventana no recibe teclas, asi que un ejemplo ensenaria
a reclamar la pantalla entera, que es el modelo del que se sale.

[!] Y lo que ese puerto destapo: `superficie.h` leia `__bmo_bloque_cap` **sin
traerlo**, asi que una app que solo queria una ventana no compilaba. De ahi sale
`bloque.h`.

---

## Como se usa

```c
#include <bmo/bmo.h>          /* las dos puertas */
#include <bmo/superficie.h>   /* y lo que necesites */
```

El buscador de cabeceras es `toolchain/forge/bmo-mods`, y mira en este orden --
**gana el primero que tenga el fichero**:

```
   1  $BMO_MODS       una o varias rutas separadas por ';'   <- la puerta de los terceros
   2  mods/           en la raiz del repo, si existe
   3  tables/         esto de aqui: las tablas del sistema
```

[!] Los guiones se ejecutan con el `cwd` en la **raiz del repo**: la busqueda de
raices sube desde el directorio actual, y desde el arbol de un proyecto de fuera
no encuentra `tables/`.

---

## Lo que REX NO tiene, hoy

No para desanimar: para que nadie lo descubra a mitad de un proyecto.

- **Enlace de COBOL y de Ada.** REX es C y Rust (`toolchain/lang/base/bmo-rt`).
- **Entrada dentro de una ventana.** `entrada.h` habla por relevo de pantalla
  entera; una app en un marco todavia no recibe el clic.
- **Sonido de verdad.** `sonido.h` y `musica.h` existen y debajo hay un contrato
  y el altavoz del PC. No hay driver HDA ni transferencias isocronas por USB.
- **Hilos.** No hay hilos de Ring 3.
- **Mas de un fichero por proyecto.** Una sola unidad de traduccion. Es el techo
  que mas se nota viniendo de fuera.

Ver [`META-SDK_HARD.md`](../../../../../META-SDK_HARD.md) (la ley de REX) y
[`META-APP_HARD.md`](../../../../../META-APP_HARD.md) (que exige BMO-X de algo
que quiera ser una app).
