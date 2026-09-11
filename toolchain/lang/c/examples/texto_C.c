/* texto_C.c -- un bloc de notas, dentro de una ventana.
 *
 * == La pregunta que contesta ==
 *
 * Se puede ESCRIBIR en BMO-X? Hasta hoy no: una app en ventana recibia
 * SCANCODES --que tecla se movio-- y nunca la LETRA que produjo. Con eso se
 * juega, porque un juego pregunta "esta la flecha abajo pulsada AHORA"; con eso
 * no se escribe, porque quien escribe pregunta "que letra salio".
 *
 * Sacar la letra del scancode habria significado copiar la distribucion
 * espanola entera --tildes, la ene, AltGr, teclas muertas-- aqui dentro. Dos
 * mapas de teclado son dos teclados, y se separan el dia que alguien arregle
 * una tecla en uno de los dos. Asi que la letra la manda quien ya la sabe: el
 * kernel la cocina, el DIRECTOR la reenvia con el bit 62 puesto, y esto la lee.
 * Ver `bmo_sup_es_caracter` en `<bmo/superficie.h>`.
 *
 * == Lo que hace ==
 *
 * ```text
 *    abre datos/notas.txt     y si no existe, empieza en blanco
 *    se escribe encima        letras, acentos, retroceso, Supr, retorno
 *    se navega                flechas, Inicio, Fin, RePag, AvPag
 *    se guarda                con el boton de la barra
 *    se vuelve a cargar       con el otro, y pierde lo que no guardaste
 * ```
 *
 * == ** LOS VATIOS ESTAN EN LAS DECISIONES QUE NO SE VEN ==
 *
 * Un bloc de notas es el programa mas facil de escribir mal en consumo, porque
 * la version obvia --repintar sesenta veces por segundo-- gasta lo mismo
 * mirando una pagina quieta que escribiendo a toda velocidad. Aqui hay tres
 * decisiones, y las tres son de `docs/maestro/EFICIENCIA_MAESTRO.md`:
 *
 *    1. **Si no paso nada, no se pinta.** El dibujo cuelga de `repinta`, y
 *       `repinta` solo se enciende cuando llega una tecla, un clic, o la
 *       ventana vuelve a verse. Una pagina quieta cuesta un despertar cada
 *       16 ms para mirar un buzon vacio, y nada mas.
 *
 *    2. **Si no se ve, no se pinta** (R-APP8). El DIRECTOR lo deja escrito en
 *       el buzon y aqui se salta el fotograma entero. El bucle sigue vivo: al
 *       volver, el primer dibujo ya es el de ahora.
 *
 *    3. **El cursor NO parpadea.** Un cursor que parpadea es un repintado dos
 *       veces por segundo para siempre, incluso con la maquina en reposo y
 *       nadie delante. Es la clase de gasto que nadie mide porque parece
 *       gratis. Aqui el cursor es una barra quieta, y eso es una decision de
 *       vatios, no de estilo.
 *
 * == Lo que NO hace, dicho entero ==
 *
 *    sin seleccionar          no hay marcar con el raton, ni copiar, ni pegar.
 *                            Pegar pide un portapapeles, y eso no existe aun
 *    sin deshacer             un borrado es definitivo
 *    sin doblar lineas        una linea mas larga que la ventana se corta en la
 *                            pantalla; el texto sigue entero en memoria
 *    64 KiB de tope           se dice en la barra de abajo cuando se llena, y
 *                            no se pierde nada sin avisar
 *    se cierra por el marco   el boton lo pone el DIRECTOR, y **lo que no
 *                            guardaste se pierde**. El punto naranja de la
 *                            barra de arriba es el aviso
 *
 * [!] SIN TILDES DENTRO DE LAS CADENAS, y no es estilo: un caracter no-ASCII
 * en un literal hace que BMO C emita ~500 KB de basura, y `MAX_BEX` es 1 MiB.
 * Esta anotado en `leer_C.c` con los numeros. En los comentarios da igual.
 */

#include <stdlib.h>
#include <bmo/bmo.h>
#include <bmo/archivo.h>
#include <bmo/superficie.h>
#include <bmo/entrada.h>
#include <bmo/fuente.h>

