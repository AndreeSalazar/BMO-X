/* raycaster_C.c -- 2.5D en BMO C, sobre la pantalla de verdad.
 *
 * == Para que existe ==
 *
 * Es el ensayo general de DOOM, y esta hecho para contestar UNA pregunta que
 * hoy nadie puede contestar: **puede un programa en C tomar la pantalla,
 * dibujar un fotograma, leer el teclado y repetirlo sesenta veces por segundo,
 * en el metal?**
 *
 * DOOM son ~35.000 lineas en cincuenta ficheros. Si esto corre, DOOM deja de
 * ser "se puede?" y pasa a ser "cuanta libc falta", que es una lista y no una
 * pregunta.
 *
 * -- Los techos, RE-MEDIDOS el 2026-08-07 --
 *
 * Aqui decia que DOOM chocaria con "la tabla de cadenas del IR (256 cadenas)" y
 * con "una libc de ocho funciones sin E/S de ficheros". **Las dos eran falsas**,
 * y conviene corregirlas porque mandaban a preocuparse por lo que no toca:
 *
 *   - **El IR no esta en el camino de compilacion.** `compile_source_to_bef` va
 *     `parse` -> `codegen` directo a bytes; los topes de `bmo_abi::ir` (64
 *     funciones, 256 sentencias) no los ve nadie. Medido: 2.500 funciones y
 *     20.006 lineas compilan en 0,67 s.
 *   - **La E/S de ficheros existe** desde `b791ce4b`: `fopen`/`fread`/`fseek`/
 *     `fclose` sobre `KIND_ARCHIVO`.
 *
 * Los techos de verdad, con su numero: el `.bex` de esas 20.006 lineas mide
 * 1.007.536 bytes y **`MAX_BEX` es 1.048.576 -- el 96%**; y siguen sin existir
 * `sprintf`, `atoi` y `exit`.
 *
 * == Por que NO hay un solo `float` ==
 *
 * Punto fijo 16.16, como el `fixed_t` de DOOM -- y por el mismo motivo que
 * tuvieron ellos en 1993, no por nostalgia: la ruta de coma flotante de BMO C
 * es joven (el emulador estreno SSE hace tres dias) y un renderizador es el
 * peor sitio para estrenarla. Aqui un entero de 32 bits vale por un numero con
 * dieciseis bits de parte decimal, y las multiplicaciones pasan por 64 bits
 * para no perder nada por el camino.
 *
 * == Por que no hay tabla de senos ==
 *
 * No hace falta ni una. Se lleva un VECTOR de direccion y un VECTOR de plano de
 * camara, y girar es multiplicar por dos constantes --el coseno y el seno de un
 * angulo fijo--. Cero trigonometria en tiempo de ejecucion, cero tablas
 * globales grandes que el compilador tenga que colocar.
 *
 * == Y por que no hay correccion de ojo de pez ==
 *
 * Porque con el plano de camara **no hace falta**: el rayo se define como
 * `dir + plano*x`, y entonces el parametro `t` con el que se avanza YA es la
 * distancia perpendicular a la pantalla. La correccion clasica por coseno es lo
 * que se paga por trabajar con angulos en vez de con vectores.
 */

/* La superficie sale del MONTON, y el monton pide UN bloque al kernel. La
 * imagen son 640*400*4 = 1.024.000 bytes, o sea que con el 1 MiB de serie no
 * cabe: `malloc` devolveria 0 y no habria ventana. Se declara antes del
 * `#include`, que es como `<stdlib.h>` lo lee. */
#define BMO_MONTON_BYTES (2 * 1024 * 1024)
#include <stdlib.h>

#include <bmo/bmo.h>
#include <bmo/superficie.h>
/* Los scancodes y los dos bits del evento crudo. Hasta hoy no hacia falta --
 * este programa solo leia CARACTERES-- y entra con el buzon: dentro de una
 * ventana lo que llega es scancode, y hay que saber cual es cual. */
#include <bmo/entrada.h>

/* Lo que mide la ventana cuando hay compositor. En pantalla exclusiva se usa
 * lo que mida el panel, que es lo que hacia este programa desde el primer dia. */
#define VEN_ANCHO 640
#define VEN_ALTO  400

/* Operaciones sobre el handle de pantalla (KIND_FRAMEBUFFER). */
#define FB_BASE   0x01
#define FB_DIMS   0x02   /* ancho<<32 | alto */
#define FB_STRIDE 0x03   /* stride<<32 | formato -- stride va en PIXELES */

