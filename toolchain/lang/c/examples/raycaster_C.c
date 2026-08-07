/* raycaster_C.c — 2.5D en BMO C, sobre la pantalla de verdad.
 *
 * ══ Para qué existe ══
 *
 * Es el ensayo general de DOOM, y está hecho para contestar UNA pregunta que
 * hoy nadie puede contestar: **¿puede un programa en C tomar la pantalla,
 * dibujar un fotograma, leer el teclado y repetirlo sesenta veces por segundo,
 * en el metal?**
 *
 * DOOM son ~35.000 líneas en cincuenta ficheros, y hoy chocaría con dos techos
 * del compilador que están medidos: la tabla de cadenas del IR (256 cadenas,
 * 4096 bytes) y una libc de ocho funciones sin E/S de ficheros. Este programa
 * **no toca ninguno de los dos**: cabe en una unidad de traducción, no abre
 * archivos y usa tres literales.
 *
 * Si esto corre, DOOM deja de ser "¿se puede?" y pasa a ser "cuánta libc
 * falta", que es una lista y no una pregunta.
 *
 * ══ Por qué NO hay un solo `float` ══
 *
 * Punto fijo 16.16, como el `fixed_t` de DOOM — y por el mismo motivo que
 * tuvieron ellos en 1993, no por nostalgia: la ruta de coma flotante de BMO C
 * es joven (el emulador estrenó SSE hace tres días) y un renderizador es el
 * peor sitio para estrenarla. Aquí un entero de 32 bits vale por un número con
 * dieciséis bits de parte decimal, y las multiplicaciones pasan por 64 bits
 * para no perder nada por el camino.
 *
 * ══ Por qué no hay tabla de senos ══
 *
 * No hace falta ni una. Se lleva un VECTOR de dirección y un VECTOR de plano de
 * cámara, y girar es multiplicar por dos constantes —el coseno y el seno de un
 * ángulo fijo—. Cero trigonometría en tiempo de ejecución, cero tablas
 * globales grandes que el compilador tenga que colocar.
 *
 * ══ Y por qué no hay corrección de ojo de pez ══
 *
 * Porque con el plano de cámara **no hace falta**: el rayo se define como
 * `dir + plano*x`, y entonces el parámetro `t` con el que se avanza YA es la
 * distancia perpendicular a la pantalla. La corrección clásica por coseno es lo
 * que se paga por trabajar con ángulos en vez de con vectores.
 */

#include <bmo/bmo.h>

/* Operaciones sobre el handle de pantalla (KIND_FRAMEBUFFER). */
#define FB_BASE   0x01
#define FB_DIMS   0x02   /* ancho<<32 | alto */
#define FB_STRIDE 0x03   /* stride<<32 | formato — stride va en PIXELES */

/* Operaciones sobre el handle de entrada (KIND_INPUT). */
#define ENT_TECLA 0x03   /* no bloquea: 0 = no hay nada */

#define UNO   65536      /* 1.0 en 16.16 */
#define MAPA_N 16

/* El mundo. Un solo literal, y por eso una sola entrada en la tabla de cadenas
 * del IR — que tiene 256 y conviene no gastarlas en decoración. */
/* Puntero al literal y no `char mapa[]`: BMO C todavía no deduce el tamaño de
 * un array desde su inicializador, y aquí no hace falta — se indexa igual. */
char *mapa =
    "1111111111111111"
    "1000000000000001"
    "1011110000111101"
    "1010000000000101"
    "1010111011101101"
    "1010001010001001"
    "1011101010111011"
    "1000101000100001"
    "1110101110101111"
    "1000100010100001"
    "1011111010111101"
    "1000001000100001"
    "1111101111101111"
    "1000001000000001"
    "1000001000000001"
    "1111111111111111";

int pared(int x, int y) {
    if (x < 0) return 1;
    if (y < 0) return 1;
    if (x >= MAPA_N) return 1;
    if (y >= MAPA_N) return 1;
    if (mapa[y * MAPA_N + x] == '1') return 1;
    return 0;
}

/* Multiplicar dos 16.16 sin perder los bits de en medio. El paso por 64 bits no
 * es prudencia: `a*b` de dos 16.16 tiene 32 bits de parte decimal y desborda un
 * entero de 32 en cuanto los operandos pasan de 1.0. */
int fmul(int a, int b) {
    long long p;
    p = (long long)a * (long long)b;
    return (int)(p >> 16);
}

int fdiv(int a, int b) {
    long long p;
    if (b == 0) return 0x7FFFFFFF;
    p = ((long long)a) << 16;
    return (int)(p / (long long)b);
}

