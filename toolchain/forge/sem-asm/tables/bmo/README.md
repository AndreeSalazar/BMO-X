# REX -- las cabeceras con las que se escribe una app de BMO-X

> **La ley esta en [`META-SDK_HARD.md`](../../../../../FUERO/META-SDK_HARD.md)** y el
> reparto entero --lo que el sistema concede y lo que exige-- en
> [`EL_FUERO.md`](../../../../../FUERO/EL_FUERO.md). Esto es el indice de REX: que
> hay, para que sirve cada pieza y por donde se empieza.

** Y desde el 2026-09-11 `tables/` tiene nombre en el reparto de BMO C: es LA
FABRICA, lo que C promete. Los ports --DOOM, y lo que venga-- son LA EXPANSION,
y la tapan sin tocarla. La regla y el porque estan en
[`toolchain/lang/c/README.md`](../../../../lang/c/README.md).

REX es lo que hay entre las **dos puertas congeladas** (`INVOKE` y `WAIT`) y un
programa. Catorce cabeceras publicas, 4.312 lineas en 23 ficheros, y dos
propiedades que conviene
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

## Las doce piezas

| Cabecera | Lineas | Color | Que resuelve | Ejemplo |
|---|---|---|---|---|
| [`bmo.h`](bmo.h) | 457 | ROJO | las dos puertas, en C. **Se empieza por aqui** | `examples/sonda_C.c` |
| &nbsp;&nbsp;[`bmo/roja.h`](bmo/roja.h) | 157 | ROJO | `INVOKE`, `WAIT` y los numeros de operacion | -- |
| &nbsp;&nbsp;[`bmo/verde.h`](bmo/verde.h) | 229 | VERDE | la tabla `INFO_*`: crece por filas | -- |
| [`archivo.h`](archivo.h) | 522 | ROJO | leer ficheros de verdad, contra `KIND_ARCHIVO` | `examples/leer_C.c` |
| &nbsp;&nbsp;[`archivo/roja.h`](archivo/roja.h) | 340 | ROJO | abrir, `fread`, `fwrite`, `fclose` | -- |
| &nbsp;&nbsp;[`archivo/amarilla.h`](archivo/amarilla.h) | 117 | AMARILLO | el cursor, que es un ESPEJO del del kernel | -- |
| [`entrada.h`](entrada.h) | 372 | AMARILLO | teclado y raton, y **devolverlos** | `examples/pantalla_C.c` |
| [`fuente.h`](fuente.h) | 324 | VERDE | **escribir texto DENTRO de tu superficie** | `examples/texto_C.c` |
| &nbsp;&nbsp;[`fuente/datos.h`](fuente/datos.h) | 152 | VERDE | los glifos 8x16. AUTO-GENERADO por `tools/fontgen`, del MISMO arte que la tabla del kernel | -- |
| [`imagen.h`](imagen.h) | 205 | VERDE | **pintar una imagen que tu app lleva dentro** (`BICO`, a escala entera) | `examples/imagen_C.c` |
| [`monton.h`](monton.h) | 351 | ROJO | `malloc`/`free`/`realloc`. Llega por `<stdlib.h>` | `examples/memoria_C.c` |
| &nbsp;&nbsp;[`monton/roja.h`](monton/roja.h) | 168 | ROJO | la arena y el reparto | -- |
| &nbsp;&nbsp;[`monton/verde.h`](monton/verde.h) | 74 | VERDE | cuanto queda y cuanto cabe | -- |
| [`musica.h`](musica.h) | 269 | VERDE | notas, figuras y compas, encima de `sonido.h` | `examples/vivaldi_C.c` |
| [`pantalla.h`](pantalla.h) | 304 | ROJO | **la pantalla entera**: tomarla, medirla y devolverla | `examples/pantalla_C.c` |
| [`paquete.h`](paquete.h) | 261 | AMARILLO | leer los datos que viajan **dentro** del `.bex` | `examples/caja_C.c` |
| [`prestado.h`](prestado.h) | 284 | ROJO | **memoria que viaja sin copiarse**: prestar, tomar y soltar | `examples/prestado_C.c` |
| [`superficie.h`](superficie.h) | 635 | ROJO | dibujar en TU memoria y ofrecerla al DIRECTOR | `examples/raycaster_C.c` |
| &nbsp;&nbsp;[`superficie/roja.h`](superficie/roja.h) | 176 | ROJO | pedir el bloque y **ofrecerlo** | -- |
| &nbsp;&nbsp;[`superficie/amarilla.h`](superficie/amarilla.h) | 266 | AMARILLO | decodificar eventos, puntero, VISTA y **el caracter cocido** | -- |
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

★★ **El hueco de `entrada.h` se cerro el 2026-09-01**, y lo cerro la cabecera
que faltaba. Llevaba abierto desde el 19-08 y el README lo decia cada dia:
*"ninguno"*. La razon de que no llegara antes estaba escrita aqui mismo --*"un
ejemplo ensenaria a reclamar la pantalla entera, que es el modelo del que se
sale"*-- y era cierta: no habia cabecera para eso.

