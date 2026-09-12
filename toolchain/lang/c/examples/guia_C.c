/* guia_C.c -- la guia de BMO-X, y el texto NO esta en el programa.
 *
 * == Por que existe ==
 *
 * Lo pidio el dueno despues de una pantalla azul: *"puedes crear un icono como
 * Guia.bex, ese mismo es texto que genera, es para guiar"*. Un sistema que no
 * se explica a si mismo obliga a tener a alguien al lado, y eso es justo lo que
 * no escala.
 *
 * == ** Y ES LA IDEA DEL `.datex`, HECHA COMO EL SISTEMA YA LA SOPORTA ==
 *
 * El dueno llevaba tres conversaciones dandole vueltas a un formato de DATOS
 * separado del programa. Esto es eso, sin formato nuevo: **el texto viaja como
 * un recurso DENTRO del `.bex`**, en la seccion `Resources` que `bmo-pack` ya
 * escribe y que `paquete.h` ya lee en ejecucion.
 *
 * ```text
 *    el programa      guia_C.c, y no sabe ni una palabra de lo que dice
 *    el texto         guia.txt, un fichero de verdad en el repo
 *    se junta         al empaquetar, y viajan como UN fichero
 * ```
 *
 * Para cambiar la guia se edita `guia.txt` y se vuelve a empacar. **El programa
 * no se toca, y no hay que recompilarlo.** Eso es lo que un `.datex` iba a dar,
 * y cuesta cero formatos nuevos.
 *
 * ** Y no ocupa RAM hasta que se pide: el cargador SALTA la seccion de
 * recursos. Un `.bex` con su guia dentro no es un `.bex` mas gordo en memoria.
 *
 * == Lo que hace ==
 *
 * ```text
 *    lee su propio paquete    sin escribir ninguna ruta -- `paquete_mio`
 *    lo parte en lineas       y las cuenta al arrancar, una sola vez
 *    los titulos en color     una linea que empieza por `#` es un titulo
 *    se recorre               flechas, RePag, AvPag, Inicio, Fin
 *    barra de posicion        a la derecha, para saber cuanto queda
 * ```
 *
 * == Los vatios, otra vez ==
 *
 * Un lector de texto quieto no tiene NADA que repintar, y aqui no se repinta:
 * el dibujo cuelga de `repinta`, que solo se enciende con una tecla, un clic o
 * al volver a verse (R-APP8). Una guia abierta y quieta cuesta un despertar
 * cada 16 ms para mirar un buzon vacio.
 *
 * [!] Sin tildes dentro de las cadenas del PROGRAMA -- ver `leer_C.c`. El TEXTO
 * de la guia es otra cosa: son bytes de un recurso, no literales, asi que ahi
 * no se aplica (aunque hoy `guia.txt` tambien sea ASCII, porque la fuente
 * dibuja Latin-1 y el fichero se escribe en UTF-8).
 */

/* *** EL MONTON, DECLARADO -- y sin esto no hay ventana. (2026-09-12)
 *
 * La superficie sale del MONTON, y el monton de serie es **1 MiB**. Esta imagen
 * pide 720x520x4 = 1.497.600 bytes, asi que con el de serie `malloc` devuelve 0,
 * `bmo_superficie_crear_con_buzon` contesta 0 y el programa se va diciendo
 * "hace falta el escritorio" -- **culpando al compositor de un fallo suyo**.
 *
 * Se declara ANTES del `#include`, que es como `<stdlib.h>` lo lee.
 * `raycaster_C.c` lo lleva escrito desde siempre con este mismo motivo; yo no
 * lo copie, y por eso `guia.bex` no abrio ni una vez.
 */
#define BMO_MONTON_BYTES (4 * 1024 * 1024)
#include <stdlib.h>
#include <bmo/bmo.h>
#include <bmo/paquete.h>
#include <bmo/superficie.h>
#include <bmo/entrada.h>
#include <bmo/fuente.h>

#define VEN_ANCHO 720
#define VEN_ALTO 520

#define BARRA 30
#define PIE 22
#define MARGEN 16

#define TY (BARRA + 8)
#define FILAS ((VEN_ALTO - PIE - 8 - TY) / BMO_FUENTE_ALTO)

