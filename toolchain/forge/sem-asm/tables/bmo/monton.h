/* monton.h -- EL ASIGNADOR DE RING 3, sobre un bloque de `KIND_MEMORIA`.
 *
 * == El hueco que tapa, con su numero ==
 *
 * Hasta el 2026-08-09 cada `malloc` de C era **una peticion al kernel**, y el
 * kernel da **cuatro por proceso**. El quinto devolvia 0. Para lo que se
 * escribio --DOOM pide su zona UNA vez y se la administra con `Z_Zone`-- eso
 * parecia suficiente, y la cuenta estaba mal hecha: el arranque de DOOM llama a
 * `malloc` **una docena de veces**. Solo `I_AtExit` son siete, uno por cada
 * funcion que se registra al salir; y ademas van `DG_ScreenBuffer`, la zona de
 * 6 MiB, el directorio de lumps del WAD, la paleta y las rutas.
 *
 * Con cuatro peticiones, DOOM muere en el quinto `malloc` con un `I_Error`.
 *
 * == Por que el kernel NO hace esto, y esta bien que no lo haga ==
 *
 * `ring0/obj/memoria.rs` lo dice en su cabecera: el kernel entrega **un bloque
 * grande, entero y contiguo**, y el reparto es POLITICA. La politica vive en
 * Ring 3, donde cada lenguaje puede traer la suya sin pedirle permiso a nadie.
 * Esto es la de C.
 *
 * == Las tres cosas que arregla de golpe ==
 *
 * 1. **Los mas de cuatro `malloc`.** Una peticion al kernel, N reparticiones.
 * 2. **`realloc`**, que devolvia 0 y decia por que: *"sin el tamano viejo,
 *    copiar es adivinar"*. Ahora el tamano viejo esta en la cabecera del
 *    bloque, a ocho bytes del puntero. Se escribe en tres lineas, como estaba
 *    prometido en `<stdlib.h>`.
 * 3. ** **El contrato de `fread`.** El kernel solo acepta escribir dentro de un
 *    bloque que el concedio, y `<bmo/archivo.h>` traduce el puntero a un
 *    desplazamiento contra `__bmo_bloque_base`. Con un `malloc` por peticion,
 *    solo el PRIMER bloque estaba publicado: leer un fichero a cualquier otro
 *    devolvia 0 **sin quejarse**. Con un solo bloque para todo, cualquier
 *    puntero del monton cae dentro del publicado. El contrato deja de depender
 *    del orden en que se pidio la memoria.
 *
 * == UNA sola arena, y es una decision ==
 *
 * El monton pide UN bloque y no vuelve a pedir. Cuando se acaba, `malloc`
 * devuelve 0.
 *
 * Podria pedir un segundo --el kernel da cuatro-- y no lo hace por el punto 3:
 * `fread` traduce contra la base de UN bloque, asi que una segunda arena
 * traeria de vuelta el fallo silencioso que este fichero acaba de quitar. Un
 * `malloc` que dice "no" es mejor que un `fread` que dice "lei cero".
 *
 * == Cuanto pide ==
 *
 * `BMO_MONTON_BYTES`, y **el programa lo declara**:
 *
 *     #define BMO_MONTON_BYTES (12 * 1024 * 1024)
 *     #include <stdlib.h>
 *
 * Por defecto **1 MiB**: lo bastante para un programa normal, y lo bastante
 * poco para que un `hola mundo` que llame a `malloc(32)` no se lleve por
 * delante ocho megas de RAM fisica contigua. Quien necesita mas lo dice, que es
 * la misma idea que la seccion `Manifest` de BEF persigue en grande.
 *
 * Y no se pide nada hasta el primer `malloc`: un programa que no reserva
 * memoria no gasta ni una pagina ni una de sus cuatro peticiones.
 *
 * == La forma, y por que esta ==
 *
 * Boundary tags con lista implicita: los bloques van pegados dentro de la
 * arena y se recorren sumando su tamano. Cada uno lleva 16 bytes de cabecera
 * --el tamano total y si esta libre-- y el reparto es **primer hueco que
 * sirve**.
 *
 * ** La fusion de huecos se hace AL BUSCAR, no al liberar**, y eso es lo que
 * quita el puntero al bloque anterior: fusionar en `free` obliga a saber quien
 * hay detras, y para eso hace falta o un enlace mas por bloque o recorrer la
 * arena entera desde el principio. Recorriendola ya en la busqueda, dos huecos
 * seguidos se ven solos y se juntan sin que nadie lleve un puntero de mas.
 *
 * El coste es lineal en el numero de bloques. Se dice porque es real: para los
 * quince `malloc` de DOOM no significa nada, y para un programa que reserve
 * cien mil trozos si. Ese dia lo que toca es una lista de libres por tamano, no
 * un parche aqui -- y la forma de saber que ha llegado ese dia es medirlo, no
 * suponerlo.
 *
 * -- ** ESTE FICHERO SE PARTIO (L6g, nivel 3) -------------------------
 *
 * Tenia DOS MASAS con costes distintos, y la regla de L6e dice que eso
 * es un fichero mal cortado: *el corte va justo por donde cambia el
 * coste*. Cada mitad vive ahora en `bmo/monton/` con su semaforo,
 * su `[cuesta]` y su `[riesgo]`.
 *
 * **Fuera no cambia nada.** `#include <bmo/monton.h>` sigue trayendo
 * lo mismo que traia: esto es la fachada, igual que un `mod.rs` que
 * re-exporta. Incluir un carril suelto tambien vale.
 *
 *     roja.h     la arena y el reparto -- boundary tags EN BANDA, sin red
 *     verde.h    los instrumentos: cuanto queda y cuanto cabe
 *
 * [carril]  ROJO         el reparto, y hereda el color del carril que manda
 * [cuesta]  DATO         hereda de `roja.h`: repartir dos veces el mismo trozo
 *                        es dos duenos de un byte
 * [riesgo]  AJENO SILENCIO
 *                        hereda de `roja.h`: las cabeceras van EN BANDA y
 *                        pisarlas no da fault
 *
 */
#ifndef BMO_MONTON_H
#define BMO_MONTON_H

#include <bmo/monton/roja.h>
#include <bmo/monton/verde.h>

#endif /* BMO_MONTON_H */
