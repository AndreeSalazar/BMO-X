/* semantic/bits.h — contar y buscar bits.
 *
 * Un asignador de marcos de memoria vive de esto. En 16 GiB hay cuatro millones
 * de marcos, o sea cuatro millones de bits: buscar el primero libre recorriendo
 * de uno en uno son cuatro millones de vueltas; con `bsf`, una por palabra de
 * 32. Es la diferencia entre reservar memoria y esperar a que reserve.
 */
#ifndef SEMANTIC_BITS_H
#define SEMANTIC_BITS_H

#include <semantic/tipos.h>

/* Cuantos bits hay a uno. */
u32 bits_contar(u32 v) { return __popcnt(v); }

/* ★ El indice del bit bajo puesto a uno — INDEFINIDO si `v` es cero.
 *
 * "Indefinido" en serio: el CPU deja el destino sin tocar, asi que devuelve lo
 * que hubiera antes. Un mapa de bits lleno pasado por aqui sin comprobar
 * devuelve el indice de la busqueda ANTERIOR y reserva un marco que ya estaba
 * dado. Comprobar el cero es obligatorio, no una precaucion. */
u32 bits_primero(u32 v) { return __bsf(v); }

/* El indice del bit alto. Mismo aviso con el cero. */
u32 bits_ultimo(u32 v) { return __bsr(v); }

/* Como los dos de arriba pero DEFINIDOS en cero: dan 32. Piden BMI1/LZCNT — en
 * un CPU que no los tenga, `tzcnt` se decodifica como `bsf` y devuelve basura
 * en vez de fallar, que es lo peor de los dos mundos. Zen 3 los tiene. */
u32 bits_ceros_derecha(u32 v) { return __tzcnt(v); }
u32 bits_ceros_izquierda(u32 v) { return __lzcnt(v); }

/* Da la vuelta a los bytes. La red y muchos formatos de disco hablan
 * big-endian; x86 es little-endian. */
u32 bytes_al_reves(u32 v) { return __bswap(v); }
u64 bytes_al_reves64(u64 v) { return __bswap64(v); }

#endif /* SEMANTIC_BITS_H */