/* La guia entera, en bytes, y cuantas lineas se indexan. Ocho KiB es mucho mas
 * de lo que mide `guia.txt` y sigue siendo menos que un icono a 32x32. */
#define TOPE 8192
#define LINEAS_MAX 400

#define C_FONDO 0x001B1F24
#define C_BARRA 0x00242A31
#define C_LINEA 0x00343B44
#define C_TEXTO 0x00D8E0EA
#define C_FLOJO 0x00768390
#define C_TITULO 0x0058A6FF
#define C_MALO 0x00E35C5C

static unsigned int *g_px;
static char *g_txt;
static int g_n;
/* El indice: donde empieza cada linea y cuanto mide. Se hace UNA vez al
 * arrancar -- recorrer el texto en cada fotograma para encontrar la linea 120
 * seria pagar por lo mismo sesenta veces por segundo. */
static int g_ini[LINEAS_MAX];
static int g_largo[LINEAS_MAX];
static int g_lineas;
static int g_prim;

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

/* Partir el texto en lineas, una sola vez.
 *
 * Se tragan los `\r` para que un fichero guardado en Windows no pinte un hueco
 * al final de cada linea: el fichero viene de un repo que vive en Windows y la
 * fuente no tiene glifo para el byte 13. */
static void indexar(void) {
    int i;
    int ini;
    int fin;
    g_lineas = 0;
    ini = 0;
    i = 0;
    while (i <= g_n) {
        if (i == g_n || g_txt[i] == 10) {
            fin = i;
            if (fin > ini && g_txt[fin - 1] == 13) {
                fin = fin - 1;
            }
            if (g_lineas < LINEAS_MAX) {
                g_ini[g_lineas] = ini;
                g_largo[g_lineas] = fin - ini;
                g_lineas = g_lineas + 1;
            }
            ini = i + 1;
        }
        i = i + 1;
    }
    if (g_lineas == 0) {
        g_lineas = 1;
        g_ini[0] = 0;
        g_largo[0] = 0;
    }
}

static void pinta(void) {
    int f;
    int li;
    int alto;
    int y;
    int barra;
    unsigned int color;

    relleno(0, 0, VEN_ANCHO, VEN_ALTO, C_FONDO);
    relleno(0, 0, VEN_ANCHO, BARRA, C_BARRA);
    relleno(0, BARRA - 1, VEN_ANCHO, 1, C_LINEA);
    relleno(0, VEN_ALTO - PIE, VEN_ANCHO, PIE, C_BARRA);
    relleno(0, VEN_ALTO - PIE, VEN_ANCHO, 1, C_LINEA);
    texto(MARGEN, 7, "Guia de BMO-X", C_TITULO);
    texto(VEN_ANCHO - MARGEN - bmo_texto_ancho("flechas / RePag / AvPag / Inicio / Fin"),
          7, "flechas / RePag / AvPag / Inicio / Fin", C_FLOJO);

    f = 0;
    while (f < FILAS) {
        li = g_prim + f;
        if (li >= g_lineas) break;
        if (g_largo[li] > 0) {
            /* Un titulo es una linea que empieza por `#`. Se pinta en el color
             * del acento y SIN su almohadilla: la marca es para el fichero, no
             * para quien lee. */
            color = C_TEXTO;
            if (g_txt[g_ini[li]] == '#') {
                bmo_texto_n(g_px, VEN_ANCHO, VEN_ANCHO, VEN_ALTO, MARGEN,
                            TY + f * BMO_FUENTE_ALTO, g_txt + g_ini[li] + 2,
                            g_largo[li] - 2, C_TITULO);
            } else {
                bmo_texto_n(g_px, VEN_ANCHO, VEN_ANCHO, VEN_ALTO, MARGEN,
                            TY + f * BMO_FUENTE_ALTO, g_txt + g_ini[li],
                            g_largo[li], color);
            }
        }
        f = f + 1;
    }

    /* La barra de posicion: cuanto de la guia estas viendo, y donde. Sin esto
     * no hay forma de saber si quedan dos lineas o doscientas. */
    if (g_lineas > FILAS) {
        alto = FILAS * (VEN_ALTO - PIE - TY) / g_lineas;
        if (alto < 12) alto = 12;
        y = TY + g_prim * (VEN_ALTO - PIE - TY - alto) / (g_lineas - FILAS);
        barra = VEN_ANCHO - 8;
        relleno(barra, TY, 4, VEN_ALTO - PIE - TY, C_BARRA);
        relleno(barra, y, 4, alto, C_LINEA);
    }
}

