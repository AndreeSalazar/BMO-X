/* archivo.h -- leer ficheros desde C, de verdad.
 *
 * == Por que esto es una CABECERA y no un builtin del compilador ==
 *
 * Es la misma razon que da `bmo.h` de si misma: aqui no hay libc que enlazar,
 * asi que **la cabecera trae el cuerpo**. Y hay un motivo mas fuerte para que
 * `fopen` no sea un caso del `match` del codegen: abrir un fichero son varias
 * llamadas --empujar la ruta en paquetes de ocho bytes y luego pedir el
 * handle--, y eso escrito a mano en opcodes serian doscientas lineas de bytes
 * que nadie podria leer. En C se lee, se prueba y se corrige.
 *
 * Los intrinsecos del compilador se quedan para lo que es UNA instruccion.
 *
 * == Lo que hace que esto sea rapido, y no lo era ==
 *
 * `ARCH_OP_LEER` devuelve **siete bytes por llamada**. Cargar un WAD de DOOM de
 * 4 MB asi son ~600.000 llamadas al sistema.
 *
 * `ARCH_OP_LEER_EN` copia un bloque entero de golpe. Y no hace falta que el
 * kernel valide ningun puntero --infraestructura que no existe-- porque el
 * destino **es un bloque que concedio el kernel**: comprobar es una resta
 * contra lo que entrego. Contrato en vez de comprobacion.
 *
 * == Como se usa ==
 *
 *     FILE *f = fopen("apps/doom1.wad", "r");
 *     char *mem = malloc(4*1024*1024);
 *     fread(mem, 1, 4*1024*1024, f);
 *     fseek(f, 0, 0);
 *     fclose(f);
 *
 * [!] `fread` solo puede escribir dentro del bloque que dio `malloc`, porque es
 * ese bloque el que el kernel conoce. Un puntero a la pila NO vale, y contesta
 * cero en vez de escribir donde no debe.
 */
#ifndef BMO_ARCHIVO_H
#define BMO_ARCHIVO_H

#include <bmo/bmo.h>

#define BMO_ARCH_LEER     0x01
#define BMO_ARCH_TAMANO   0x03
#define BMO_ARCH_CERRAR   0x04
#define BMO_ARCH_LEER_EN  0x06
#define BMO_ARCH_SALTAR   0x07

/* -- La ruta, en paquetes de ocho -------------------------------------
 *
 * El kernel acumula la ruta llamada a llamada y corta en el primer byte cero.
 * Si la ruta mide un multiplo de ocho, el ultimo paquete va entero y hace falta
 * uno mas con el cero: el bucle lo hace solo porque `fin` solo se pone cuando
 * se ha VISTO el terminador, no cuando se llenan ocho.
 */
unsigned long long bmo_abrir(char *ruta) {
    int i;
    int k;
    int fin;
    unsigned long long p;
    unsigned long long c;

    i = 0;
    fin = 0;
    while (fin == 0) {
        p = 0;
        k = 0;
        while (k < 8) {
            c = (unsigned long long)ruta[i];
            c = c & 0xFF;
            if (c == 0) {
                fin = 1;
                k = 8;
            } else {
                p = p | (c << (k * 8));
                i = i + 1;
                k = k + 1;
            }
        }
        bmo_codigo(BMO_TAREA_ACTUAL, BMO_OP_RUTA, p, 0, 0);
    }
    return bmo_valor(BMO_TAREA_ACTUAL, BMO_OP_ARCHIVO_ABRIR, 0, 0, 0);
}

/* -- `FILE` -----------------------------------------------------------
 *
 * Tres campos y ninguno de adorno: el handle del archivo, el del BLOQUE donde
 * se puede leer, y donde empieza ese bloque en memoria. El tercero hace falta
 * para traducir el puntero que da el usuario a un desplazamiento dentro del
 * bloque, que es lo unico que el kernel acepta.
 */
struct BMO_FILE {
    unsigned long long cap;
    unsigned long long bloque;
    unsigned long long base;
    /* * DONDE VA EL CURSOR, contado aqui.
     *
     * El kernel tiene `SALTAR` pero **no una operacion que devuelva la
     * posicion**, asi que `ftell` no puede preguntarsela: hay que llevarla. La
     * actualizan `fread` y `fseek`, que son los dos unicos que la mueven.
     *
     * Llevar un espejo de un estado que vive en otro sitio es una cosa que sale
     * mal sola en cuanto aparece un tercero que lo mueva. Hoy no lo hay, y por
     * eso se puede -- el dia que exista, esto se cambia por una operacion del
     * kernel y no por otro sitio donde apuntar. */
    unsigned long long pos;
};
typedef struct BMO_FILE FILE;

