/* stdlib.h -- conversiones y memoria.
 *
 * `malloc` y `free` los reconoce el codegen (`KIND_MEMORIA`, cuatro bloques por
 * proceso). Aqui va lo que falta y es puro.
 */
#ifndef BMO_STDLIB_H
#define BMO_STDLIB_H

#include <ctype.h>

int atoi(const char *s) {
    int i;
    int signo;
    int v;
    i = 0;
    while (isspace((int)s[i])) {
        i = i + 1;
    }
    signo = 1;
    if (s[i] == '-') {
        signo = -1;
        i = i + 1;
    } else if (s[i] == '+') {
        i = i + 1;
    }
    v = 0;
    while (isdigit((int)s[i])) {
        v = v * 10 + (s[i] - '0');
        i = i + 1;
    }
    /* El estandar dice que `atoi` NO informa de errores: una cadena sin
     * digitos vale 0, igual que la cadena "0". Es una mala interfaz y es la que
     * hay -- quien necesite distinguirlas usa `strtol`, que no esta escrita. */
    return signo * v;
}

int abs(int v) {
    if (v < 0) {
        return -v;
    }
    return v;
}

#endif /* BMO_STDLIB_H */
