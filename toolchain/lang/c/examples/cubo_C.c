/* cubo_C.c -- un cubo girando, y SIN UN SOLO TRIANGULO.
 *
 * == La pregunta del dueno, que es mejor que el cubo ==
 *
 * *"el triangulo esta muy quemado y me gustaria usar otro estilo, en vez de
 * tipico poligono otro elemento para renderizar"*.
 *
 * Y para ESTA figura no solo se puede: **sale mejor**. Aqui el elemento que se
 * dibuja es LA CAJA, entera, y no doce triangulos que la aparentan.
 *
 * == Por que el triangulo es el estandar, para no tirarlo por gusto ==
 *
 * No es moda. Son cuatro propiedades que sostienen todo lo demas:
 *
 * ```text
 *    siempre PLANO       tres puntos definen un plano. Un cuadrilatero de
 *                        cuatro puntos puede estar alabeado
 *    siempre CONVEXO     y por eso cada fila de pantalla da UN tramo -- que es
 *                        justo lo que explota `bmo-dibujo::triangulo`
 *    interpolacion       las baricentricas son unicas y afines: color,
 *    UNICA               textura y profundidad con la misma formula
 *    es el minimo        cualquier otro poligono se parte en triangulos
 * ```
 *
 * ** Y por eso `bmo-dibujo` se queda como esta: es el ORACULO con el que se
 * juzgara la GPU el dia que haya driver, y **una GPU habla triangulos**. Lo que
 * NO es cierto es que una app tenga que hablarlos. Son dos cosas distintas y
 * este fichero es la prueba.
 *
 * == Lo que se hace aqui: TRAZAR LA CAJA, exacto y sin mallas ==
 *
 * El metodo de las LAMINAS (slabs). Una caja son tres pares de planos; para
 * cada eje se calcula por donde entra y por donde sale el rayo, y la caja
 * entera se resuelve con un maximo y un minimo:
 *
 * ```text
 *    por eje:  t1 = (-b - o) / d      t2 = (+b - o) / d
 *    entra  =  max de los minimos     sale = min de los maximos
 *    hay golpe si  sale >= entra  y  sale >= 0
 * ```
 *
 * Lo que eso compra, y no es poco:
 *
 * ```text
 *    EXACTO             el borde es el borde a cualquier zoom. No hay
 *                       silueta poligonal que se note al acercarse
 *    sin vertices       girar es girar EL RAYO, no ocho puntos y una matriz
 *    sin recorte        no hay poligono que cortar contra la pantalla
 *    la NORMAL sale     el eje que dio el `entra` ES la cara golpeada, con su
 *    de la cuenta       signo. No hace falta calcularla aparte
 *    sin raiz cuadrada  solo tres divisiones por pixel
 * ```
 *
 * [!] Y su precio, dicho: **cuesta por PIXEL y no por vertice.** Un cubo de
 * doce triangulos cuesta doce triangulos aunque llene la pantalla; esto cuesta
 * lo que ocupa. Para una figura es la eleccion correcta; para un mundo de mil
 * objetos, la equivocada. Por eso esto es OTRA herramienta y no una sustituta.
 *
 * == Y por eso trae su propio metro ==
 *
 * `[cubo]` dice los microsegundos del fotograma, igual que hace DOOM. No se
 * predice lo que cuesta: se mide. Esta casa acaba de aprender --el 12-09-- lo
 * que pasa cuando alguien repite una cifra sin comprobarla.
 *
 * [!] Sin tildes dentro de las cadenas. Ver `leer_C.c`.
 */

/* *** EL MONTON, DECLARADO -- y sin esto no habria ventana. (2026-09-12)
 *
 * La superficie sale del MONTON, y el monton de serie es **1 MiB**. Esta imagen
 * pide 360x360x4 = 518.400 bytes, que CABEN en el de serie -- pero se declara igual, porque
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

/* Cuadrada, y modesta a proposito: esto cuesta por pixel. El numero real lo
 * dira el `[cubo]` en el metal, y de ahi sale si sube o baja. */
#define VEN 360
#define UNO 65536

#define C_FONDO 0x00121519
#define C_CIELO 0x001B2129
#define C_TEXTO 0x00E6EDF7
#define C_FLOJO 0x00768390

static unsigned int *g_px;
static unsigned char *g_tex;
static int g_tex_n;
static int g_tex_lado;

/* Multiplicar dos 16.16 sin perder los bits de en medio. El paso por 64 bits
 * no es prudencia: `a*b` de dos 16.16 tiene 32 bits de parte decimal y
 * desborda un entero de 32 en cuanto los operandos pasan de 1.0. Es la misma
 * pareja que `raycaster_C.c` ya probo en el Ryzen. */
