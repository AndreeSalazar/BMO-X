# REX -- las cabeceras con las que se escribe una app de BMO-X

> **La ley esta en [`META-SDK_HARD.md`](../../../../../META-SDK_HARD.md)** y el
> reparto entero --lo que el sistema concede y lo que exige-- en
> [`EL_FUERO.md`](../../../../../EL_FUERO.md). Esto es el indice de REX: que
> hay, para que sirve cada pieza y por donde se empieza.

REX es lo que hay entre las **dos puertas congeladas** (`INVOKE` y `WAIT`) y un
programa. Nueve cabeceras, 2.316 lineas, y dos propiedades que conviene saber
antes de usarlas:

1. **No es un runtime.** Una cabecera de REX **trae el cuerpo**: no hay
   `libbmo.so` que alguien tenga que resolver despues, porque aqui no hay
   enlazado dinamico. Lo que incluyes, compila hacia dentro de tu `.bex`.
2. **Se puede tapar sin bifurcar el repo.** Estos ficheros son la ultima raiz
   que consulta `bmo-mods`: si dejas tu propia version en `$BMO_MODS` o en
   `mods/`, gana la tuya. Por eso REX vive aqui dentro y no en una carpeta mas
   bonita -- ver la seccion 6 de la ley.

---

## Las nueve piezas

| Cabecera | Lineas | Que resuelve | Ejemplo |
|---|---|---|---|
| [`bmo.h`](bmo.h) | 334 | las dos puertas, en C. **Se empieza por aqui** | `examples/sonda_C.c`, `examples/raycaster_C.c` |
| [`archivo.h`](archivo.h) | 444 | leer ficheros de verdad, contra `KIND_ARCHIVO` | `examples/leer_C.c` |
| [`entrada.h`](entrada.h) | 335 | teclado y raton | ⚠ **ninguno** |
| [`monton.h`](monton.h) | 284 | `malloc`/`free`/`realloc` sobre UN bloque del kernel. Llega por `<stdlib.h>` | `examples/memoria_C.c` |
| [`musica.h`](musica.h) | 255 | notas, figuras y compas, encima de `sonido.h` | `examples/vivaldi_C.c`, `examples/musica_C.c` |
| [`paquete.h`](paquete.h) | 245 | leer los datos que viajan **dentro** del propio `.bex` | `examples/caja_C.c` |
| [`superficie.h`](superficie.h) | 190 | dibujar en TU memoria y ofrecerla al DIRECTOR | ⚠ **ninguno** |
| [`scroll.h`](scroll.h) | 126 | una ventana que se mueve sobre un historial | `examples/scroll_C.c` |
| [`sonido.h`](sonido.h) | 103 | el sonido | `examples/sonido_C.c` |

Los ejemplos viven en `toolchain/lang/c/examples/`.

★ **Los dos huecos de la tabla no son casualidad y hay que decirlos**:
`superficie.h` y `entrada.h` son justamente las dos piezas que hacen falta para
escribir la primera app con ventana, y son las dos que **no tienen un ejemplo
que copiar**. Tienen tests (`lang/c/src/tests/puerta.rs`), que prueban que
funcionan pero no ensenan a usarlas.

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