`examples/pantalla_C.c` ensena las dos a la vez porque el caso real es UNO:
quien toma la pantalla necesita el teclado para poder salir. Y demuestra lo
que ningun otro ejemplo podia demostrar -- que se **devuelven**, y el proceso
sigue vivo.

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

## Con que viaja tu app: imagenes, tablas, y lo que no cabe dentro

La pregunta del dueno el 2026-09-11: *"si los programadores quieren poner
imagenes, sintaxis y eso... aunque en `.bex` se lleva todo, como DOOM"*.

**La regla no es "todo dentro".** Son tres sitios, y el criterio no es el gusto:
es **cuanta RAM cuesta**.

```text
   DENTRO del .bex     un recurso de la seccion `Resources` (0x0B). Lo escribe
   `paquete.h`         `bmo-pack` y lo lee `paquete_leer` EN EJECUCION. Y esto
                       es lo que no se ve: **el cargador SALTA esa seccion**, asi
                       que una imagen dentro de tu `.bex` cuesta CERO RAM hasta
                       que la pides. El icono de tu app ya viaja asi

   AL LADO             lo grande. `doom.bex` son 874 KB y `doom1.wad` 4,2 MB
                       FUERA, y el motivo es un numero, no una preferencia:
                       `lanzar.rs::con_buffer` trae el fichero ENTERO a un bufer
                       de 4 MiB, asi que un paquete de 5,5 MB no cargaria

   UN SERVICIO         lo que usarian muchas apps. Es como ya funciona el
                       DIRECTOR: un proceso, y los demas le hablan por su
                       superficie o por un endpoint. Sin enlazado dinamico es la
                       UNICA forma de no pagar diez copias de lo mismo
```

** Y el formato ya distingue los dos primeros. `CLASE_RECURSOS` de la seccion
`Requisitos` declara **lo que quieres RESIDENTE en RAM**, y dice de si misma:
*"lo que se lee a demanda por su puerta no se declara aqui: eso vive en el disco
y no le cuesta RAM a nadie"*.

### La sintaxis --y casi todo lo que se parece-- es una TABLA

Un resaltado de sintaxis son palabras y colores, no codigo: va como recurso y se
lee con `paquete_leer`, igual que `saludo.txt` dentro de `caja.bex`. Es la regla
de la casa --**tablas y no plugins**-- y la misma por la que los intrinsecos del
ensamblador son un TOML y la cara del escritorio es MAQUETA.

### ⚠ Lo que falta de verdad no es el sitio: es el DECODIFICADOR

El contenedor esta. **Lo que no hay es quien descifre una imagen**: no existe un
decodificador de PNG, de JPEG ni de zlib en todo el repo. El unico formato que
BMO-X sabe leer es `BICO` --ocho bytes de cabecera y pixeles BGRA en crudo--, que
es precisamente **una imagen sin decodificador**.

Asi que hoy, un tercero que quiera una imagen tiene dos salidas honestas: llevarla
en crudo (como el icono) o traerse su propio decodificador dentro del `.bex`.

** Y ahi esta el precio de no tener enlazado dinamico, dicho entero: **si diez
apps quieren PNG, son diez copias del decodificador.** A cambio no hay una sola
libreria compartida que pueda romper las diez a la vez -- es el intercambio que
esta casa eligio a proposito, y el mismo que hace que un hola mundo pese 2.791
bytes. El dia que ese numero moleste, la salida ya tiene forma y esta tres
parrafos arriba: un SERVICIO.

## Lo que REX NO tiene, hoy

No para desanimar: para que nadie lo descubra a mitad de un proyecto.

- **Enlace de COBOL y de Ada.** REX es C y Rust (`toolchain/lang/base/bmo-rt`).
- **Un `Ctrl+algo` propio.** Una app con buzon ya recibe el clic (23-08) y **la
  LETRA ya cocida, con tildes y ene** (11-09, bit 62 del evento), pero el
  escritorio se queda TODA tecla con modificador: es su forma de no entregar el
  aparato. Un atajo propio se hace con una tecla desnuda o con un boton de tu
  superficie. Ver `superficie/amarilla.h`.
- **Descifrar una imagen COMPRIMIDA.** No hay PNG, JPEG ni zlib. `imagen.h`
  pinta `BICO` --pixeles en crudo-- y con eso una app ya lleva y dibuja sus
  imagenes; lo que no hay es quien descomprima. Ver la seccion de arriba.
- **Sonido de verdad.** `sonido.h` y `musica.h` existen y debajo hay un contrato
  y el altavoz del PC. No hay driver HDA ni transferencias isocronas por USB.
- **Hilos.** No hay hilos de Ring 3.
- **Mas de un fichero por proyecto.** Una sola unidad de traduccion. Es el techo
  que mas se nota viniendo de fuera.

Ver [`META-SDK_HARD.md`](../../../../../FUERO/META-SDK_HARD.md) (la ley de REX) y
[`META-APP_HARD.md`](../../../../../FUERO/META-APP_HARD.md) (que exige BMO-X de algo
que quiera ser una app).
