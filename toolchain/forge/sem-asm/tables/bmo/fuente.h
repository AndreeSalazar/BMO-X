/* fuente.h -- escribir texto DENTRO de tu superficie.
 *
 * == Por que esto hacia falta, y cuanto tiempo ==
 *
 * Una app dibuja en SU memoria y el DIRECTOR la compone. El DIRECTOR sabe
 * escribir texto --usa la fuente del kernel a traves de `Pantalla::texto`-- y
 * una app NO: dentro de su ventana solo hay pixeles. La huella de esa carencia
 * esta escrita en `raycaster_C.c`, que explica sus teclas con BARRAS de color
 * porque *"no hay fuente de texto en este ejemplo"*.
 *
 * Sin esto no hay un bloc de notas, ni una lista, ni una etiqueta debajo de un
 * boton. Era el techo de lo que una app podia ENSENAR.
 *
 * == De donde salen los glifos: del MISMO sitio que los del kernel ==
 *
 * De `toolchain/tools/fontgen`, en la misma pasada que escribe la tabla de
 * Ring 0, y por eso `fuente/datos.h` dice AUTO-GENERADO. No es una copia de la
 * fuente del kernel: es la segunda salida del mismo arte.
 *
 * ** Una fuente mantenida en dos sitios son dos fuentes, y se separan el dia
 * que alguien corrige una letra en uno de los dos. Aqui no se puede: el dia que
 * la `n` cambie, cambia en las dos porque las dos se generan.
 *
 * == Lo que cuesta y lo que NO hace ==
 *
 * ```text
 *    8x16 por letra, un bit por pixel      4 KB de tabla en el .bex
 *    un pixel por bit encendido            no hay suavizado ni mezcla
 *    RECORTA contra tu superficie          un texto que no cabe se corta
 * ```
 *
 * ** El recorte no es cortesia: es la diferencia entre un texto que se sale y
 * un `#PF`. Escribir pasado el final del bloque es lo que ya le costo DOS
 * fallos de pagina a `raycaster_C.c` en el Ryzen, y por eso aqui el ancho y el
 * alto de TU superficie son argumentos y no una suposicion.
 *
 * == Como se usa ==
 *
 *     #include <bmo/superficie.h>
 *     #include <bmo/fuente.h>
 *
 *     unsigned int *px = bmo_superficie_pixeles(s);
 *     bmo_texto(px, 640, 640, 400, 8, 8, "hola", 0x00E6EDF7);
 *
 * [carril]  VERDE        escribe en la memoria de la PROPIA app, y recorta
 *                        contra lo que ella declara
 * [cuesta]  NADA         un texto mal dibujado se ve y se arregla
 * [riesgo]  ESPEJO       el formato del glifo --bit 7 = columna izquierda,
 *                        arte en las filas 2..14-- lo fija `fontgen`, y el
 *                        renderer del kernel lee la misma tabla con la misma
 *                        cuenta. Si una de las dos lecturas cambia, la letra
 *                        sale del reves en un solo lado
 */
#ifndef BMO_FUENTE_H
#define BMO_FUENTE_H

#include <bmo/fuente/datos.h>

/* La celda de una letra. El arte vive en las filas 2..14; las otras tres son
 * el interlineado, y por eso dos lineas seguidas no se tocan. */
#define BMO_FUENTE_ANCHO 8
#define BMO_FUENTE_ALTO 16

/* El indice del glifo de un byte, o -1 si esta fuente no lo tiene.
 *
 * ASCII 32..126 van seguidos desde el 0. Los acentos y la ene son Latin-1 (un
 * byte por letra, que es lo que entrega el teclado espanol del kernel) y viven
 * detras, en el orden de `bmo_fuente_latin1`. Un byte sin glifo contesta -1 y
 * quien dibuja pone un hueco: **inventar un glifo seria ensenar otra letra**. */
int bmo_fuente_indice(int byte) {
    int i;
    if (byte >= 32 && byte <= 126) {
        return byte - 32;
    }
    i = 0;
    while (i < BMO_FUENTE_EXTRAS) {
        if ((int)bmo_fuente_latin1[i] == byte) {
            return BMO_FUENTE_ASCII + i;
        }
        i = i + 1;
    }
    return -1;
}

/* Cuanto mide un texto en pixeles. Ocho por letra, sin excepciones: esta fuente
 * es de ancho fijo, y eso es lo que deja alinear columnas sin medir nada. */
int bmo_texto_ancho_n(const char *s, int n) {
    if (s == 0 || n < 0) {
        return 0;
    }
    return n * BMO_FUENTE_ANCHO;
}

int bmo_texto_ancho(const char *s) {
    int n;
    if (s == 0) {
        return 0;
    }
    n = 0;
    while (s[n] != 0) {
        n = n + 1;
    }
    return n * BMO_FUENTE_ANCHO;
}

/* **Escribe `n` bytes** en tu superficie, y devuelve la `x` donde se quedo.
 *
 * `stride` es el paso en PIXELES (el mismo que declara la cabecera `BSUP`);
 * `ancho` y `alto` son los de tu superficie, y son el recorte. Un byte sin
 * glifo avanza el hueco y no dibuja nada.
 *
 * No toca nada fuera de `[0, ancho) x [0, alto)`. Ver el `[riesgo]` de arriba:
 * esa comprobacion es la que no estaba el dia de los dos `#PF`. */
int bmo_texto_n(unsigned int *px, int stride, int ancho, int alto,
                int x, int y, const char *s, int n, unsigned int color) {
    int i;
    int g;
    int fila;
    int col;
    int cx;
    int py;
    unsigned char bits;

    if (px == 0 || s == 0 || stride <= 0) {
        return x;
    }
    i = 0;
    while (i < n) {
        g = bmo_fuente_indice((int)(unsigned char)s[i]);
        if (g >= 0) {
            fila = 0;
            while (fila < BMO_FUENTE_ALTO) {
                py = y + fila;
                bits = bmo_fuente_glifos[g * BMO_FUENTE_ALTO + fila];
                if (bits != 0 && py >= 0 && py < alto) {
                    col = 0;
                    while (col < BMO_FUENTE_ANCHO) {
                        /* Bit 7 = la columna de la izquierda. Es el contrato de
                         * `fontgen`, y el renderer del kernel lee igual. */
                        if ((bits & (0x80 >> col)) != 0) {
                            cx = x + col;
                            if (cx >= 0 && cx < ancho && cx < stride) {
                                px[py * stride + cx] = color;
                            }
                        }
                        col = col + 1;
                    }
                }
                fila = fila + 1;
            }
        }
        x = x + BMO_FUENTE_ANCHO;
        i = i + 1;
    }
    return x;
}

/* Lo mismo con una cadena terminada en cero. */
int bmo_texto(unsigned int *px, int stride, int ancho, int alto,
              int x, int y, const char *s, unsigned int color) {
    int n;
    if (s == 0) {
        return x;
    }
    n = 0;
    while (s[n] != 0) {
        n = n + 1;
    }
    return bmo_texto_n(px, stride, ancho, alto, x, y, s, n, color);
}

#endif /* BMO_FUENTE_H */
