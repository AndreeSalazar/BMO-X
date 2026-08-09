/* stdlib.h -- conversiones y memoria.
 *
 * `malloc` y `free` los reconoce el codegen (`KIND_MEMORIA`, cuatro bloques por
 * proceso). Aqui va lo que falta y es puro.
 */
#ifndef BMO_STDLIB_H
#define BMO_STDLIB_H

#include <ctype.h>
#include <bmo/bmo.h>

/* Terminar el proceso.
 *
 * [!] **El codigo de salida se pierde, y se dice.** `BMO_OP_SALIR` no lo lleva:
 * el kernel revoca las capabilities del proceso y lo cosecha, y no hay nadie
 * esperando un numero al otro lado -- no hay `wait()` en esta superficie. Un
 * `exit(1)` termina igual que un `exit(0)`.
 *
 * Que exista igual vale: un programa portado llama a `exit` para IRSE, y eso si
 * pasa. Lo que no se hace es fingir que alguien va a leer el codigo. */
void exit(int codigo) {
    bmo_salir();
}

/* `abort` es `exit` sin la cortesia. Aqui son lo mismo porque no hay senales
 * que levantar ni volcado que dejar. */
void abort() {
    bmo_salir();
}

/* `calloc` -- reservar Y poner a cero, que es lo que la distingue.
 *
 * El cero se escribe aqui a mano y no se da por supuesto. Las paginas que
 * entrega el kernel vienen limpias, pero eso es una propiedad del kernel y no
 * el contrato de `calloc`: el dia que `malloc` reparta trozos de un bloque ya
 * usado, quien confiara en la limpieza se encontraria la basura de antes. Y ese
 * fallo aparece lejos de aqui. */
void *calloc(unsigned long long n, unsigned long long tam) {
    char *p;
    unsigned long long total;
    unsigned long long i;

    total = n * tam;
    p = (char *)malloc(total);
    if (p == 0) {
        return 0;
    }
    for (i = 0; i < total; i = i + 1) {
        p[i] = 0;
    }
    return p;
}

/* `realloc` DEVUELVE 0, y hay que decir por que.
 *
 * No es que falte escribirla: es que hoy `malloc` no es un asignador. El kernel
 * entrega **bloques grandes, enteros y contiguos** --cuatro por proceso y ni uno
 * mas-- y no hay forma de devolver uno ni de saber cuanto media el anterior. Sin
 * el tamano viejo, copiar es adivinar: copiar el nuevo tamano lee fuera del
 * bloque de origen cuando se esta creciendo, que es el caso normal.
 *
 * Devolver 0 es el "no se pudo" que el estandar define y que quien llama esta
 * obligado a mirar. Fingir que se pudo daria un puntero con basura detras.
 *
 * * Esto se arregla con **el asignador de Ring 3 sobre `KIND_MEMORIA`**, que ya
 * esta en la hoja de ruta: un `malloc` de verdad encima del bloque grande. Ese
 * dia esta funcion se escribe en tres lineas. */
void *realloc(void *p, unsigned long long tam) {
    return 0;
}

/* -- Las que aqui NO tienen a quien preguntar -------------------------
 *
 * Contestan lo que el estandar define para "no se pudo", y no es una carencia
 * que tapar: son preguntas que en BMO-X **no tienen respuesta**, no cosas que
 * falten por escribir.
 */

/* No hay variables de entorno. Y no es un hueco: el entorno de un proceso de
 * BMO-X son sus CAPABILITIES, que no son cadenas ni se heredan por nombre. Un
 * `getenv("DOOMWADDIR")` contesta que no hay, y quien llama usa su valor por
 * defecto -- que es exactamente lo que hace DOOM. */
char *getenv(const char *nombre) {
    return 0;
}

/* No hay shell que invocar, y tampoco va a haberla: lanzar un programa aqui es
 * `BMO_OP_EJECUTAR` con una ruta, no una cadena que otro interpreta. `-1` es
 * "no se pudo ejecutar el mandato", que es la verdad. */
int system(const char *mandato) {
    return -1;
}

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

/* `atof` -- y es la primera funcion de la libc de BMO que devuelve un `double`.
 *
 * Se apoya en la ruta SSE del compilador: los `double` se evaluan en `xmm`,
 * devolver uno funciona (`xmm0`) y desde esta misma sesion **pasarlo tambien**
 * -- viaja en una ranura de pila como cualquier otro argumento.
 *
 * Sin exponente (`1e-3`): el codigo que la usa aqui lee ganancias y volumenes
 * de un fichero de configuracion, y ahi no aparece. Si algun dia aparece, lo
 * que hoy devuelve es el numero hasta la `e`, no basura. */
double atof(const char *s) {
    double v;
    double escala;
    int i;
    int neg;

    i = 0;
    while (s[i] == ' ' || s[i] == '\t') { i = i + 1; }
    neg = 0;
    if (s[i] == '-') { neg = 1; i = i + 1; }
    else if (s[i] == '+') { i = i + 1; }

    v = 0.0;
    while (s[i] >= '0' && s[i] <= '9') {
        v = v * 10.0 + (double)((int)s[i] - '0');
        i = i + 1;
    }
    if (s[i] == '.') {
        i = i + 1;
        escala = 0.1;
        while (s[i] >= '0' && s[i] <= '9') {
            v = v + (double)((int)s[i] - '0') * escala;
            escala = escala * 0.1;
            i = i + 1;
        }
    }
    if (neg) { return -v; }
    return v;
}

int abs(int v) {
    if (v < 0) {
        return -v;
    }
    return v;
}

#endif /* BMO_STDLIB_H */
