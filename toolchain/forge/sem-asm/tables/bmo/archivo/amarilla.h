/* archivo/amarilla.h -- el cursor, que es un ESPEJO del que lleva el kernel
 *
 * Un CARRIL de `<bmo/archivo.h>` (L6g). La cabecera entera --que
 * explica por que existe esta pieza-- esta en la fachada; aqui va lo
 * que cambia de color.
 *
 * [carril]  AMARILLO     no toca memoria de nadie: hace cuentas sobre
 *                        `f->pos`, que es una copia local de un estado que
 *                        vive en el kernel. Se puede cambiar sin arrastrar a
 *                        nadie, y por eso mismo se puede desincronizar sin que
 *                        nadie avise
 * [cuesta]  NADA         se equivoca y un programa lee la parte del fichero
 *                        que no era
 * [riesgo]  ESPEJO SILENCIO
 *                        el propio fichero lo dice: `llevar un espejo de un
 *                        estado que vive en otro sitio es una cosa que sale
 *                        mal sola`. Ya salio mal DOS veces: `SEEK_END`
 *                        ignorado dejo el WAD midiendo cero, y `feof`
 *                        comparando contra lo que QUEDA daba EOF pasada la
 *                        mitad
 */
#ifndef BMO_ARCHIVO_AMARILLA_H
#define BMO_ARCHIVO_AMARILLA_H

#include <bmo/archivo/roja.h>

/* Mover el cursor. `desde` es `SEEK_SET` (0), `SEEK_CUR` (1) o `SEEK_END` (2).
 *
 * ** LOS TRES, y no solo el primero. Hasta el 2026-08-09 esta funcion hacia
 * `(void)desde` con el comentario *"solo SEEK_SET por ahora, y se dice"*, y
 * decirlo no bastaba: `M_FileLength` de DOOM --el que mide el WAD-- es
 *
 *     fseek(handle, 0, SEEK_END);  length = ftell(handle);
 *
 * o sea que con `SEEK_END` ignorado el cursor se iba a 0, `ftell` devolvia 0 y
 * **el WAD media cero bytes**. Sin un error, sin una linea: `W_AddFile` con un
 * fichero de longitud cero. Un fallo que no se parece en nada a su causa.
 *
 * El kernel solo sabe SALTAR a una posicion absoluta, y esta bien que sea asi:
 * `SEEK_CUR` y `SEEK_END` son aritmetica sobre dos numeros que este lado ya
 * tiene -- el cursor propio y el tamano que da `BMO_ARCH_TAMANO`. Se resuelven
 * aqui y por la puerta sigue pasando una sola cosa.
 *
 * El desplazamiento va con SIGNO porque el estandar lo define asi y porque
 * `SEEK_END` sin negativos no sirve de nada: `fseek(f, -4, SEEK_END)` es como se
 * lee la cola de un fichero. Un destino que quedara por debajo de cero se
 * recorta a cero -- el estandar dice que es indefinido, y aqui lo definido es
 * mejor que lo sorprendente. */
int fseek(FILE *f, long long desplazamiento, int desde) {
    long long destino;

    if (f == 0) return -1;
    if (desde == 1) {
        destino = (long long)f->pos + desplazamiento;
    } else if (desde == 2) {
        /* [!] `bmo_quedan` son los bytes QUE QUEDAN, no el tamano: lo dice el
         * kernel en `ARCH_OP_TAMANO` y lo repite el ABI. El final del fichero
         * es cursor + lo que queda; usarlo como si fuera el tamano da un
         * `SEEK_END` que se mueve segun donde estuviera el cursor. */
        destino = (long long)(f->pos + bmo_quedan(f)) + desplazamiento;
    } else {
        destino = desplazamiento;
    }
    if (destino < 0) {
        destino = 0;
    }
    bmo_valor(f->cap, BMO_ARCH_SALTAR, (unsigned long long)destino, 0, 0);
    f->pos = (unsigned long long)destino;
    return 0;
}

unsigned long long bmo_quedan(FILE *f) {
    if (f == 0) return 0;
    return bmo_valor(f->cap, BMO_ARCH_TAMANO, 0, 0, 0);
}

/* -- Las tres ultimas de la lista de DOOM ------------------------------- */

/* Donde esta el cursor. Sale del espejo del `FILE`, no del kernel: ver `pos`. */
unsigned long long ftell(FILE *f) {
    if (f == 0) {
        return 0;
    }
    return f->pos;
}

/* Se acabo el fichero?
 *
 * [!] Ojo con la semantica, que es la trampa clasica de C: `feof` **no
 * adivina**. En C de verdad solo dice que si DESPUES de que una lectura se
 * quedara corta, no cuando el cursor llega al final. Aqui se contesta con la
 * comparacion directa --cursor contra tamano-- que es lo que un bucle
 * `while (!feof(f))` espera de verdad, y ademas no puede quedarse colgado.
 *
 * Se dice porque es una DIFERENCIA con el estandar, y una diferencia callada es
 * la que muerde a quien trae codigo de fuera. */
int feof(FILE *f) {
    if (f == 0) {
        return 1;
    }
    /* ** ESTO DECIA `f->pos >= bmo_quedan(f)`, Y ESTABA MAL DESPLEGADO.
     *
     * `bmo_quedan` son los bytes QUE QUEDAN por leer, no el tamano del fichero
     * -- `ARCH_OP_TAMANO` lo dice con esas palabras y el ABI lo repite. Contra
     * un fichero de diez bytes con el cursor en el seis quedan cuatro, y
     * `6 >= 4` es cierto: **`feof` daba EOF pasada la mitad de cualquier
     * fichero**. Un `while (!feof(f))` leia poco mas de la mitad y salia
     * tranquilo, sin error, con datos incompletos.
     *
     * Lo que hay que preguntar no lleva resta: se acabo cuando no queda nada. */
    if (bmo_quedan(f) == 0) {
        return 1;
    }
    return 0;
}

#endif /* BMO_ARCHIVO_AMARILLA_H */