#define RUTA "datos/notas.txt"

/* La ventana. Cabe de sobra en el panel y deja ver el escritorio detras, que es
 * lo que uno quiere de un bloc de notas y no de un juego. */
#define VEN_ANCHO 760
#define VEN_ALTO 500

/* Las tres bandas: barra de arriba, canal de numeros, barra de abajo. */
#define BARRA 30
#define PIE 24
#define CANAL 46

/* El area de texto, calculada de las bandas para que no haya dos numeros que
 * puedan discrepar. */
#define TX (CANAL + 8)
#define TY (BARRA + 6)
#define FILAS ((VEN_ALTO - PIE - 6 - TY) / BMO_FUENTE_ALTO)
#define COLS ((VEN_ANCHO - TX - 8) / BMO_FUENTE_ANCHO)

/* Los dos botones de la barra. */
#define BOT_Y 5
#define BOT_ALTO 20
#define BOT_ANCHO 88
#define BOT_G (VEN_ANCHO - 2 * BOT_ANCHO - 16)
#define BOT_R (VEN_ANCHO - BOT_ANCHO - 8)

#define CAP 65536

/* Los colores. Oscuro y de poco contraste en todo menos en el texto, que es lo
 * unico que hay que leer. */
#define C_FONDO 0x001B1F24
#define C_CANAL 0x0016191D
#define C_BARRA 0x00242A31
#define C_LINEA 0x00343B44
#define C_TEXTO 0x00E6EDF7
#define C_FLOJO 0x00768390
#define C_ACENTO 0x0058A6FF
#define C_SUCIO 0x00E3A008
#define C_BOTON 0x002F3743

static unsigned int *g_px;
static char *g_txt;
/* Bytes usados, y el cursor como DESPLAZAMIENTO en bytes. Un cursor en
 * (linea, columna) tendria que mantenerse de acuerdo con el texto en cada
 * insercion; un desplazamiento no puede estar de acuerdo o no estarlo: ES el
 * sitio. La linea y la columna se calculan cuando hacen falta, que es para
 * pintar y para nada mas. */
static int g_n;
static int g_cur;
static int g_sucio;
/* Primera linea visible. */
static int g_prim;
static char *g_aviso;

/* -- DIBUJO ------------------------------------------------------------- */

/* Un rectangulo, recortado contra la superficie. El recorte no es cortesia:
 * escribir pasado el final del bloque es un `#PF`, y ya se pagaron dos en
 * `raycaster_C.c`. */
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

/* -- NUMEROS A TEXTO, a mano -------------------------------------------- */

/* ** Y NO CON `sprintf`, a proposito. `sprintf` arrastra el formateador de
 * ejecucion entero de `<stdio.h>` --banderas, anchura, precision-- para escribir
 * un numero de dos cifras en el canal de la izquierda. Esto son catorce lineas
 * y no paga por lo que no usa, que es la regla de `EFICIENCIA_MAESTRO` aplicada
 * a los bytes del `.bex` en vez de a los vatios.
 *
 * Las dos devuelven la posicion donde se quedaron, asi que se encadenan. */
static int pega(char *dst, int i, const char *s) {
    int k;
    k = 0;
    while (s[k] != 0) {
        dst[i] = s[k];
        i = i + 1;
        k = k + 1;
    }
    dst[i] = 0;
    return i;
}

static int pega_num(char *dst, int i, int v) {
    char tmp[12];
    int k;
    if (v < 0) {
        dst[i] = '-';
        i = i + 1;
        v = -v;
    }
    k = 0;
    if (v == 0) {
        tmp[0] = '0';
        k = 1;
    }
    while (v > 0) {
        tmp[k] = (char)(48 + v % 10);
        k = k + 1;
        v = v / 10;
    }
    /* Las cifras salieron del reves: se devuelven al reves. */
    while (k > 0) {
        k = k - 1;
        dst[i] = tmp[k];
        i = i + 1;
    }
    dst[i] = 0;
    return i;
}

static void texto(int x, int y, const char *s, unsigned int c) {
    bmo_texto(g_px, VEN_ANCHO, VEN_ANCHO, VEN_ALTO, x, y, s, c);
}

