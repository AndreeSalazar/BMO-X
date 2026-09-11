/* <limits.h> -- los topes de los enteros de BMO C en x86-64.
 *
 * Fabrica, no expansion (ver <stdint.h>). Los numeros son los del modelo LP64:
 * int 32, long 64, char con signo.
 *
 * PATH_MAX no es ISO C, es POSIX, y esta aqui porque DOOM lo pide sin incluir
 * nada mas. Se queda dicho para que nadie lo tome por promesa del estandar.
 */
#ifndef BMO_LIMITS_H
#define BMO_LIMITS_H
#define CHAR_MAX 127
#define UCHAR_MAX 255
#define SHRT_MAX 32767
#define SHRT_MIN (-32768)
#define USHRT_MAX 65535
#define INT_MAX 2147483647
#define INT_MIN (-2147483647-1)
#define UINT_MAX 4294967295
#define LONG_MAX 9223372036854775807
#define LONG_MIN (-9223372036854775807-1)
#define PATH_MAX 260
#endif