static int fmul(int a, int b) {
    long long p;
    p = (long long)a * (long long)b;
    return (int)(p >> 16);
}

static int fdiv(int a, int b) {
    long long p;
    if (b == 0) return 0x7FFFFFFF;
    p = ((long long)a) << 16;
    return (int)(p / (long long)b);
}

/* -- EL SENO, por tabla -------------------------------------------------
 *
 * Un cuarto de onda, 65 entradas de 16.16, y el resto por simetria. La vuelta
 * entera son 256 pasos.
 *
 * ** Tabla y no serie de Taylor, y no por velocidad: **una tabla no deriva**.
 * Girar acumulando una rotacion pequena --que es lo obvio-- encoge o agranda el
 * vector poco a poco, y el cubo se deforma despues de unas cuantas vueltas sin
 * que nada avise. Con un indice no hay estado que se estropee. */
static int TSEN[65] = {
    0, 1608, 3216, 4821, 6424, 8022, 9616, 11204,
    12785, 14359, 15924, 17479, 19024, 20557, 22078, 23586,
    25080, 26558, 28020, 29466, 30893, 32303, 33692, 35062,
    36410, 37736, 39040, 40320, 41576, 42806, 44011, 45190,
    46341, 47464, 48559, 49624, 50660, 51665, 52639, 53581,
    54491, 55368, 56212, 57022, 57798, 58538, 59244, 59914,
    60547, 61145, 61705, 62228, 62714, 63162, 63572, 63944,
    64277, 64571, 64827, 65043, 65220, 65358, 65457, 65516,
    65536
};

static int sen(int i) {
    i = i & 255;
    if (i <= 64) return TSEN[i];
    if (i <= 128) return TSEN[128 - i];
    if (i <= 192) return -TSEN[i - 128];
    return -TSEN[256 - i];
}

static int cose(int i) {
    return sen(i + 64);
}

/* -- DIBUJO ------------------------------------------------------------- */

static void relleno(int x, int y, int w, int h, unsigned int c) {
    int fy;
    int fx;
    fy = y;
    if (fy < 0) fy = 0;
    while (fy < y + h) {
        if (fy >= VEN) break;
        fx = x;
        if (fx < 0) fx = 0;
        while (fx < x + w) {
            if (fx >= VEN) break;
            g_px[fy * VEN + fx] = c;
            fx = fx + 1;
        }
        fy = fy + 1;
    }
}

static void texto(int x, int y, const char *s, unsigned int c) {
    bmo_texto(g_px, VEN, VEN, VEN, x, y, s, c);
}

/* Oscurecer un color a `luz` (16.16, 0..1). Entera y por canal: aqui no hay
 * coma flotante, y no hace falta. */
static unsigned int sombra(unsigned int c, int luz) {
    int r;
    int g;
    int b;
    if (luz < 0) luz = 0;
    if (luz > UNO) luz = UNO;
    r = (int)((c >> 16) & 0xFF);
    g = (int)((c >> 8) & 0xFF);
    b = (int)(c & 0xFF);
    r = (r * luz) >> 16;
    g = (g * luz) >> 16;
    b = (b * luz) >> 16;
    return ((unsigned int)r << 16) | ((unsigned int)g << 8) | (unsigned int)b;
}

/* Un texel de la textura que este `.bex` lleva dentro, por coordenadas 0..1 en
 * 16.16. Si no hay textura contesta 0 y quien llama pone su color. */
static unsigned int texel(int u, int v) {
    int tx;
    int ty;
    unsigned int c;
    if (g_tex_lado <= 0) return 0;
    if (u < 0) u = 0;
    if (v < 0) v = 0;
    if (u >= UNO) u = UNO - 1;
    if (v >= UNO) v = UNO - 1;
    tx = (u * g_tex_lado) >> 16;
    ty = (v * g_tex_lado) >> 16;
    c = bmo_imagen_pixel(g_tex, ty * g_tex_lado + tx);
    /* El alfa a cero es el hueco del icono: ahi se deja ver la cara. */
    if ((c >> 24) == 0) return 0;
    return c & 0x00FFFFFF;
}

/* -- EL CUBO, trazado ---------------------------------------------------- */

/* Medio lado, en 16.16. */
#define LADO 45000

