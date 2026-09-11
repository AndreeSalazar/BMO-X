/* <errno.h> -- los codigos que un port espera encontrar.
 *
 * Fabrica, no expansion (ver <stdint.h>). Solo los que algun programa ha
 * pedido; los numeros son los de Linux, que es de donde vienen los ports.
 */
#ifndef BMO_ERRNO_H
#define BMO_ERRNO_H
extern int errno;
#define ENOENT 2
#define EINTR 4
#define EAGAIN 11
#define ENOMEM 12
#define EACCES 13
#define EEXIST 17
#define EISDIR 21
#endif