/* -- EL TEXTO: DONDE EMPIEZA CADA LINEA --------------------------------- */

/* Cuantas lineas hay. Un texto vacio tiene UNA linea, no cero: el cursor
 * siempre esta en alguna. */
static int lineas(void) {
    int i;
    int n;
    n = 1;
    i = 0;
    while (i < g_n) {
        if (g_txt[i] == 10) n = n + 1;
        i = i + 1;
    }
    return n;
}

/* El byte donde empieza la linea `li`. Si se pide una linea que no existe
 * contesta el final del texto, que es donde esta el cursor cuando se pasa. */
static int ini_linea(int li) {
    int i;
    int n;
    if (li <= 0) return 0;
    n = 0;
    i = 0;
    while (i < g_n) {
        if (g_txt[i] == 10) {
            n = n + 1;
            if (n == li) return i + 1;
        }
        i = i + 1;
    }
    return g_n;
}

/* El byte donde acaba la linea que empieza en `ini`: su salto, o el final. */
static int fin_linea(int ini) {
    int i;
    i = ini;
    while (i < g_n) {
        if (g_txt[i] == 10) return i;
        i = i + 1;
    }
    return g_n;
}

/* En que linea esta el cursor. */
static int linea_cur(void) {
    int i;
    int n;
    n = 0;
    i = 0;
    while (i < g_cur) {
        if (g_txt[i] == 10) n = n + 1;
        i = i + 1;
    }
    return n;
}

/* El cursor tiene que estar a la vista: si no, se escribe a ciegas. Se llama
 * despues de CADA movimiento y de cada insercion, en un solo sitio. */
static void seguir(void) {
    int li;
    li = linea_cur();
    if (li < g_prim) g_prim = li;
    if (li >= g_prim + FILAS) g_prim = li - FILAS + 1;
    if (g_prim < 0) g_prim = 0;
}

/* -- EL TEXTO: CAMBIARLO ------------------------------------------------ */

static void inserta(int c) {
    if (g_n >= CAP) {
        g_aviso = "lleno: 64 KiB es el tope de este bloc";
        return;
    }
    memmove(g_txt + g_cur + 1, g_txt + g_cur, g_n - g_cur);
    g_txt[g_cur] = (char)c;
    g_n = g_n + 1;
    g_cur = g_cur + 1;
    g_sucio = 1;
}

static void borra_en(int pos) {
    if (pos < 0) return;
    if (pos >= g_n) return;
    memmove(g_txt + pos, g_txt + pos + 1, g_n - pos - 1);
    g_n = g_n - 1;
    g_sucio = 1;
}

/* -- EL DISCO ----------------------------------------------------------- */

static void cargar(void) {
    FILE *f;
    unsigned long long quedan;
    unsigned long long trajo;

    g_n = 0;
    g_cur = 0;
    g_prim = 0;
    g_sucio = 0;
    f = fopen(RUTA, "r");
    if (f == 0) {
        /* No es un fallo: es un fichero que todavia no existe. Decirlo asi
         * --en vez de "no se pudo abrir"-- es la diferencia entre mandar a
         * alguien a revisar la ruta y decirle que escriba. */
        g_aviso = "nuevo: datos/notas.txt aun no existe. Escribe y guarda";
        return;
    }
    quedan = bmo_quedan(f);
    if (quedan > (unsigned long long)CAP) quedan = (unsigned long long)CAP;
    if (quedan > 0) {
        /* El destino sale de `malloc` a proposito: el kernel solo escribe
         * dentro de un bloque que el mismo concedio. Ver `leer_C.c`. */
        trajo = fread(g_txt, 1, quedan, f);
        g_n = (int)trajo;
    }
    fclose(f);
    g_aviso = "cargado";
}

