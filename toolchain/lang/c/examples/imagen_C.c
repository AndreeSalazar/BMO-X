/* imagen_C.c -- una app saca su PROPIA imagen de dentro de si misma y la pinta.
 *
 * == La pregunta que contesta ==
 *
 * Un programador que llega de fuera pregunta lo primero de todo: *"y si quiero
 * poner imagenes?"*. Hasta hoy la respuesta era incomoda -- una app podia LLEVAR
 * una imagen dentro de su `.bex` desde que existe `bmo-pack`, y no sabia
 * pintarla: el unico que descifraba un `BICO` era el DIRECTOR, y solo a 16x16.
 *
 * Esto junta las tres piezas que ya existian por separado y nunca se habian
 * usado juntas:
 *
 * ```text
 *    `paquete.h`     abrir MI PROPIA imagen sin escribir ninguna ruta
 *    `imagen.h`      descifrar el `BICO` y pintarlo, a escala entera
 *    `superficie.h`  en una ventana que compone el DIRECTOR
 * ```
 *
 * == Lo que hay que mirar, en orden ==
 *
 *   1. Soy un paquete?          `paquete_mio`, y sin decir donde estoy
 *   2. Tengo el recurso?        `paquete_leer("icono", ...)`
 *   3. Es un BICO, y entero?    la cabecera promete WxH: estan todos?
 *   4. Se ve a 1x, 2x y 4x?     la misma imagen, tres veces
 *   5. ** EL ALFA ES UN BIT     sobre un tablero de cuadros: los pixeles
 *                               transparentes tienen que dejar ver el tablero,
 *                               y si no, se vera el cuadro negro del icono
 *
 * El paso 5 es el que no se puede fingir. Una imagen con fondo opaco se ve
 * perfecta encima de un fondo liso y se delata sobre un tablero.
 *
 * == Y el icono es el SUYO ==
 *
 * El mismo recurso `icono` que el escritorio pinta en la rejilla. No hay una
 * segunda copia para el programa: **el dato es uno y lo leen los dos**, el
 * DIRECTOR desde fuera y esto desde dentro. Es el mismo fichero que el
 * escritorio lista.
 *
 * [!] Sin tildes dentro de las cadenas -- ver la cabecera de `leer_C.c`, que
 * lleva los numeros de lo que cuesta un acento en un literal.
 */

/* *** EL MONTON, DECLARADO -- y sin esto no habria ventana. (2026-09-12)
 *
 * La superficie sale del MONTON, y el monton de serie es **1 MiB**. Esta imagen
 * pide 520x300x4 = 624.000 bytes, que CABEN en el de serie -- pero se declara igual, porque
 * el margen aqui no cuesta nada y quedarse justo si: la textura y los buffers
 * salen del mismo sitio.
 *
 * Se declara ANTES del `#include`, que es como `<stdlib.h>` lo lee.
 */
#define BMO_MONTON_BYTES (2 * 1024 * 1024)
#include <stdlib.h>
#include <bmo/bmo.h>
#include <bmo/paquete.h>
#include <bmo/superficie.h>
#include <bmo/entrada.h>
#include <bmo/fuente.h>
#include <bmo/imagen.h>

#define VEN_ANCHO 520
#define VEN_ALTO 300

/* 16x16 en crudo son 1032 bytes con cabecera. 4 KiB deja sitio de sobra para
 * una imagen mas grande sin tocar esto. */
#define TOPE 4096

#define C_FONDO 0x001B1F24
#define C_CUADRO 0x00262C34
#define C_TEXTO 0x00E6EDF7
#define C_FLOJO 0x00768390
#define C_MALO 0x00E35C5C

static unsigned int *g_px;

static void relleno(int x, int y, int w, int h, unsigned int c) {
    int fy;
    int fx;
    fy = y;
    if (fy < 0) fy = 0;
    while (fy < y + h) {
        if (fy >= VEN_ALTO) break;
        fx = x;
        if (fx < 0) fx = 0;
        while (fx < x + w) {
            if (fx >= VEN_ANCHO) break;
            g_px[fy * VEN_ANCHO + fx] = c;
            fx = fx + 1;
        }
        fy = fy + 1;
    }
}

static void texto(int x, int y, const char *s, unsigned int c) {
    bmo_texto(g_px, VEN_ANCHO, VEN_ANCHO, VEN_ALTO, x, y, s, c);
}

/* ** EL TABLERO, que es el instrumento de esta prueba y no decoracion.
 *
 * Cuadros de ocho pixeles. Si el alfa no se respetara, encima de esto se veria
 * el rectangulo opaco del icono tapando los cuadros -- que es exactamente el
 * fallo que un fondo liso esconde. */
