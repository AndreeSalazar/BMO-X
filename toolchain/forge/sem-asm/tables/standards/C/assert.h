/* <assert.h> -- LIMITE DECLARADO: assert no comprueba nada en ejecucion.
 *
 * Fabrica, no expansion (ver <stdint.h>). Y se dice en vez de callarlo: un
 * assert que no hace nada es un assert que MIENTE si nadie lo sabe. Lo que
 * falta para que compruebe es un camino de aborto con mensaje desde un .bex,
 * y ese camino es del sistema, no de esta cabecera.
 */
#ifndef BMO_ASSERT_H
#define BMO_ASSERT_H
#define assert(x) ((void)0)
#endif