/* Operaciones sobre el handle de entrada (KIND_INPUT). */
#define ENT_TECLA 0x03   /* no bloquea: 0 = no hay nada */

#define UNO   65536      /* 1.0 en 16.16 */
#define MAPA_N 16

/* El mundo. Un solo literal, y por eso una sola entrada en la tabla de cadenas
 * del IR -- que tiene 256 y conviene no gastarlas en decoracion. */
/* Puntero al literal y no `char mapa[]`: BMO C todavia no deduce el tamano de
 * un array desde su inicializador, y aqui no hace falta -- se indexa igual.
 *
 * * Y AQUI HAY UNA HISTORIA, porque esta linea estuvo mintiendo meses.
 *
 * Un global inicializado con una cadena guarda la DIRECCION del literal, y esa
 * no se conoce hasta cargar el programa. El codegen no sabia ponerla y
 * **rellenaba de ceros sin decir nada**, asi que `pared()` leia
 * `mapa[y*16+x]` desde la direccion 0 --el primer byte de la imagen, o sea el
 * `push rbp` de la primera funcion-- y **las paredes de este laberinto eran el
 * codigo maquina del propio programa**.
 *
 * No se noto porque un raycaster que dibuja paredes desde bytes cualesquiera
 * sigue dibujando paredes: salia un laberinto plausible que no era este. Lo
 * destapo un test de globales, no una foto de la pantalla.
 *
 * El 2026-08-07 se arreglo primero moviendo la asignacion a `main` --el remedio
 * que el propio mensaje de error recomendaba-- y despues de verdad: **el BEF ya
 * tiene relocations `SeccionAbs64`** y el cargador de Ring 0 las aplica, asi
 * que el mapa puede volver a estar donde se escribio. El compilador deja el
 * hueco a cero y anota quien lo rellena; la direccion la pone el cargador, que
 * es el unico que la sabe. */
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

/* -- 1.5  LAS TECLAS LOGICAS, y por que hacen falta ---------------------
 *
 * ** ESTE PROGRAMA LEE DE DOS SITIOS DISTINTOS, y no dicen lo mismo.
 *
 *    pantalla exclusiva   la cola de `KIND_ENTRADA`  ->  CARACTERES ya cocidos
 *    en una ventana       el BUZON de la superficie  ->  SCANCODES con flanco
 *
 * Traducir cada uno a la MISMA lista de acciones es lo que impide que este
 * fichero acabe con el juego escrito dos veces. Y hay un motivo mas fuerte que
 * la comodidad: un caracter **no tiene soltar**, asi que la version de pantalla
 * nunca podra saber que una tecla sigue pulsada. Con las acciones separadas del
 * origen, el dia que eso importe se arregla en una funcion. */
#define K_NADA 0
#define K_SALIR 1
#define K_ADELANTE 2
#define K_ATRAS 3
#define K_GIRA_IZQ 4
#define K_GIRA_DER 5
#define K_LADO_IZQ 6
#define K_LADO_DER 7
#define K_MENU 8
#define K_SUBE 9
#define K_BAJA 10
#define K_MENOS 11
#define K_MAS 12

int accion_de_caracter(int c) {
    if (c == 27) return K_SALIR;
    if (c == 'w' || c == 'W') return K_ADELANTE;
    if (c == 's' || c == 'S') return K_ATRAS;
    if (c == 'a' || c == 'A') return K_GIRA_IZQ;
    if (c == 'd' || c == 'D') return K_GIRA_DER;
    if (c == 'q' || c == 'Q') return K_LADO_IZQ;
    if (c == 'e' || c == 'E') return K_LADO_DER;
    if (c == 'm' || c == 'M') return K_MENU;
    if (c == '-') return K_MENOS;
    if (c == '+') return K_MAS;
    return K_NADA;
}

int accion_de_scancode(int sc) {
    if (sc == BMO_SC_ESC) return K_SALIR;
    if (sc == BMO_SC_W) return K_ADELANTE;
    if (sc == BMO_SC_S) return K_ATRAS;
    if (sc == BMO_SC_A) return K_GIRA_IZQ;
    if (sc == BMO_SC_D) return K_GIRA_DER;
    if (sc == BMO_SC_Q) return K_LADO_IZQ;
    if (sc == BMO_SC_E) return K_LADO_DER;
    if (sc == BMO_SC_M) return K_MENU;
    if (sc == BMO_SC_ARRIBA) return K_SUBE;
    if (sc == BMO_SC_ABAJO) return K_BAJA;
    if (sc == BMO_SC_IZQUIERDA) return K_MENOS;
    if (sc == BMO_SC_DERECHA) return K_MAS;
    return K_NADA;
}