/* Un fotograma entero. `ang` es el giro, en pasos de 1/256 de vuelta. */
static void pinta(int ang)
{
    int sy;
    int cy;
    int sx;
    int cx;
    int px;
    int py;
    int u;
    int v;
    /* Origen del rayo, en el espacio del CUBO. Una por linea: BMO C no
     * digiere `int a, b, c;` -- lo parsea como una declaracion y dos
     * expresiones sueltas, y el error sale MUCHO despues, al asignarles. */
    int ox;
    int oy;
    int oz;
    int dx;
    int dy;
    int dz;
    int rx;
    int ry;
    int rz;
    int t1;
    int t2;
    int lo;
    int hi;
    int entra;
    int sale;
    int eje;             /* 0=X 1=Y 2=Z, la cara golpeada */
    int signo;
    int hx;
    int hy;
    int hz;
    int tu;
    int tv;
    int luz;
    unsigned int color;
    unsigned int tc;
    int fondo;

    sy = sen(ang);
    cy = cose(ang);
    /* El segundo eje gira a otro ritmo: si los dos van igual, el cubo parece
     * girar sobre una sola diagonal y no se le ven las seis caras. */
    sx = sen(ang * 3 / 7 + 20);
    cx = cose(ang * 3 / 7 + 20);

    py = 0;
    while (py < VEN) {
        /* Un degradado vertical de fondo, calculado una vez por fila. */
        fondo = (py * 255) / VEN;
        px = 0;
        while (px < VEN) {
            /* El rayo, en el espacio del MUNDO. La camara mira al +Z desde
             * -2.6, y el 1.6 es la distancia focal: mas alto, menos angulo. */
            u = ((px * 2 - VEN) * UNO) / VEN;
            v = ((py * 2 - VEN) * UNO) / VEN;
            dx = u;
            dy = v;
            dz = 105000;
            ox = 0;
            oy = 0;
            oz = -170000;

            /* ** Y AQUI ESTA EL TRUCO ENTERO: no se gira el cubo, se gira EL
             * RAYO al reves. Asi la caja sigue alineada con los ejes y las
             * laminas son tres restas. Girar el cubo obligaria a girar seis
             * planos por pixel. */
            /* -ang alrededor de Y */
            rx = fmul(dx, cy) - fmul(dz, sy);
            rz = fmul(dx, sy) + fmul(dz, cy);
            dx = rx;
            dz = rz;
            rx = fmul(ox, cy) - fmul(oz, sy);
            rz = fmul(ox, sy) + fmul(oz, cy);
            ox = rx;
            oz = rz;
            /* -ang alrededor de X */
            ry = fmul(dy, cx) - fmul(dz, sx);
            rz = fmul(dy, sx) + fmul(dz, cx);
            dy = ry;
            dz = rz;
            ry = fmul(oy, cx) - fmul(oz, sx);
            rz = fmul(oy, sx) + fmul(oz, cx);
            oy = ry;
            oz = rz;

            /* -- Las tres laminas -- */
            entra = -0x40000000;
            sale = 0x40000000;
            eje = 0;
            signo = 1;

            /* X */
            if (dx != 0) {
                t1 = fdiv(-LADO - ox, dx);
                t2 = fdiv(LADO - ox, dx);
                lo = t1;
                hi = t2;
                if (lo > hi) {
                    lo = t2;
                    hi = t1;
                }
                if (lo > entra) {
                    entra = lo;
                    eje = 0;
                    if (dx > 0) { signo = -1; } else { signo = 1; }
                }
                if (hi < sale) sale = hi;
            } else if (ox < -LADO || ox > LADO) {
                sale = -1;
            }
            /* Y */
            if (dy != 0) {
                t1 = fdiv(-LADO - oy, dy);
                t2 = fdiv(LADO - oy, dy);
                lo = t1;
                hi = t2;
                if (lo > hi) {
                    lo = t2;
                    hi = t1;
                }
                if (lo > entra) {
                    entra = lo;
                    eje = 1;
                    if (dy > 0) { signo = -1; } else { signo = 1; }
                }
                if (hi < sale) sale = hi;
            } else if (oy < -LADO || oy > LADO) {
                sale = -1;
            }
            /* Z */
            if (dz != 0) {
                t1 = fdiv(-LADO - oz, dz);
                t2 = fdiv(LADO - oz, dz);
                lo = t1;
                hi = t2;
                if (lo > hi) {
                    lo = t2;
                    hi = t1;
                }
                if (lo > entra) {
                    entra = lo;
                    eje = 2;
                    if (dz > 0) { signo = -1; } else { signo = 1; }
                }
                if (hi < sale) sale = hi;
            } else if (oz < -LADO || oz > LADO) {
                sale = -1;
            }

            if (sale < entra || sale < 0) {
                /* Sin golpe: el fondo. */
                g_px[py * VEN + px] = sombra(C_CIELO, UNO - (fondo * 180));
                px = px + 1;
                continue;
            }

            /* El punto de entrada, en el espacio del cubo. */
            hx = ox + fmul(dx, entra);
            hy = oy + fmul(dy, entra);
            hz = oz + fmul(dz, entra);

            /* ** LA CARA YA LA SABEMOS: es `eje`, y `signo` dice cual de las
             * dos. La normal no se calcula, SALE de la interseccion -- eso es
             * lo que una malla no te da gratis. Las dos coordenadas que NO son
             * del eje son la textura. */
            if (eje == 0) {
                tu = fdiv(hz + LADO, LADO * 2);
                tv = fdiv(hy + LADO, LADO * 2);
                color = 0x009AA4AE;
                luz = 45000;
            } else if (eje == 1) {
                tu = fdiv(hx + LADO, LADO * 2);
                tv = fdiv(hz + LADO, LADO * 2);
                color = 0x00C2CBD4;
                /* Sin ternario: BMO C no lo digiere, y un `if` se lee igual.
                 * La cara de arriba recibe mas luz que la de abajo. */
                if (signo > 0) {
                    luz = 66000;
                } else {
                    luz = 30000;
                }
            } else {
                tu = fdiv(hx + LADO, LADO * 2);
                tv = fdiv(hy + LADO, LADO * 2);
                color = 0x00AEB8C2;
                luz = 56000;
            }

            /* La textura va en las caras de Z y en la de arriba: las mismas
             * tres que se ven de un vistazo. */
            if (eje != 0) {
                tc = texel(tu, tv);
                if (tc != 0) color = tc;
            }

            g_px[py * VEN + px] = sombra(color, luz);
            px = px + 1;
        }
        py = py + 1;
    }
}