static void guardar(void) {
    FILE *f;
    unsigned long long puestos;

    f = fopen(RUTA, "w");
    if (f == 0) {
        g_aviso = "NO se pudo crear datos/notas.txt";
        return;
    }
    puestos = 0;
    if (g_n > 0) puestos = fwrite(g_txt, 1, (unsigned long long)g_n, f);
    /* Nada llega al disco hasta aqui: el kernel acumula y vuelca al cerrar.
     * Ver `<bmo/archivo.h>`. */
    fclose(f);
    if ((int)puestos == g_n) {
        g_sucio = 0;
        g_aviso = "guardado";
    } else {
        /* Se dice el numero, no "error al guardar": un fichero a medias con un
         * mensaje vago es lo peor de los dos mundos. */
        g_aviso = "guardado A MEDIAS: el disco tomo menos de lo que se le dio";
    }
}

/* -- UN FOTOGRAMA ------------------------------------------------------- */

static void boton(int x, const char *etiqueta, unsigned int color) {
    relleno(x, BOT_Y, BOT_ANCHO, BOT_ALTO, C_BOTON);
    relleno(x, BOT_Y, BOT_ANCHO, 1, C_LINEA);
    relleno(x, BOT_Y + BOT_ALTO - 1, BOT_ANCHO, 1, C_LINEA);
    texto(x + (BOT_ANCHO - bmo_texto_ancho(etiqueta)) / 2, BOT_Y + 2, etiqueta,
          color);
}

static void pinta(void) {
    char num[16];
    char est[64];
    int f;
    int li;
    int ini;
    int fin;
    int largo;
    int col;
    int total;
    int k;

    total = lineas();

    /* El fondo, el canal de los numeros y las dos barras. */
    relleno(0, 0, VEN_ANCHO, VEN_ALTO, C_FONDO);
    relleno(0, 0, CANAL, VEN_ALTO, C_CANAL);
    relleno(0, 0, VEN_ANCHO, BARRA, C_BARRA);
    relleno(0, BARRA - 1, VEN_ANCHO, 1, C_LINEA);
    relleno(0, VEN_ALTO - PIE, VEN_ANCHO, PIE, C_BARRA);
    relleno(0, VEN_ALTO - PIE, VEN_ANCHO, 1, C_LINEA);

    /* La barra de arriba: el nombre, el punto de "sin guardar", y los botones. */
    texto(10, 7, RUTA, C_ACENTO);
    if (g_sucio == 1) {
        relleno(14 + bmo_texto_ancho(RUTA), 13, 6, 6, C_SUCIO);
    }
    boton(BOT_G, "Guardar", C_TEXTO);
    boton(BOT_R, "Recargar", C_FLOJO);

    /* Las lineas visibles. Se pinta lo que cabe y el resto se queda fuera: una
     * linea larga se corta en la pantalla y sigue entera en memoria. */
    f = 0;
    while (f < FILAS) {
        li = g_prim + f;
        if (li >= total) break;
        ini = ini_linea(li);
        fin = fin_linea(ini);
        largo = fin - ini;
        if (largo > COLS) largo = COLS;

        /* El numero, alineado a la derecha del canal. Alinear a la derecha es
         * lo que hace que el 9 y el 10 no bailen. */
        pega_num(num, 0, li + 1);
        texto(CANAL - 8 - bmo_texto_ancho(num), TY + f * BMO_FUENTE_ALTO, num,
              C_FLOJO);
        if (largo > 0) {
            bmo_texto_n(g_px, VEN_ANCHO, VEN_ANCHO, VEN_ALTO, TX,
                        TY + f * BMO_FUENTE_ALTO, g_txt + ini, largo, C_TEXTO);
        }
        f = f + 1;
    }

    /* El cursor, encima del texto y sin parpadear. Ver la cabecera: parpadear
     * cuesta dos repintados por segundo para siempre. */
    li = linea_cur();
    if (li >= g_prim && li < g_prim + FILAS) {
        col = g_cur - ini_linea(li);
        if (col > COLS) col = COLS;
        relleno(TX + col * BMO_FUENTE_ANCHO, TY + (li - g_prim) * BMO_FUENTE_ALTO,
                2, BMO_FUENTE_ALTO, C_TEXTO);
    }

    /* La barra de abajo: donde estas, cuanto hay, y lo ultimo que paso. */
    k = pega(est, 0, "lin ");
    k = pega_num(est, k, li + 1);
    k = pega(est, k, "  col ");
    k = pega_num(est, k, g_cur - ini_linea(li) + 1);
    k = pega(est, k, "   ");
    k = pega_num(est, k, g_n);
    k = pega(est, k, " bytes");
    texto(10, VEN_ALTO - PIE + 4, est, C_FLOJO);
    if (g_aviso != 0) {
        texto(300, VEN_ALTO - PIE + 4, g_aviso, C_ACENTO);
    }
}