/* Un rectangulo relleno. Estaba escrito tres veces en el bucle principal, con
 * los mismos cuatro `while` anidados cada vez. */
void barra(unsigned int *fb, int stride, int x, int y, int w, int h, unsigned int c) {
    int i;
    int j;
    j = 0;
    while (j < h) {
        i = 0;
        while (i < w) {
            fb[(y + j) * stride + (x + i)] = c;
            i = i + 1;
        }
        j = j + 1;
    }
}

/* -- 1.6  EL MENU, dibujado con barras -----------------------------------
 *
 * No hay fuente de texto para una app de C --REX no trae ninguna todavia-- asi
 * que las opciones son BARRAS: una fila por ajuste, y tantos segmentos
 * encendidos como vale. Se lee de un vistazo y no promete un idioma que este
 * programa no sabe escribir.
 *
 * La fila SENALADA lleva su marca a la izquierda. Es la unica diferencia que
 * hace falta para poder navegar sin leer. */
#define MENU_FILAS 3

void menu_pinta(unsigned int *fb, int stride, int ancho, int alto, int sel,
                int fov, int vel, int tema) {
    int px; int py; int pw; int ph;
    int f;
    int v;
    int k;
    int cx;

    pw = 240;
    ph = 24 + MENU_FILAS * 30;
    px = (ancho - pw) / 2;
    py = (alto - ph) / 2;

    barra(fb, stride, px, py, pw, ph, 0x00101828);
    barra(fb, stride, px, py, pw, 2, 0x0000E5FF);

    f = 0;
    while (f < MENU_FILAS) {
        v = fov;
        if (f == 1) v = vel;
        if (f == 2) v = tema;

        /* La marca de la fila senalada. */
        if (f == sel) {
            barra(fb, stride, px + 10, py + 22 + f * 30, 6, 14, 0x0000E5FF);
        }
        /* Tres segmentos: los encendidos hasta `v`. */
        k = 0;
        cx = px + 26;
        while (k < 3) {
            if (k <= v) {
                barra(fb, stride, cx, py + 22 + f * 30, 60, 14, 0x00308CB0);
            } else {
                barra(fb, stride, cx, py + 22 + f * 30, 60, 14, 0x00203038);
            }
            cx = cx + 66;
            k = k + 1;
        }
        f = f + 1;
    }
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

    /* Posicion y orientacion, todo en 16.16. Se empieza mirando al este. */
    int posx; int posy;
    int dirx; int diry;
    int plax; int play;

    /* Girar 5 grados: cos = 0.99619, sen = 0.08716. Las dos unicas constantes
     * trigonometricas del programa. */
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
    /* La ventana, si hay compositor. 0 = no lo hay, y entonces se va por el
     * camino de la pantalla exclusiva de siempre. */
    BMO_SUPERFICIE *sup;
    /* -- EL MENU Y LO QUE AJUSTA ---------------------------------------
     * `menu` abierto o no; `sel` la fila senalada; y los tres ajustes, cada
     * uno de 0 a 2. Van en enteros pequenos y no en banderas porque lo que se
     * dibuja son SEGMENTOS: el valor ES la cuenta de los encendidos. */
    int menu; int sel;
    int fov; int vel; int tema;
    int accion;
    unsigned long long ev;
    /* Lo que sale de los ajustes, ya en las unidades del bucle. */
    int paso_v;
    int fovval;
    unsigned int col_techo;
    unsigned int col_suelo;

    /* * LA PANTALLA TIENE UN SOLO DUENO, y eso no es una limitacion de este
     * programa: es el modelo. `gui.bex` la reclama al arrancar y no la suelta,
     * asi que mientras el escritorio viva, aqui se contesta que no.
     *
     * No es un fallo que haya que arreglar en este fichero -- un compositor que
     * cediera la pantalla a cualquiera que la pida seria un compositor que no
     * sirve. Lo que falta es que el escritorio sepa PRESTARLA y recuperarla, y
     * eso es trabajo suyo, no de un ejemplo. */
    /* ** PRIMERO SE PIDE VENTANA, Y SOLO SI NO HAY SE TOMA LA PANTALLA.
     *
     * Ese orden es el paso 2b de `docs/plan/PLAN_DIRECTOR.md`, y no es una
     * preferencia: mientras el escritorio viva, `PANTALLA_RECLAMAR` contesta
     * que no, asi que preguntar primero por ahi seria preguntar por el camino
     * que casi nunca esta abierto.
     *
     * `bmo_superficie_crear` devuelve 0 cuando **nadie compone** --lanzado
     * desde el shell de Ring 0--, y eso no es un fallo: es la otra mitad del
     * mismo programa. */
    pant = 0;
    ent = 0;
    /* ** SE PIDE BUZON, Y ESO ES LO QUE HACE QUE ESTA VENTANA SE PUEDA TOCAR.
     *
     * Sesenta y cuatro ranuras: un dedo no produce tantas entre dos fotogramas
     * ni de lejos, y si alguna vez se llenara el DIRECTOR descarta en vez de
     * esperar -- ver `<bmo/superficie.h>`. Una app que solo ensenara llamaria a
     * `bmo_superficie_crear` a secas y el escritorio conservaria las teclas. */
    sup = bmo_superficie_crear_con_buzon(VEN_ANCHO, VEN_ALTO, 64);
    if (sup != 0) {
        fb = bmo_superficie_pixeles(sup);
        ancho = VEN_ANCHO;
        alto = VEN_ALTO;
        /* Sin relleno: la cabecera declara `stride = ancho`, y quien pinta
         * tiene que usar el MISMO numero o pintaria en diagonal. */
        stride = VEN_ANCHO;
        printf("raycaster: en una ventana de 640x400\n");
        /* *** Y AQUI NO SE RECLAMA LA ENTRADA, A PROPOSITO.
         *
         * `ENTRADA_RECLAMAR` es de la pantalla entera: pedirla desde dentro de
         * una ventana le quitaria el teclado al escritorio, que es exactamente
         * el modelo viejo del que esto sale. Asi que en ventana **este
         * programa se mira y no se toca** -- es la casilla 4 de
         * `META-APP_HARD.md`, y se cierra en el paso 2c, no aqui.
         *
         * Se sale por el boton de cerrar del marco, que lo pone el DIRECTOR. */
    } else {
        /* * LA PANTALLA TIENE UN SOLO DUENO. Si no hay compositor que preste
         * una caja, se toma entera, que es lo que este ejemplo hacia siempre. */
        pant = bmo_valor(BMO_TAREA_ACTUAL, BMO_OP_PANTALLA_RECLAMAR, 0, 0, 0);
        if (pant == 0) {
            printf("ni ventana ni pantalla: no hay donde dibujar\n");
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
    }

    posx = 3 * UNO + 32768;
    posy = 3 * UNO + 32768;
    dirx = UNO;  diry = 0;
    plax = 0;    play = 43690;   /* 0.666 -- el campo de vision de siempre */
    cosg = 65286;
    seng = 5712;

    menu = 0;
    sel = 0;
    fov = 1;   /* 0 estrecho, 1 normal, 2 ancho */
    vel = 1;   /* 0 lenta,    1 normal, 2 rapida */
    tema = 1;  /* 0 noche,    1 normal, 2 claro  */

    vivo = 1;
    while (vivo == 1) {
        /* -- LOS AJUSTES, PASADOS A LAS UNIDADES DEL BUCLE ---------------
         *
         * Se derivan cada vuelta y no al cambiarlos. Cuesta seis comparaciones
         * por fotograma --nada al lado de 256.000 pixeles-- y a cambio no hay
         * dos sitios donde un ajuste se pueda quedar a medio aplicar: el menu
         * solo toca el NUMERO, y lo que ese numero significa se decide en un
         * unico sitio. El campo de vision es la excepcion y se dice por que en
         * su propio comentario: toca el plano de camara, que es estado. */
        paso_v = 6553;
        if (vel == 0) paso_v = 3276;
        if (vel == 2) paso_v = 13107;
        col_techo = 0x00101820;
        col_suelo = 0x00202020;
        if (tema == 0) { col_techo = 0x00060A10; col_suelo = 0x00101010; }
        if (tema == 2) { col_techo = 0x00203040; col_suelo = 0x00384048; }
        /* -- UNA COLUMNA, UN RAYO --------------------------------------- */
        x = 0;
        while (x < ancho) {
            /* camx va de -1.0 a +1.0 de un borde al otro de la pantalla. */
            camx = fdiv(2 * x, ancho) - UNO;
            rayx = dirx + fmul(plax, camx);
            rayy = diry + fmul(play, camx);

            /* Marchar el rayo. Un paso de 1/32 de casilla: suficiente para que
             * no se cuele por una esquina, y barato. Se para a 20 casillas --
             * mas lejos no hay nada que ensenar y si mucho que calcular. */
            t = 0;
            paso = 2048;
            golpe = 0;
            while (t < 20 * UNO) {
                t = t + paso;
                mx = (posx + fmul(rayx, t)) >> 16;
                my = (posy + fmul(rayy, t)) >> 16;
                if (pared(mx, my) == 1) {
                    golpe = 1;
                    /* * `break`, y NO `t = 20 * UNO`.
                     *
                     * Salir del bucle asignandole el tope al contador funciona
                     * --la condicion deja de cumplirse-- pero **borra la unica
                     * cosa que el bucle habia averiguado**: a que distancia
                     * estaba la pared. Y como despues se le restaba ese mismo
                     * tope, `t` valia CERO en todos los golpes. */
                    break;
                }
            }

            if (golpe == 1) {
                /* `t` YA es la distancia perpendicular: ver la cabecera. */
                if (t < 2048) t = 2048;
                /* * SIN `>> 16`, y esto es una leccion de unidades.
                 *
                 * `fdiv` divide dos numeros en 16.16 y devuelve 16.16. Pero
                 * aqui el dividendo `alto` son PIXELES --un entero pelado, 768--
                 * y el divisor `t` si es 16.16, asi que lo que sale ya es un
                 * entero: (alto<<16) / (d<<16) = alto/d. Desplazar otros 16
                 * bits daba **cero siempre**, y cero de altura es cielo y suelo
                 * sin una sola pared. */
                altura = fdiv(alto, t);
            } else {
                altura = 0;
            }
            if (altura > alto) altura = alto;

            mitad = alto / 2;
            y0 = mitad - altura / 2;
            y1 = mitad + altura / 2;
            /* ** LOS CUATRO TOPES, Y ANTES SOLO HABIA DOS.
             *
             * Estaban `y0 < 0` e `y1 > alto`, que son los que se le ocurren a
             * uno pensando en una pared muy alta. Faltaban los otros dos, y son
             * los que se recorren cuando `altura` sale NEGATIVA: entonces
             * `y0 = mitad - altura/2` se va hacia ARRIBA sin tope --el `y0 < 0`
             * no lo ve, porque es positivo y grande-- y el bucle del cielo
             * escribe pasado el final del framebuffer.
             *
             * [!] En el Ryzen eso no es un garabato: el kernel mapea
             * EXACTAMENTE `alto * stride * 4` redondeado a pagina
             * (`fb.rs::mapped_bytes`), asi que el primer pixel de mas es un
             * `#PF` y la tarea muere. Es lo que dejo dos entradas en
             * `datos/fallos.txt` el 2026-08-13, las dos `escribiendo`.
             *
             * Y es de la familia de los que se destapan al arreglar el de
             * delante: mientras `altura` valia siempre 0 --el `>>16` de mas del
             * 08-08-- este tope no podia hacer falta. */
            if (y0 < 0) y0 = 0;
            if (y0 > alto) y0 = alto;
            if (y1 > alto) y1 = alto;
            if (y1 < y0) y1 = y0;

            /* El color por distancia: lo unico que da sensacion de profundidad
             * cuando no hay texturas. Cerca claro, lejos oscuro. */
            color = 255 - (t >> 13);
            if (color < 32) color = 32;
            if (color > 255) color = 255;
            color = (color << 16) | (color << 8) | color;

            /* Cielo, pared, suelo. Tres tramos y ni un pixel sin escribir: el
             * fotograma anterior esta debajo y no se limpia aparte. */
            y = 0;
            while (y < y0) { fb[y * stride + x] = col_techo; y = y + 1; }
            while (y < y1) { fb[y * stride + x] = color;      y = y + 1; }
            while (y < alto) { fb[y * stride + x] = col_suelo; y = y + 1; }

            x = x + 1;
        }

        /* COMO SE SALE, DICHO EN LA PANTALLA.
         *
         * Seis barras juntas para W A S D Q E y una aparte, en cian, para ESC.
         * No es adorno: es el fallo de usabilidad que costo una sesion. Este
         * programa toma la pantalla ENTERA, asi que el escritorio desaparece y
         * con el el sitio donde uno leeria que hacer. El dueno busco la salida
         * con Alt+Tab y con Ctrl+Alt, que son atajos del escritorio y aqui no
         * existen.
         *
         * El que SI existe pase lo que pase es `Ctrl+Alt+ESC`, y no lo pone
         * aqui a proposito: lo mira el kernel en `poll_ascii`, antes de que
         * este programa vea la tecla. Es la red de abajo, no la salida normal
         * -- si hace falta usarla, este bucle tiene un fallo.
         *
         * No hay fuente de texto en este ejemplo, asi que se dibujan BARRAS.
         * No es un manual, pero es mejor que una pantalla que no dice nada. */
        bh = 6;
        bw = 26;
        by = alto - 22;
        bx = 24;
        i = 0;
        /* Solo en pantalla exclusiva: en una ventana la salida es el boton de
         * cerrar del marco, que ya esta ahi y lo pone el DIRECTOR. Dibujar
         * ademas estas barras seria ensenar una salida que aqui no existe. */
        if (sup != 0) i = 6;
        while (i < 6) {
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
        if (sup != 0) y = by + bh;
        while (y < by + bh) {
            x = bx;
            while (x < bx + bw + 14) { fb[y * stride + x] = 0x0000E5FF; x = x + 1; }
            y = y + 1;
        }

        /* -- ENTRADA ------------------------------------------------------
         *
         * ** SE DRENA LA COLA, no se lee UNA tecla por fotograma.
         *
         * Ninguno de los dos origenes bloquea y los dos entregan **una sola**
         * cosa por llamada. Con una lectura por cuadro, mantener la W pulsada
         * encola mas deprisa de lo que se saca --la repeticion automatica del
         * teclado va a 33 ms-- y el personaje sigue andando despues de soltar.
         * Ocho por cuadro es mas de lo que produce un dedo.
         *
         * ** Y AQUI ES DONDE ESTE FICHERO DEJA DE TENER DOS MITADES. Antes la
         * rama de ventana ni siquiera entraba --se mira y no se toca-- porque
         * no habia por donde llegar una tecla. Ahora hay dos origenes y una
         * sola lista de acciones: ver `accion_de_caracter` y
         * `accion_de_scancode` arriba. */
        i = 0;
        while (i < 8) {
            i = i + 1;
            accion = K_NADA;
            if (sup != 0) {
                ev = bmo_superficie_evento(sup);
                if ((ev & BMO_EVENTO_HAY) == 0) break;
                /* ** SOLO AL PULSAR. El buzon trae las DOS caras de cada tecla,
                 * y sin esto cada pulsacion contaria dos veces: un paso al
                 * bajar el dedo y otro al subirlo. Es la diferencia que la cola
                 * de caracteres esconde -- ahi el soltar no llega nunca. */
                if ((ev & BMO_EVENTO_PULSADA) == 0) continue;
                accion = accion_de_scancode((int)(ev & 0xFF));
            } else {
                tecla = (int)bmo_valor(ent, ENT_TECLA, 0, 0, 0);
                if (tecla == 0) break;             /* la cola esta vacia */
                /* *** EL BIT QUE DEJABA ESTE PROGRAMA SIN CONTROL.
                 *
                 * El kernel no contesta el caracter a secas: contesta
                 * `0x100 | byte`. Sin quitar ese bit, comparar la tecla contra
                 * 27 compara **283** contra 27 y no es cierto jamas -- el
                 * programa leia el teclado perfectamente y descartaba todo lo
                 * que leia. Eso es lo que dejo la maquina de rehen en el Ryzen:
                 * un `& 0xFF` de diferencia entre un programa y un secuestro. */
                accion = accion_de_caracter(tecla & 0xFF);
            }
            if (accion == K_NADA) continue;

            /* -- ** CON EL MENU ABIERTO, LAS MISMAS TECLAS SIGNIFICAN OTRA COSA
             *
             * Y esa es la razon de que las acciones esten separadas del origen:
             * el reparto de que hace cada tecla se decide UNA vez, aqui, y no
             * en cada rama de lectura. */
            if (menu == 1) {
                if (accion == K_MENU || accion == K_SALIR) { menu = 0; continue; }
                if (accion == K_SUBE || accion == K_ADELANTE) {
                    if (sel > 0) sel = sel - 1;
                    continue;
                }
                if (accion == K_BAJA || accion == K_ATRAS) {
                    if (sel < MENU_FILAS - 1) sel = sel + 1;
                    continue;
                }
                if (accion == K_MENOS || accion == K_GIRA_IZQ) {
                    if (sel == 0 && fov > 0) fov = fov - 1;
                    if (sel == 1 && vel > 0) vel = vel - 1;
                    if (sel == 2 && tema > 0) tema = tema - 1;
                    /* ** EL CAMPO DE VISION SE REHACE DESDE LA DIRECCION.
                     *
                     * El plano de camara es perpendicular a ella, asi que
                     * escalarlo por su cuenta lo dejaria torcido en cuanto se
                     * hubiera girado una vez -- y el sintoma seria una imagen
                     * que se deforma sola despues de dar una vuelta. */
                    if (sel == 0) {
                        fovval = 43690;
                        if (fov == 0) fovval = 32768;
                        if (fov == 2) fovval = 58982;
                        plax = fmul(0 - diry, fovval);
                        play = fmul(dirx, fovval);
                    }
                    continue;
                }
                if (accion == K_MAS || accion == K_GIRA_DER) {
                    if (sel == 0 && fov < 2) fov = fov + 1;
                    if (sel == 1 && vel < 2) vel = vel + 1;
                    if (sel == 2 && tema < 2) tema = tema + 1;
                    if (sel == 0) {
                        fovval = 43690;
                        if (fov == 0) fovval = 32768;
                        if (fov == 2) fovval = 58982;
                        plax = fmul(0 - diry, fovval);
                        play = fmul(dirx, fovval);
                    }
                    continue;
                }
                continue;
            }

            if (accion == K_MENU) { menu = 1; continue; }
            /* ** ESC NO CIERRA UNA VENTANA, CIERRA UN PROGRAMA A PANTALLA
             * COMPLETA. En una caja la salida es el boton del marco, que lo
             * pone el DIRECTOR y no se puede quitar desde aqui: una app que
             * decidiera cuando se la puede cerrar seria el modelo viejo. */
            if (accion == K_SALIR) {
                if (sup == 0) vivo = 0;
                continue;
            }
            if (accion == K_ADELANTE) {
                nx = posx + fmul(dirx, paso_v);
                ny = posy + fmul(diry, paso_v);
                if (pared(nx >> 16, posy >> 16) == 0) posx = nx;
                if (pared(posx >> 16, ny >> 16) == 0) posy = ny;
            }
            if (accion == K_ATRAS) {
                nx = posx - fmul(dirx, paso_v);
                ny = posy - fmul(diry, paso_v);
                if (pared(nx >> 16, posy >> 16) == 0) posx = nx;
                if (pared(posx >> 16, ny >> 16) == 0) posy = ny;
            }
            /* Girar es rotar los DOS vectores. Si se rota solo el de direccion,
             * el plano deja de ser perpendicular y la imagen se va deformando
             * un poco en cada giro -- y eso no se ve hasta que llevas veinte. */
            if (accion == K_GIRA_IZQ) {
                nx = fmul(dirx, cosg) + fmul(diry, seng);
                ny = fmul(diry, cosg) - fmul(dirx, seng);
                dirx = nx; diry = ny;
                nx = fmul(plax, cosg) + fmul(play, seng);
                ny = fmul(play, cosg) - fmul(plax, seng);
                plax = nx; play = ny;
            }
            if (accion == K_GIRA_DER) {
                nx = fmul(dirx, cosg) - fmul(diry, seng);
                ny = fmul(diry, cosg) + fmul(dirx, seng);
                dirx = nx; diry = ny;
                nx = fmul(plax, cosg) - fmul(play, seng);
                ny = fmul(play, cosg) + fmul(plax, seng);
                plax = nx; play = ny;
            }
            /* -- ANDAR DE LADO, sin girar la cabeza ----------------------
             *
             * El perpendicular a la direccion es `(diry, -dirx)`: mismo largo,
             * asi que se anda de lado igual de rapido que de frente. NO se usa
             * el plano de camara para esto aunque tambien sea perpendicular --
             * mide el campo de vision, y ahora ademas lo cambia el menu, o sea
             * que andar de lado dependeria de una opcion de dibujo.
             *
             * El choque se comprueba eje por eje, igual que en las dos de
             * arriba: asi uno se desliza a lo largo de una pared en vez de
             * quedarse pegado. */
            if (accion == K_LADO_IZQ) {
                nx = posx + fmul(diry, paso_v);
                ny = posy - fmul(dirx, paso_v);
                if (pared(nx >> 16, posy >> 16) == 0) posx = nx;
                if (pared(posx >> 16, ny >> 16) == 0) posy = ny;
            }
            if (accion == K_LADO_DER) {
                nx = posx - fmul(diry, paso_v);
                ny = posy + fmul(dirx, paso_v);
                if (pared(nx >> 16, posy >> 16) == 0) posx = nx;
                if (pared(posx >> 16, ny >> 16) == 0) posy = ny;
            }
        }

        /* -- EL MENU, ENCIMA DE TODO -------------------------------------
         *
         * Se pinta el ULTIMO del fotograma, despues del mundo: es lo unico que
         * tiene que taparlo todo. Y se pinta cada vuelta en vez de recordar si
         * ya estaba, porque el mundo se redibuja entero debajo -- guardar un
         * "ya lo pinte" seria guardar algo que el fotograma siguiente borra. */
        if (menu == 1) menu_pinta(fb, stride, ancho, alto, sel, fov, vel, tema);

        /* ** SEGUIMOS SIENDO LOS DUENOS DE LA PANTALLA?
         *
         * Esta pregunta es la que faltaba, y su ausencia es lo que dejo dos
         * `#PF` en `datos/fallos.txt` el 2026-08-13, los dos **escribiendo** y
         * los dos en direcciones del framebuffer.
         *
         * Cuando alguien pulsa `Ctrl+Alt+ESC`, el kernel no "avisa": ejecuta
         * `fb::release`, que **desmapea las paginas del framebuffer y revoca el
         * handle**. Desde ese instante `fb` apunta a memoria que ya no existe,
         * y el siguiente pixel que este programa escriba es un fallo de pagina.
         * Desde fuera se ve como *"la pantalla se limpio pero sigo dentro de la
         * app"*: el escritorio vuelve, y este proceso muere un momento despues.
         *
         * El kernel hace lo correcto -- un rescate que pidiera permiso no seria
         * un rescate--. Lo que faltaba era el otro lado del contrato: **un
         * programa que toma la pantalla tiene que comprobar que la sigue
         * teniendo**, y salir por su pie cuando no.
         *
         * Cuesta un INVOKE por fotograma, que es lo mismo que ya cuesta leer una
         * tecla. Y con el handle revocado la operacion contesta 0, que es un
         * valor que `FB_BASE` no puede devolver siendo valido. */
        if (sup == 0) {
            if (bmo_valor(pant, FB_BASE, 0, 0, 0) == 0) {
                printf("raycaster: me quitaron la pantalla, salgo\n");
                return 0;
            }
        }

        /* ** EL DIBUJO ESTA ENTERO. Es `R-APP4` de `META-APP_HARD.md`, y va
         * DESPUES del ultimo pixel del fotograma y nunca antes: subir la
         * secuencia al empezar seria prometer un dibujo que todavia se esta
         * haciendo, y el peor caso dejaria de ser "se ve el anterior una vuelta
         * mas" para pasar a ser "se ve medio dibujo". */
        if (sup != 0) bmo_superficie_lista(sup);

        /* ** EN VENTANA SE DUERME; CON LA PANTALLA ENTERA SE CEDE.
         *
         * No es un detalle de cortesia: `bmo_ceder()` vuelve a la cola LISTO, o
         * sea que un bucle que solo cede se lleva un turno entero cada vuelta.
         * Con la pantalla para uno eso no molesta a nadie. En una ventana, el
         * que sufre es el DIRECTOR -- que es justo quien tiene que componer
         * estos pixeles.
         *
         * Visto en el Ryzen el 2026-08-19: el escritorio se quedaba tan lento
         * que parecia que el teclado no respondia. No estaba secuestrado
         * --aqui no se reclama la entrada-- es que no quedaba turno para
         * pintar la tecla.
         *
         * 16 ms son ~60 fotogramas por segundo, que es todo lo que puede
         * ensenar la pantalla. */
        if (sup != 0) {
            bmo_dormir(16000000);
        } else {
            bmo_ceder();
        }
    }

    /* Dejar la pantalla en negro al salir: quien viene detras no tiene por que
     * heredar los restos de otro. */
    fila = fb;
    i = 0;
    /* Solo si la pantalla era nuestra. En ventana, borrar seria borrar la
     * propia superficie y dejarle al DIRECTOR un rectangulo negro que pegar. */
    if (sup != 0) i = alto * stride;
    while (i < alto * stride) { fila[i] = 0; i = i + 1; }
    printf("raycaster: fuera\n");
    return 0;
}