int main() {
    BMO_SUPERFICIE *sup;
    PAQUETE *p;
    unsigned long long ev;
    unsigned long long t0;
    unsigned long long hz;
    unsigned long long acum;
    int marcos;
    int ang;
    int i;
    int se_ve;
    int antes;

    sup = bmo_superficie_crear_con_buzon(VEN, VEN, 32);
    if (sup == 0) {
        printf("cubo: hace falta el escritorio (nadie compone)\n");
        return 1;
    }
    g_px = bmo_superficie_pixeles(sup);

    /* La textura sale de MI PROPIO paquete: el mismo icono que el escritorio
     * pinta en la rejilla. El dato es uno. */
    g_tex = (unsigned char *)malloc(4096);
    g_tex_n = 0;
    g_tex_lado = 0;
    if (g_tex != 0) {
        p = paquete_mio();
        if (p != 0) {
            g_tex_n = (int)paquete_leer(p, "icono", g_tex, 4096);
        }
        if (bmo_imagen_completa(g_tex, g_tex_n) == 1) {
            g_tex_lado = bmo_imagen_ancho(g_tex, g_tex_n);
            if (bmo_imagen_alto(g_tex, g_tex_n) != g_tex_lado) {
                /* Esta demo mapea con un solo lado: una textura no cuadrada
                 * saldria estirada y sin decir por que. Mejor sin textura. */
                g_tex_lado = 0;
            }
        }
    }
    printf("cubo: textura de %d bytes, lado %d\n", g_tex_n, g_tex_lado);

    /* El reloj de referencia, para pasar ticks a microsegundos. `INFO_TSC_HZ`
     * es el reloj INVARIANTE: no cambia aunque el nucleo suba o baje. */
    hz = bmo_valor(BMO_TAREA_ACTUAL, BMO_OP_INFO, BMO_INFO_TSC_HZ, 0, 0);
    if (hz == 0) hz = 1;
    acum = 0;
    marcos = 0;
    ang = 0;
    antes = 1;

    for (;;) {
        i = 0;
        while (i < 32) {
            i = i + 1;
            ev = bmo_superficie_evento(sup);
            if ((ev & BMO_EVENTO_HAY) == 0) break;
        }

        /* R-APP8: si no se ve, no se pinta. Un cubo girando SI tiene que
         * repintar cada fotograma --eso es lo que hace un cubo girando-- y por
         * eso es honesto que se pare del todo cuando nadie lo mira. */
        se_ve = bmo_superficie_se_ve(sup);
        if (se_ve == 1) {
            if (antes == 0) marcos = 0;
            t0 = __rdtsc();
            pinta(ang);
            acum = acum + (__rdtsc() - t0);
            marcos = marcos + 1;
            ang = (ang + 1) & 255;

            /* El metro, una vez por segundo de fotogramas. */
            if (marcos >= 60) {
                bmo_superficie_lista(sup);
                printf("[cubo] %dx%d  fotograma %d us\n", VEN, VEN,
                       (int)(acum / (unsigned long long)marcos
                             * 1000000 / hz));
                acum = 0;
                marcos = 0;
            } else {
                bmo_superficie_lista(sup);
            }
        }
        antes = se_ve;
        bmo_dormir(16000000);
    }
}