/* -- LA ENTRADA --------------------------------------------------------- */

/* Un clic. Devuelve 1 si hay que repintar. */
static int clic(int mx, int my) {
    int f;
    int li;
    int ini;
    int fin;
    int col;

    if (my < BARRA) {
        if (my >= BOT_Y && my < BOT_Y + BOT_ALTO) {
            if (mx >= BOT_G && mx < BOT_G + BOT_ANCHO) {
                guardar();
                return 1;
            }
            if (mx >= BOT_R && mx < BOT_R + BOT_ANCHO) {
                cargar();
                return 1;
            }
        }
        return 0;
    }
    if (my < TY) return 0;
    if (my >= VEN_ALTO - PIE) return 0;

    /* Dentro del texto: el cursor va donde cayo el dedo, recortado al final de
     * esa linea. Poner el cursor mas alla del final seria un cursor que senala
     * un sitio que no existe. */
    f = (my - TY) / BMO_FUENTE_ALTO;
    li = g_prim + f;
    if (li >= lineas()) li = lineas() - 1;
    ini = ini_linea(li);
    fin = fin_linea(ini);
    col = 0;
    if (mx > TX) col = (mx - TX) / BMO_FUENTE_ANCHO;
    if (col > fin - ini) col = fin - ini;
    g_cur = ini + col;
    return 1;
}

/* Una letra, o una tecla de navegacion. Devuelve 1 si hay que repintar. */
static int letra(int c) {
    int ini;
    int fin;
    int col;
    int li;
    int i;

    li = linea_cur();
    ini = ini_linea(li);

    if (c == BMO_TECLA_IZQUIERDA) {
        if (g_cur > 0) g_cur = g_cur - 1;
        seguir();
        return 1;
    }
    if (c == BMO_TECLA_DERECHA) {
        if (g_cur < g_n) g_cur = g_cur + 1;
        seguir();
        return 1;
    }
    if (c == BMO_TECLA_ARRIBA || c == BMO_TECLA_ABAJO) {
        /* Subir y bajar conservan la COLUMNA, que es lo que uno espera, y la
         * recortan al largo de la linea de destino. */
        col = g_cur - ini;
        if (c == BMO_TECLA_ARRIBA) {
            if (li == 0) return 0;
            li = li - 1;
        } else {
            if (li + 1 >= lineas()) return 0;
            li = li + 1;
        }
        ini = ini_linea(li);
        fin = fin_linea(ini);
        if (col > fin - ini) col = fin - ini;
        g_cur = ini + col;
        seguir();
        return 1;
    }
    if (c == BMO_TECLA_INICIO) {
        g_cur = ini;
        return 1;
    }
    if (c == BMO_TECLA_FIN) {
        g_cur = fin_linea(ini);
        return 1;
    }
    if (c == BMO_TECLA_REPAG || c == BMO_TECLA_AVPAG) {
        col = g_cur - ini;
        if (c == BMO_TECLA_REPAG) {
            li = li - FILAS;
            if (li < 0) li = 0;
        } else {
            li = li + FILAS;
            if (li >= lineas()) li = lineas() - 1;
        }
        ini = ini_linea(li);
        fin = fin_linea(ini);
        if (col > fin - ini) col = fin - ini;
        g_cur = ini + col;
        seguir();
        return 1;
    }
    if (c == BMO_TECLA_SUPR) {
        borra_en(g_cur);
        return 1;
    }
    if (c == 8) {
        if (g_cur == 0) return 0;
        g_cur = g_cur - 1;
        borra_en(g_cur);
        seguir();
        return 1;
    }
    if (c == 10 || c == 13) {
        inserta(10);
        seguir();
        return 1;
    }
    if (c == 9) {
        /* El tabulador entra como CUATRO ESPACIOS, y es a proposito: un byte
         * 9 guardado obliga a decidir cuanto mide al pintarlo, y entonces el
         * fichero se ve distinto segun quien lo abra. Cuatro espacios miden lo
         * mismo en todas partes. */
        i = 0;
        while (i < 4) {
            inserta(32);
            i = i + 1;
        }
        seguir();
        return 1;
    }
    /* Lo imprimible. Los codigos de navegacion que esta fuente no dibuja
     * --0x80..0x94 de `<bmo/entrada.h>`-- ya salieron arriba; el resto de los
     * de control se descarta en vez de meter un byte invisible en el fichero. */
    if (c >= 32 && c <= 255) {
        inserta(c);
        seguir();
        return 1;
    }
    return 0;
}

