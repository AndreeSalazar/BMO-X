/* <stdbool.h> -- bool es int en BMO C, y se dice.
 *
 * Fabrica, no expansion (ver <stdint.h>).
 *
 * `__bool_true_false_are_defined` NO es decoracion: un stdbool.h de verdad lo
 * define, y hay codigo que lo mira --doomtype.h de DOOM entre otros-- para
 * decidir si declara su propio `boolean` como enum. Sin la macro lo declara, y
 * entonces el `false` de ESTE fichero reescribe el miembro del enum en un
 * literal: 52 ficheros que culpaban al compilador, y era esta cabecera.
 *
 *   > Un stub que esta mal no cuenta una verdad mas pequena: cuenta otra.
 */
#ifndef BMO_STDBOOL_H
#define BMO_STDBOOL_H

#define bool int
#define true 1
#define false 0
#define __bool_true_false_are_defined 1

#endif
