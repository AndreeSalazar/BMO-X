/* raycaster_C.c — 2.5D en BMO C, sobre la pantalla de verdad.
 *
 * ══ Para qué existe ══
 *
 * Es el ensayo general de DOOM, y está hecho para contestar UNA pregunta que
 * hoy nadie puede contestar: **¿puede un programa en C tomar la pantalla,
 * dibujar un fotograma, leer el teclado y repetirlo sesenta veces por segundo,
 * en el metal?**
 *
 * DOOM son ~35.000 líneas en cincuenta ficheros. Si esto corre, DOOM deja de
 * ser "¿se puede?" y pasa a ser "cuánta libc falta", que es una lista y no una
 * pregunta.
 *
 * ── Los techos, RE-MEDIDOS el 2026-08-07 ──
 *
 * Aquí decía que DOOM chocaría con «la tabla de cadenas del IR (256 cadenas)» y
 * con «una libc de ocho funciones sin E/S de ficheros». **Las dos eran falsas**,
 * y conviene corregirlas porque mandaban a preocuparse por lo que no toca:
 *
 *   · **El IR no está en el camino de compilación.** `compile_source_to_bef` va
 *     `parse` → `codegen` directo a bytes; los topes de `bmo_abi::ir` (64
 *     funciones, 256 sentencias) no los ve nadie. Medido: 2.500 funciones y
 *     20.006 líneas compilan en 0,67 s.
 *   · **La E/S de ficheros existe** desde `b791ce4b`: `fopen`/`fread`/`fseek`/
 *     `fclose` sobre `KIND_ARCHIVO`.
 *
 * Los techos de verdad, con su número: el `.bex` de esas 20.006 líneas mide
 * 1.007.536 bytes y **`MAX_BEX` es 1.048.576 — el 96%**; y siguen sin existir
 * `sprintf`, `atoi` y `exit`.
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
 * un array desde su inicializador, y aquí no hace falta — se indexa igual.
 *
 * ★ Y AQUÍ HAY UNA HISTORIA, porque esta línea estuvo mintiendo meses.
 *
 * Un global inicializado con una cadena guarda la DIRECCIÓN del literal, y ésa
 * no se conoce hasta cargar el programa. El codegen no sabía ponerla y
 * **rellenaba de ceros sin decir nada**, así que `pared()` leía
 * `mapa[y*16+x]` desde la dirección 0 —el primer byte de la imagen, o sea el
 * `push rbp` de la primera función— y **las paredes de este laberinto eran el
 * código máquina del propio programa**.
 *
 * No se notó porque un raycaster que dibuja paredes desde bytes cualesquiera
 * sigue dibujando paredes: salía un laberinto plausible que no era éste. Lo
 * destapó un test de globales, no una foto de la pantalla.
 *
 * El 2026-08-07 se arregló primero moviendo la asignación a `main` —el remedio
 * que el propio mensaje de error recomendaba— y después de verdad: **el BEF ya
 * tiene relocations `SeccionAbs64`** y el cargador de Ring 0 las aplica, así
 * que el mapa puede volver a estar donde se escribió. El compilador deja el
 * hueco a cero y anota quién lo rellena; la dirección la pone el cargador, que
 * es el único que la sabe. */
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
    /* La barra de ayuda. Declaradas arriba porque BMO C pide las
     * declaraciones al principio de la funcion, estilo C89. */
    int bx; int by; int bw; int bh;

    /* ★ LA PANTALLA TIENE UN SOLO DUEÑO, y eso no es una limitación de este
     * programa: es el modelo. `gui.bex` la reclama al arrancar y no la suelta,
     * así que mientras el escritorio viva, aquí se contesta que no.
     *
     * No es un fallo que haya que arreglar en este fichero — un compositor que
     * cediera la pantalla a cualquiera que la pida sería un compositor que no
     * sirve. Lo que falta es que el escritorio sepa PRESTARLA y recuperarla, y
     * eso es trabajo suyo, no de un ejemplo. */
    pant = bmo_valor(BMO_TAREA_ACTUAL, BMO_OP_PANTALLA_RECLAMAR, 0, 0, 0);
    if (pant == 0) {
        printf("la pantalla ya tiene dueno: el escritorio la reclamo al arrancar\n");
        printf("esto corre desde el shell de Ring 0, donde no hay compositor\n");
        return 1;
    }
    base = bmo_valor(pant, FB_BASE, 0, 0, 0);
    dims = bmo_valor(pant, FB_DIMS, 0, 0, 0);
    st = bmo_valor(pant, FB_STRIDE, 0, 0, 0);
    fb = (unsigned int *)base;
    ancho = (int)(dims >> 32);
    alto = (int)(dims & 0xFFFFFFFF);
    stride = (int)(st >> 32);

    /* SIN ENTRADA NO SE ARRANCA. Ver la nota al final del fichero. */
    ent = bmo_valor(BMO_TAREA_ACTUAL, BMO_OP_ENTRADA_RECLAMAR, 0, 0, 0);
    if (ent == 0) {
        printf("sin teclado: no arranco, porque no podria salir\n");
        return 1;
    }

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

        /* COMO SE SALE, DICHO EN LA PANTALLA.
         *
         * Cuatro barras juntas para W A S D y una aparte, en cian, para ESC.
         * No es adorno: es el fallo de usabilidad que costo una sesion. Este
         * programa toma la pantalla ENTERA, asi que el escritorio desaparece y
         * con el el sitio donde uno leeria que hacer. El dueno busco la salida
         * con Alt+Tab y con Ctrl+Alt, que son atajos del escritorio y aqui no
         * existen.
         *
         * No hay fuente de texto en este ejemplo, asi que se dibujan BARRAS.
         * No es un manual, pero es mejor que una pantalla que no dice nada. */
        bh = 6;
        bw = 26;
        by = alto - 22;
        bx = 24;
        i = 0;
        while (i < 4) {
            y = by;
            while (y < by + bh) {
                x = bx;
                while (x < bx + bw) { fb[y * stride + x] = 0x00405060; x = x + 1; }
                y = y + 1;
            }
            bx = bx + bw + 8;
            i = i + 1;
        }
        bx = bx + 22;
        y = by;
        while (y < by + bh) {
            x = bx;
            while (x < bx + bw + 14) { fb[y * stride + x] = 0x0000E5FF; x = x + 1; }
            y = y + 1;
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