/* Devuelve 1 si hay que repintar. */
static int mover(int a) {
    int antes;
    antes = g_prim;
    g_prim = a;
    if (g_prim > g_lineas - FILAS) g_prim = g_lineas - FILAS;
    if (g_prim < 0) g_prim = 0;
    if (g_prim == antes) return 0;
    return 1;
}

int main() {
    BMO_SUPERFICIE *sup;
    PAQUETE *p;
    unsigned long long n;
    unsigned long long ev;
    int i;
    int c;
    int repinta;
    int se_ve;
    int antes;

    sup = bmo_superficie_crear_con_buzon(VEN_ANCHO, VEN_ALTO, 32);
    if (sup == 0) {
        printf("guia: hace falta el escritorio (nadie compone)\n");
        return 1;
    }
    g_px = bmo_superficie_pixeles(sup);
    g_txt = (char *)malloc(TOPE);
    if (g_txt == 0) {
        printf("guia: sin memoria\n");
        return 1;
    }

    /* Mi propio paquete, sin escribir ninguna ruta. */
    g_n = 0;
    p = paquete_mio();
    if (p != 0) {
        n = paquete_leer(p, "guia.txt", g_txt, TOPE);
        g_n = (int)n;
    }
    printf("guia: %d bytes de texto desde mi propio paquete\n", g_n);

    if (g_n == 0) {
        /* Se dice lo que pasa, en la ventana y no solo en la consola: quien
         * abre una guia vacia merece saber que el fallo es del paquete y no
         * suyo. */
        relleno(0, 0, VEN_ANCHO, VEN_ALTO, C_FONDO);
        texto(MARGEN, 40, "este .bex no trae el recurso 'guia.txt'", C_MALO);
        texto(MARGEN, 64, "se mete al empaquetar: ver $cRecursos en el build",
              C_FLOJO);
        bmo_superficie_lista(sup);
    } else {
        indexar();
        printf("guia: %d lineas, %d caben en la ventana\n", g_lineas, FILAS);
        pinta();
        bmo_superficie_lista(sup);
    }

    g_prim = 0;
    repinta = 0;
    antes = 1;
    for (;;) {
        i = 0;
        while (i < 32) {
            i = i + 1;
            ev = bmo_superficie_evento(sup);
            if ((ev & BMO_EVENTO_HAY) == 0) break;
            /* El raton no hace nada aqui todavia, pero se DRENA: un buzon que
             * se llena hace que el DIRECTOR descarte, y entonces las teclas
             * tampoco llegan. */
            if (bmo_sup_es_raton(ev) == 1) continue;
            if (bmo_sup_es_caracter(ev) == 0) continue;
            if (g_n == 0) continue;
            c = bmo_sup_caracter(ev);
            if (c == BMO_TECLA_ABAJO) {
                if (mover(g_prim + 1) == 1) repinta = 1;
            } else if (c == BMO_TECLA_ARRIBA) {
                if (mover(g_prim - 1) == 1) repinta = 1;
            } else if (c == BMO_TECLA_AVPAG || c == 32) {
                if (mover(g_prim + FILAS - 1) == 1) repinta = 1;
            } else if (c == BMO_TECLA_REPAG) {
                if (mover(g_prim - FILAS + 1) == 1) repinta = 1;
            } else if (c == BMO_TECLA_INICIO) {
                if (mover(0) == 1) repinta = 1;
            } else if (c == BMO_TECLA_FIN) {
                if (mover(g_lineas) == 1) repinta = 1;
            }
        }

        se_ve = bmo_superficie_se_ve(sup);
        if (se_ve == 1 && antes == 0) repinta = 1;
        antes = se_ve;

        if (se_ve == 1 && repinta == 1 && g_n > 0) {
            pinta();
            bmo_superficie_lista(sup);
            repinta = 0;
        }
        bmo_dormir(16000000);
    }
}
