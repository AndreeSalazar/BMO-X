/* string.h -- cadenas, las que faltaban.
 *
 * `strlen`, `strcpy`, `strcmp`, `strchr`, `strncmp`, `memcmp`, `memcpy` y
 * `memset` las SINTETIZA el compilador y no hacen falta aqui. Estas seis no
 * existian, y son las que el unity build de DOOM pedia.
 *
 * == memmove NO es memcpy, y esa es la unica de las seis con miga ==
 *
 * `memcpy` puede copiar en cualquier orden porque promete que los bloques no se
 * solapan. `memmove` **no lo promete**, asi que cuando el destino cae dentro
 * del origen hay que copiar HACIA ATRAS: de frente, cada byte escrito pisa uno
 * que todavia no se ha leido.
 *
 * Un `memmove` que sea un `memcpy` con otro nombre funciona en todas las
 * pruebas donde los bloques no se tocan, y corrompe datos justo el dia que se
 * usa para lo que existe. No da error: da un numero.
 */
#ifndef BMO_STRING_H
#define BMO_STRING_H

#include <ctype.h>

/* [!] `memmove` NO ESTA AQUI, Y NO ES UN OLVIDO.
 *
 * El codegen lo intercepta POR NOMBRE (`codegen/mod.rs`, `("memmove", 3)`) y
 * emite su propia version, asi que una definicion en esta cabecera **no se
 * llega a llamar nunca**. Se escribio una, con su rama hacia atras y todo, y el
 * banco demostro que no se ejecutaba: seguia dando `ababab`.
 *
 * Y la que emite el codegen **copia de frente**, o sea que es un `memcpy` con
 * otro nombre. Eso funciona en todas las pruebas donde los bloques no se
 * solapan y corrompe datos justo el dia que se usa para lo que `memmove`
 * existe -- que es el caso solapado.
 *
 * Queda apuntado con su fila en el banco (`tests/libc.rs`, marcada como
 * pendiente) porque **el arreglo es del compilador, no de una cabecera**.
 */

char *strncpy(char *dst, const char *src, int n) {
    int i;
    i = 0;
    while (i < n && src[i] != 0) {
        dst[i] = src[i];
        i = i + 1;
    }
    /* El estandar RELLENA de ceros hasta `n`, y no es un detalle: el codigo que
     * usa `strncpy` sobre un buffer reutilizado cuenta con eso. */
    while (i < n) {
        dst[i] = 0;
        i = i + 1;
    }
    return dst;
}

char *strrchr(const char *s, int c) {
    int i;
    char *ultimo;
    ultimo = 0;
    i = 0;
    while (s[i] != 0) {
        if (s[i] == (char)c) {
            ultimo = (char *)&s[i];
        }
        i = i + 1;
    }
    /* Buscar el 0 tambien es legal y devuelve el final: lo usa quien busca la
     * extension de un nombre de fichero. */
    if ((char)c == 0) {
        return (char *)&s[i];
    }
    return ultimo;
}

char *strstr(const char *heno, const char *aguja) {
    int i;
    int j;
    /* La aguja vacia se encuentra al principio. Sin este caso, el bucle de
     * abajo devuelve el heno igual -- pero por accidente, no por decision. */
    if (aguja[0] == 0) {
        return (char *)heno;
    }
    i = 0;
    while (heno[i] != 0) {
        j = 0;
        while (aguja[j] != 0 && heno[i + j] == aguja[j]) {
            j = j + 1;
        }
        if (aguja[j] == 0) {
            return (char *)&heno[i];
        }
        i = i + 1;
    }
    return 0;
}

int strcasecmp(const char *a, const char *b) {
    int i;
    int ca;
    int cb;
    i = 0;
    while (1) {
        ca = tolower((int)a[i]);
        cb = tolower((int)b[i]);
        if (ca != cb) {
            return ca - cb;
        }
        if (ca == 0) {
            return 0;
        }
        i = i + 1;
    }
}

char *strdup(const char *s) {
    int n;
    char *nuevo;
    int i;
    n = 0;
    while (s[n] != 0) {
        n = n + 1;
    }
    nuevo = (char *)malloc(n + 1);
    /* [!] `malloc` PUEDE devolver 0 aqui: el kernel da cuatro bloques por
     * proceso y ni uno mas. Un `strdup` que no lo mire escribe en la direccion
     * 0 -- y en Ring 3 eso es un fault, no un aviso. */
    if (nuevo == 0) {
        return 0;
    }
    for (i = 0; i < n; i = i + 1) {
        nuevo[i] = s[i];
    }
    nuevo[n] = 0;
    return nuevo;
}

#endif /* BMO_STRING_H */
