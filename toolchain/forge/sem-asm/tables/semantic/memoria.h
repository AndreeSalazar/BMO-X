/* semantic/memoria.h — TLB y caches.
 *
 * Las dos cosas que el CPU guarda a tu espalda para ir rapido, y que hay que
 * saber tirar cuando lo que guardaron deja de ser verdad.
 */
#ifndef SEMANTIC_MEMORIA_H
#define SEMANTIC_MEMORIA_H

#include <semantic/tipos.h>

/* Tira del TLB la traduccion de UNA pagina.
 *
 * ★ Hay que llamarlo despues de cambiar una entrada de tabla de paginas. El TLB
 * no se entera solo: si no se invalida, el CPU sigue usando la traduccion
 * vieja, y el sintoma es una escritura que va a la pagina de antes. Una pagina,
 * no un rango: para muchas sale mas barato recargar CR3 entero. */
void tlb_olvidar(u64 direccion) { __invlpg(direccion); }

/* Expulsa de TODOS los niveles de cache la linea que contiene esa direccion.
 *
 * Para DMA: si el aparato va a LEER de un buffer que acabas de escribir, hay
 * que bajarlo a memoria — la cache no la ve el motor de DMA. */
void cache_expulsar(u64 direccion) { __clflush(direccion); }

/* Escribe TODO lo sucio y vacia las caches. Carisimo: para el cambio de modo de
 * memoria y poco mas. */
void cache_vaciar_todo() { __wbinvd(); }

#endif /* SEMANTIC_MEMORIA_H */