static void tablero(int x, int y, int w, int h) {
    int fy;
    int fx;
    fy = 0;
    while (fy < h) {
        fx = 0;
        while (fx < w) {
            if (((fx / 8) + (fy / 8)) % 2 == 0) {
                relleno(x + fx, y + fy, 8, 8, C_CUADRO);
            }
            fx = fx + 8;
        }
        fy = fy + 8;
    }
}

int main() {
    BMO_SUPERFICIE *sup;
    PAQUETE *p;
    unsigned char *buf;
    unsigned long long n;
    unsigned long long ev;
    int w;
    int h;
    int i;
    int pintado;

    sup = bmo_superficie_crear_con_buzon(VEN_ANCHO, VEN_ALTO, 16);
    if (sup == 0) {
        printf("imagen: hace falta el escritorio (nadie compone)\n");
        return 1;
    }
    g_px = bmo_superficie_pixeles(sup);

    /* 1 - Mi propia imagen, y no se escribe ninguna ruta: el kernel se acuerda
     * de por donde entre. Es tener por DERECHO y no pedir por NOMBRE. */
    p = paquete_mio();
    /* 2 - El destino sale de `malloc` porque por ahi lee el kernel. */
    buf = (unsigned char *)malloc(TOPE);
    if (buf == 0) {
        printf("imagen: sin memoria\n");
        return 1;
    }
    n = 0;
    if (p != 0) {
        n = paquete_leer(p, "icono", buf, TOPE);
    }

    /* 3 - Los numeros, dichos en la consola antes de pintar nada: si la ventana
     * saliera en blanco, esto ya dice por que. */
    w = bmo_imagen_ancho(buf, (int)n);
    h = bmo_imagen_alto(buf, (int)n);
    printf("imagen: recurso de %d bytes, BICO de %dx%d, completa=%d\n",
           (int)n, w, h, bmo_imagen_completa(buf, (int)n));

    relleno(0, 0, VEN_ANCHO, VEN_ALTO, C_FONDO);
    texto(12, 10, "mi imagen, desde DENTRO de mi propio .bex", C_TEXTO);

    if (bmo_imagen_completa(buf, (int)n) == 0) {
        /* Se dice lo que falta, no "error": un recurso que no esta y un BICO
         * cortado son dos problemas distintos y se arreglan en sitios
         * distintos. */
        if (n == 0) {
            texto(12, 40, "no hay recurso 'icono' en este paquete", C_MALO);
        } else {
            texto(12, 40, "hay recurso, pero no es un BICO entero", C_MALO);
        }
    } else {
        /* 4 y 5 - La misma imagen tres veces, sobre el tablero. */
        texto(12, 40, "1x            2x                  4x", C_FLOJO);
        tablero(12, 60, 16, 16);
        tablero(60, 60, 32, 32);
        tablero(120, 60, 64, 64);
        pintado = bmo_imagen_pinta(g_px, VEN_ANCHO, VEN_ANCHO, VEN_ALTO,
                                   12, 60, buf, (int)n);
        bmo_imagen_pinta_escala(g_px, VEN_ANCHO, VEN_ANCHO, VEN_ALTO,
                                60, 60, buf, (int)n, 2);
        bmo_imagen_pinta_escala(g_px, VEN_ANCHO, VEN_ANCHO, VEN_ALTO,
                                120, 60, buf, (int)n, 4);
        texto(12, 150, "los cuadros de detras se tienen que VER a traves:",
              C_FLOJO);
        texto(12, 168, "el alfa es un bit, y el que esta a cero no se pinta",
              C_FLOJO);
        if (pintado == 1) {
            texto(12, 200, "y el recorte: esta de abajo cae fuera a proposito",
                  C_FLOJO);
            /* El recorte, ejercitado: media imagen fuera por la derecha y
             * media por abajo. Si esto escribiera fuera del bloque seria un
             * `#PF`, y por eso la prueba esta AQUI y no en un comentario. */
            bmo_imagen_pinta_escala(g_px, VEN_ANCHO, VEN_ANCHO, VEN_ALTO,
                                    VEN_ANCHO - 24, VEN_ALTO - 24, buf,
                                    (int)n, 2);
            bmo_imagen_pinta_escala(g_px, VEN_ANCHO, VEN_ANCHO, VEN_ALTO,
                                    -16, VEN_ALTO - 40, buf, (int)n, 2);
        }
    }
    bmo_superficie_lista(sup);

    /* Se queda quieta: no hay nada que animar, asi que no hay nada que repintar.
     * Drena el buzon para no dejarlo lleno --el DIRECTOR descarta si se llena--
     * y duerme. Es la regla de `EFICIENCIA_MAESTRO`: lo que no cambia no se
     * vuelve a pintar. Se cierra por el boton del marco. */
    for (;;) {
        i = 0;
        while (i < 16) {
            i = i + 1;
            ev = bmo_superficie_evento(sup);
            if ((ev & BMO_EVENTO_HAY) == 0) break;
        }
        bmo_dormir(100000000);
    }
}
