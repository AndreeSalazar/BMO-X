/* archivo.h -- leer ficheros desde C, de verdad.
 *
 * == Por que esto es una CABECERA y no un builtin del compilador ==
 *
 * Es la misma razon que da `bmo.h` de si misma: aqui no hay libc que enlazar,
 * asi que **la cabecera trae el cuerpo**. Y hay un motivo mas fuerte para que
 * `fopen` no sea un caso del `match` del codegen: abrir un fichero son varias
 * llamadas --empujar la ruta en paquetes de ocho bytes y luego pedir el
 * handle--, y eso escrito a mano en opcodes serian doscientas lineas de bytes
 * que nadie podria leer. En C se lee, se prueba y se corrige.
 *
 * Los intrinsecos del compilador se quedan para lo que es UNA instruccion.
 *
 * == Lo que hace que esto sea rapido, y no lo era ==
 *
 * `ARCH_OP_LEER` devuelve **siete bytes por llamada**. Cargar un WAD de DOOM de
 * 4 MB asi son ~600.000 llamadas al sistema.
 *
 * `ARCH_OP_LEER_EN` copia un bloque entero de golpe. Y no hace falta que el
 * kernel valide ningun puntero --infraestructura que no existe-- porque el
 * destino **es un bloque que concedio el kernel**: comprobar es una resta
 * contra lo que entrego. Contrato en vez de comprobacion.
 *
 * == Como se usa ==
 *
 *     FILE *f = fopen("apps/doom1.wad", "r");
 *     char *mem = malloc(4*1024*1024);
 *     fread(mem, 1, 4*1024*1024, f);
 *     fseek(f, 0, 0);
 *     fclose(f);
 *
 * [!] `fread` solo puede escribir dentro del bloque que dio `malloc`, porque es
 * ese bloque el que el kernel conoce. Un puntero a la pila NO vale, y contesta
 * cero en vez de escribir donde no debe.
 *
 * -- ** ESTE FICHERO SE PARTIO (L6g, nivel 3) -------------------------
 *
 * Tenia DOS MASAS con costes distintos, y la regla de L6e dice que eso
 * es un fichero mal cortado: *el corte va justo por donde cambia el
 * coste*. Cada mitad vive ahora en `bmo/archivo/` con su semaforo,
 * su `[cuesta]` y su `[riesgo]`.
 *
 * **Fuera no cambia nada.** `#include <bmo/archivo.h>` sigue trayendo
 * lo mismo que traia: esto es la fachada, igual que un `mod.rs` que
 * re-exporta. Incluir un carril suelto tambien vale.
 *
 *     roja.h     abrir, LEER, ESCRIBIR y cerrar -- donde el kernel toca tu memoria
 *     amarilla.h el cursor, que es un ESPEJO del que lleva el kernel
 *
 * [carril]  ROJO         el reparto, y hereda el color del carril que manda
 * [cuesta]  DATO         hereda de `roja.h`: es el camino de guardar, y nada
 *                        llega al disco hasta `fclose`
 * [riesgo]  AJENO SILENCIO
 *                        hereda las dos que de verdad muerden: el puntero lo
 *                        escribe el programa, y equivocarse aqui no falla --
 *                        devuelve 0
 *
 */
#ifndef BMO_ARCHIVO_H
#define BMO_ARCHIVO_H

#include <bmo/archivo/roja.h>
#include <bmo/archivo/amarilla.h>

#endif /* BMO_ARCHIVO_H */
