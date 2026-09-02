/* monton/verde.h -- los instrumentos: cuanto queda y cuanto cabe
 *
 * Un CARRIL de `<bmo/monton.h>` (L6g). La cabecera entera --que
 * explica por que existe esta pieza-- esta en la fachada; aqui va lo
 * que cambia de color.
 *
 * [carril]  VERDE        solo LEEN la arena y suman. Ni reservan ni liberan ni
 *                        cruzan la puerta, asi que cambiarlos no arrastra nada
 * [cuesta]  NADA         se equivoca y un panel dice un numero que no es
 * [riesgo]  SILENCIO     `libre` y `hueco_mayor` contestan cosas DISTINTAS a
 *                        proposito -- la diferencia entre las dos es la
 *                        fragmentacion. Confundirlas da un `si cabe` que no
 *                        cabe
 */
#ifndef BMO_MONTON_VERDE_H
#define BMO_MONTON_VERDE_H

#include <bmo/monton/roja.h>

/* -- Lo que el monton deja mirar, para que no haya que creerselo ------
 *
 * Regla 7 de `docs/identidad/LA_RAM.md`: lo que se declara, se cumple o se grita. Un
 * asignador que no sabe contarse a si mismo no puede prometer nada.
 */

/* Bytes libres SUMADOS. No es lo mismo que "cabe una peticion de este tamano":
 * pueden estar repartidos en huecos sueltos, y esa diferencia es justo la
 * fragmentacion. Por eso hay tambien `bmo_monton_hueco_mayor`. */
unsigned long long bmo_monton_libre() {
    unsigned long long p;
    unsigned long long suma;
    struct BMO_TROZO *t;

    suma = 0;
    p = __bmo_monton_ini;
    while (p != 0 && p < __bmo_monton_fin) {
        t = (struct BMO_TROZO *)p;
        if (t->libre == 1) {
            suma = suma + t->tam - 16;
        }
        p = p + t->tam;
    }
    return suma;
}

/* El hueco contiguo mas grande, ya fusionado. Este SI dice cuanto cabe. */
unsigned long long bmo_monton_hueco_mayor() {
    unsigned long long p;
    unsigned long long mejor;
    unsigned long long corrido;
    struct BMO_TROZO *t;

    mejor = 0;
    corrido = 0;
    p = __bmo_monton_ini;
    while (p != 0 && p < __bmo_monton_fin) {
        t = (struct BMO_TROZO *)p;
        if (t->libre == 1) {
            corrido = corrido + t->tam;
            if (corrido > mejor) {
                mejor = corrido;
            }
        } else {
            corrido = 0;
        }
        p = p + t->tam;
    }
    if (mejor < 16) {
        return 0;
    }
    return mejor - 16;
}

#endif /* BMO_MONTON_VERDE_H */
