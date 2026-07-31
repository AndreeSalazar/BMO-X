/* semantic/tipos.h — los anchos, dichos por su ancho.
 *
 * `int` mide lo que el compilador diga. En un driver eso no vale: un registro
 * de 32 bits del xHCI mide 32 bits en todas las máquinas del mundo, y llamarlo
 * `int` es no decir nada. Aquí los tipos se llaman por lo que miden.
 *
 * Son los mismos que `<stdint.h>` pero sin el `_t`: en BMO C no hay
 * `<stdint.h>` porque no hay libc, y arrastrar el sufijo de un fichero que no
 * existe sólo sirve para que alguien lo busque.
 */
#ifndef SEMANTIC_TIPOS_H
#define SEMANTIC_TIPOS_H

typedef unsigned char u8;
typedef unsigned short u16;
typedef unsigned int u32;
typedef unsigned long long u64;

typedef signed char i8;
typedef short i16;
typedef int i32;
typedef long long i64;

/* Una dirección física. NO es un puntero y no se puede desreferenciar: mientras
 * la paginación esté activa, el CPU no sabe llegar ahí sin una tabla que lo
 * traduzca. Un motor de DMA sí. Tenerla con su propio nombre es lo que impide
 * pasarle una virtual al PRDT de AHCI — que ya se pagó una vez, corrompiendo
 * la página física 0. */
typedef unsigned long long fisica;

#endif /* SEMANTIC_TIPOS_H */
