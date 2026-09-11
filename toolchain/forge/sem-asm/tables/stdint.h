/* <stdint.h> -- los anchos que BMO C PROMETE en x86-64.
 *
 * Es fabrica, no expansion: cualquier programa de C lo pide, y por eso vive
 * aqui al lado de <stdio.h> y no en la carpeta de un port. Nacio en el port de
 * DOOM el 2026-08 --"minimal, for probing"-- y se mudo el 2026-09-11 cuando se
 * vio que el segundo port habria tenido que copiarlo.
 *
 * int es de 32 bits, long y long long de 64, y un puntero cabe en un long.
 * Si un dia esto miente, el sintoma es un error de TIPO, no un numero raro --
 * que es la clase de fallo que se quiere.
 */
#ifndef BMO_STDINT_H
#define BMO_STDINT_H

typedef signed char int8_t;
typedef unsigned char uint8_t;
typedef short int16_t;
typedef unsigned short uint16_t;
typedef int int32_t;
typedef unsigned int uint32_t;
typedef long long int64_t;
typedef unsigned long long uint64_t;
typedef long intptr_t;
typedef unsigned long uintptr_t;

#endif