int main() {
    unsigned long long pant;
    unsigned long long ent;
    unsigned long long base;
    unsigned long long dims;
    unsigned long long st;
    unsigned int *fb;
    int ancho;
    int alto;
    int stride;

    /* Posición y orientación, todo en 16.16. Se empieza mirando al este. */
    int posx; int posy;
    int dirx; int diry;
    int plax; int play;

    /* Girar 5 grados: cos = 0.99619, sen = 0.08716. Las dos únicas constantes
     * trigonométricas del programa. */
    int cosg; int seng;

    int x; int y;
    int camx;
    int rayx; int rayy;
    int t; int paso;
    int mx; int my;
    int golpe;
    int altura;
    int mitad;
    int y0; int y1;
    int color;
    int tecla;
    int nx; int ny;
    unsigned int *fila;
    int i;
    int vivo;

    pant = bmo_valor(BMO_TAREA_ACTUAL, BMO_OP_PANTALLA_RECLAMAR, 0, 0, 0);
    if (pant == 0) {
        printf("no me dieron la pantalla\n");
        return 1;
    }
    base = bmo_valor(pant, FB_BASE, 0, 0, 0);
    dims = bmo_valor(pant, FB_DIMS, 0, 0, 0);
    st = bmo_valor(pant, FB_STRIDE, 0, 0, 0);
    fb = (unsigned int *)base;
    ancho = (int)(dims >> 32);
    alto = (int)(dims & 0xFFFFFFFF);
    stride = (int)(st >> 32);

    ent = bmo_valor(BMO_TAREA_ACTUAL, BMO_OP_ENTRADA_RECLAMAR, 0, 0, 0);

    posx = 3 * UNO + 32768;
    posy = 3 * UNO + 32768;
    dirx = UNO;  diry = 0;
    plax = 0;    play = 43690;   /* 0.666 — el campo de visión de siempre */
    cosg = 65286;
    seng = 5712;

    vivo = 1;
    while (vivo == 1) {
        /* ── UNA COLUMNA, UN RAYO ─────────────────────────────────────── */
        x = 0;
        while (x < ancho) {
            /* camx va de -1.0 a +1.0 de un borde al otro de la pantalla. */
            camx = fdiv(2 * x, ancho) - UNO;
            rayx = dirx + fmul(plax, camx);
            rayy = diry + fmul(play, camx);

            /* Marchar el rayo. Un paso de 1/32 de casilla: suficiente para que
             * no se cuele por una esquina, y barato. Se para a 20 casillas —
             * más lejos no hay nada que enseñar y sí mucho que calcular. */
            t = 0;
            paso = 2048;
            golpe = 0;
            while (t < 20 * UNO) {
                t = t + paso;
                mx = (posx + fmul(rayx, t)) >> 16;
                my = (posy + fmul(rayy, t)) >> 16;
                if (pared(mx, my) == 1) {
                    golpe = 1;
                    t = 20 * UNO;
                }
            }

            if (golpe == 1) {
                /* `t` YA es la distancia perpendicular: ver la cabecera. */
                t = t - 20 * UNO;
                if (t < 2048) t = 2048;
                altura = fdiv(alto, t) >> 16;
            } else {
                altura = 0;
            }
            if (altura > alto) altura = alto;

            mitad = alto / 2;
            y0 = mitad - altura / 2;
            y1 = mitad + altura / 2;
            if (y0 < 0) y0 = 0;
            if (y1 > alto) y1 = alto;

            /* El color por distancia: lo único que da sensación de profundidad
             * cuando no hay texturas. Cerca claro, lejos oscuro. */
            color = 255 - (t >> 13);
            if (color < 32) color = 32;
            if (color > 255) color = 255;
            color = (color << 16) | (color << 8) | color;

            /* Cielo, pared, suelo. Tres tramos y ni un pixel sin escribir: el
             * fotograma anterior está debajo y no se limpia aparte. */
            y = 0;
            while (y < y0) { fb[y * stride + x] = 0x00101820; y = y + 1; }
            while (y < y1) { fb[y * stride + x] = color;      y = y + 1; }
            while (y < alto) { fb[y * stride + x] = 0x00202020; y = y + 1; }

            x = x + 1;
        }

        /* ── ENTRADA ──────────────────────────────────────────────────── */
        tecla = (int)bmo_valor(ent, ENT_TECLA, 0, 0, 0);
        if (tecla != 0) {
            if (tecla == 27) vivo = 0;                    /* ESC */
            if (tecla == 'w' || tecla == 'W') {
                nx = posx + fmul(dirx, 6553);
                ny = posy + fmul(diry, 6553);
                if (pared(nx >> 16, posy >> 16) == 0) posx = nx;
                if (pared(posx >> 16, ny >> 16) == 0) posy = ny;
            }
            if (tecla == 's' || tecla == 'S') {
                nx = posx - fmul(dirx, 6553);
                ny = posy - fmul(diry, 6553);
                if (pared(nx >> 16, posy >> 16) == 0) posx = nx;
                if (pared(posx >> 16, ny >> 16) == 0) posy = ny;
            }
            if (tecla == 'a' || tecla == 'A') {
                /* Girar es rotar los DOS vectores. Si se rota sólo el de
                 * dirección, el plano deja de ser perpendicular y la imagen se
                 * va deformando un poco en cada giro — y eso no se ve hasta
                 * que llevas veinte. */
                nx = fmul(dirx, cosg) + fmul(diry, seng);
                ny = fmul(diry, cosg) - fmul(dirx, seng);
                dirx = nx; diry = ny;
                nx = fmul(plax, cosg) + fmul(play, seng);
                ny = fmul(play, cosg) - fmul(plax, seng);
                plax = nx; play = ny;
            }
            if (tecla == 'd' || tecla == 'D') {
                nx = fmul(dirx, cosg) - fmul(diry, seng);
                ny = fmul(diry, cosg) + fmul(dirx, seng);
                dirx = nx; diry = ny;
                nx = fmul(plax, cosg) - fmul(play, seng);
                ny = fmul(play, cosg) + fmul(plax, seng);
                plax = nx; play = ny;
            }
        }

        /* Ceder el turno. Sin esto el bucle se come el quantum entero y el
         * sistema va a tirones — está dicho en `bmo.h` y aquí se cumple. */
        bmo_ceder();
    }

    /* Dejar la pantalla en negro al salir: quien viene detrás no tiene por qué
     * heredar los restos de otro. */
    fila = fb;
    i = 0;
    while (i < alto * stride) { fila[i] = 0; i = i + 1; }
    printf("raycaster: fuera\n");
    return 0;
}
