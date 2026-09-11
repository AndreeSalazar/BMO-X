/* ctype.h -- clasificacion de caracteres.
 *
 * Doce funciones de tres lineas que **no estaban**, y por eso el unity build de
 * DOOM se paraba. No son un reto tecnico: son el peaje de no heredar una libc.
 *
 * == Se escriben en C, no en el codegen ==
 *
 * `strlen` y `memcpy` son funciones SINTETIZADAS: el compilador emite sus bytes.
 * Eso tiene sentido para las que valen la pena en ensamblador --`memcpy` es un
 * `rep movsb`-- y ninguno para `isspace`, que es una comparacion.
 *
 * Escribirlas aqui sale gratis y se leen. Y **el nombre del fichero es el
 * estandar a proposito**: un programa de fuera que hace `#include <ctype.h>` las
 * encuentra sin cambiar una linea. Esa es la portabilidad de verdad -- no que el
 * lenguaje sea C, sino que las cabeceras se llamen como se llaman en todas
 * partes.
 *
 * [!] Solo ASCII. Un `isalpha` que dijera que si a los bytes altos seria mentir
 * en un sistema cuya consola es Latin-1: la 'n con tilde' es una letra para una
 * persona y un byte suelto para este codigo.
 */
#ifndef BMO_CTYPE_H
#define BMO_CTYPE_H

int isspace(int c) {
    /* Los seis del estandar. `\v` y `\f` casi nunca aparecen, pero un
     * `isspace` que no los cuente falla justo en el fichero raro. */
    if (c == ' ' || c == '\t' || c == '\n' || c == '\r' || c == 11 || c == 12) {
        return 1;
    }
    return 0;
}

int isdigit(int c) {
    if (c >= '0' && c <= '9') {
        return 1;
    }
    return 0;
}

int isalpha(int c) {
    if (c >= 'a' && c <= 'z') {
        return 1;
    }
    if (c >= 'A' && c <= 'Z') {
        return 1;
    }
    return 0;
}

int isalnum(int c) {
    if (isalpha(c)) {
        return 1;
    }
    return isdigit(c);
}

int toupper(int c) {
    if (c >= 'a' && c <= 'z') {
        return c - 32;
    }
    return c;
}

int tolower(int c) {
    if (c >= 'A' && c <= 'Z') {
        return c + 32;
    }
    return c;
}

#endif /* BMO_CTYPE_H */