int main() {
    BMO_SUPERFICIE *sup;
    unsigned long long ev;
    int i;
    int repinta;
    int se_ve;
    int antes;

    sup = bmo_superficie_crear_con_buzon(VEN_ANCHO, VEN_ALTO, 64);
    if (sup == 0) {
        /* Sin compositor no hay ventana, y este programa ES una ventana: no
         * tiene una segunda mitad que tomar la pantalla entera, como si la
         * tiene el raycaster. Se dice y se sale. */
        printf("texto: hace falta el escritorio (nadie compone)\n");
        return 1;
    }
    g_px = bmo_superficie_pixeles(sup);
    g_txt = (char *)malloc(CAP + 1);
    if (g_txt == 0) {
        printf("texto: sin memoria para 64 KiB\n");
        return 1;
    }
    g_aviso = 0;
    cargar();
    printf("texto: bloc de notas en %dx%d, %d columnas por %d filas\n",
           VEN_ANCHO, VEN_ALTO, COLS, FILAS);

    repinta = 1;
    antes = 1;
    for (;;) {
        /* -- La entrada. Hasta 64 eventos por vuelta: lo que no se lea se
         * queda en el buzon y se lee en la siguiente, que a esta velocidad es
         * inmediatamente. */
        i = 0;
        while (i < 64) {
            i = i + 1;
            ev = bmo_superficie_evento(sup);
            if ((ev & BMO_EVENTO_HAY) == 0) break;
            /* ** EL ORDEN DE LAS TRES PREGUNTAS NO ES LIBRE: en un evento de
             * raton el byte bajo son los BOTONES, y en uno de caracter es una
             * letra. Leer el byte sin preguntar antes por el bit 63 y por el 62
             * no da un error, da un texto con basura dentro. */
            if (bmo_sup_es_raton(ev) == 1) {
                /* Solo al PULSAR: el soltar del mismo clic llegaria tambien, y
                 * un boton que se activa dos veces por clic es un boton roto. */
                if ((ev & BMO_EVENTO_PULSADA) == 0) continue;
                if (clic(bmo_sup_raton_x(ev), bmo_sup_raton_y(ev)) == 1) {
                    repinta = 1;
                }
                continue;
            }
            if (bmo_sup_es_caracter(ev) == 1) {
                if (letra(bmo_sup_caracter(ev)) == 1) repinta = 1;
                continue;
            }
            /* Un scancode. Aqui no se usa ninguno: las letras llegan por el
             * bit 62 ya cocidas, y mirar las dos colas a la vez haria que una
             * tecla contara dos veces. Se descarta a proposito. */
        }

        /* -- R-APP8: lo que no se ve, no se pinta. Y al volver a verse hay que
         * repintar aunque nadie haya tocado nada -- mientras no se veia, el
         * DIRECTOR pudo tapar estos pixeles. */
        se_ve = bmo_superficie_se_ve(sup);
        if (se_ve == 1 && antes == 0) repinta = 1;
        antes = se_ve;

        if (se_ve == 1 && repinta == 1) {
            pinta();
            /* La secuencia, DESPUES del ultimo pixel: subirla antes seria
             * prometer un dibujo a medias. Es R-APP4. */
            bmo_superficie_lista(sup);
            repinta = 0;
        }

        bmo_dormir(16000000);
    }
}