/* * El bloque que reparte `malloc`, y su handle.
 *
 * Las escribe EL COMPILADOR: la emision de `malloc` guarda aqui el handle que
 * le devolvio el kernel y la base del bloque, justo antes de devolver el
 * puntero. Antes ese handle se tiraba --se usaba para pedir la base y adios-- y
 * sin el `fread` no puede existir, porque el kernel solo acepta escribir en un
 * bloque si le dices CUAL.
 *
 * Van declaradas aqui y no en el compilador a proposito: si un programa no
 * incluye esta cabecera, estas globales no existen y `malloc` no emite ni un
 * byte para publicarlas. Quien no lee ficheros no paga por los que si.
 *
 * Valen 0 hasta el primer `malloc`, y eso es lo correcto: antes de pedir
 * memoria no hay bloque del que hablar. */
unsigned long long __bmo_bloque_cap;
unsigned long long __bmo_bloque_base;

FILE *fopen(char *ruta, char *modo) {
    FILE *f;
    unsigned long long cap;
    /* El modo se ignora a proposito: hoy solo se puede leer, y aceptar "w"
     * para luego no escribir seria la clase de promesa que aqui no se hace. */
    (void)modo;
    cap = bmo_abrir(ruta);
    if (cap == 0) return 0;
    f = (FILE *)malloc(32);
    if (f == 0) return 0;
    f->cap = cap;
    f->bloque = __bmo_bloque_cap;
    f->base = __bmo_bloque_base;
    f->pos = 0;
    return f;
}

/* Devuelve ELEMENTOS leidos, como `fread` de verdad -- no bytes. */
unsigned long long fread(void *dst, unsigned long long tam,
                        unsigned long long n, FILE *f) {
    unsigned long long desde;
    unsigned long long leidos;
    if (f == 0 || tam == 0) return 0;
    /* El desplazamiento dentro del bloque. Si `dst` no esta dentro, esto sale
     * un numero enorme y el kernel lo rechaza por la comprobacion de rango --
     * que es exactamente lo que tiene que pasar. */
    desde = (unsigned long long)dst - f->base;
    leidos = bmo_valor(f->cap, BMO_ARCH_LEER_EN, f->bloque, desde, tam * n);
    /* El cursor avanza por lo que se leyo DE VERDAD, no por lo que se pidio.
     * Sumar `tam*n` haria que `ftell` mintiera justo al final del fichero, que
     * es donde se le pregunta. */
    f->pos = f->pos + leidos;
    return leidos / tam;
}

int fseek(FILE *f, unsigned long long pos, int desde) {
    if (f == 0) return -1;
    (void)desde; /* solo SEEK_SET por ahora, y se dice */
    bmo_valor(f->cap, BMO_ARCH_SALTAR, pos, 0, 0);
    f->pos = pos;
    return 0;
}

unsigned long long bmo_quedan(FILE *f) {
    if (f == 0) return 0;
    return bmo_valor(f->cap, BMO_ARCH_TAMANO, 0, 0, 0);
}

int fclose(FILE *f) {
    if (f == 0) return -1;
    bmo_codigo(f->cap, BMO_ARCH_CERRAR, 0, 0, 0);
    free(f);
    return 0;
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
    if (f->pos >= bmo_quedan(f)) {
        return 1;
    }
    return 0;
}

/* Escribe `n` elementos de `tam` bytes. Devuelve ELEMENTOS escritos.
 *
 * [!] **Hoy escribe SIEMPRE CERO, y no es un fallo de esta funcion.**
 *
 * `fopen` ignora el modo porque el camino de creacion --`TASK_OP_ARCHIVO_CREAR`
 * y `ARCH_OP_ESCRIBIR`-- existe en el kernel pero **no esta cableado hasta
 * aqui**: un `FILE` abierto por `fopen` es de lectura, y no hay forma de
 * decirle otra cosa.
 *
 * Existir con esta forma vale igual: los 64 `fwrite` de DOOM COMPILAN y
 * enlazan, que es lo que hoy bloquea el unity build. Y devolver 0 es la
 * respuesta honesta -- quien mire el valor de retorno se entera de que no se
 * escribio, que es exactamente lo que un `fwrite` que falla debe contestar.
 *
 * Lo que NO se hace es fingir que escribio: eso daria un programa que cree
 * haber guardado la partida. */
unsigned long long fwrite(const void *src, unsigned long long tam,
                          unsigned long long n, FILE *f) {
    (void)src;
    (void)tam;
    (void)n;
    (void)f;
    return 0;
}

#endif /* BMO_ARCHIVO_H */
