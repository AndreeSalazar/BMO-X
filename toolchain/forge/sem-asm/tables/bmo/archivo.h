/* archivo.h — leer ficheros desde C, de verdad.
 *
 * ══ Por qué esto es una CABECERA y no un builtin del compilador ══
 *
 * Es la misma razón que da `bmo.h` de sí misma: aquí no hay libc que enlazar,
 * así que **la cabecera trae el cuerpo**. Y hay un motivo más fuerte para que
 * `fopen` no sea un caso del `match` del codegen: abrir un fichero son varias
 * llamadas —empujar la ruta en paquetes de ocho bytes y luego pedir el
 * handle—, y eso escrito a mano en opcodes serían doscientas líneas de bytes
 * que nadie podría leer. En C se lee, se prueba y se corrige.
 *
 * Los intrínsecos del compilador se quedan para lo que es UNA instrucción.
 *
 * ══ Lo que hace que esto sea rápido, y no lo era ══
 *
 * `ARCH_OP_LEER` devuelve **siete bytes por llamada**. Cargar un WAD de DOOM de
 * 4 MB así son ~600.000 llamadas al sistema.
 *
 * `ARCH_OP_LEER_EN` copia un bloque entero de golpe. Y no hace falta que el
 * kernel valide ningún puntero —infraestructura que no existe— porque el
 * destino **es un bloque que concedió el kernel**: comprobar es una resta
 * contra lo que entregó. Contrato en vez de comprobación.
 *
 * ══ Cómo se usa ══
 *
 *     FILE *f = fopen("apps/doom1.wad", "r");
 *     char *mem = malloc(4*1024*1024);
 *     fread(mem, 1, 4*1024*1024, f);
 *     fseek(f, 0, 0);
 *     fclose(f);
 *
 * ⚠️ `fread` sólo puede escribir dentro del bloque que dio `malloc`, porque es
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

/* ── La ruta, en paquetes de ocho ─────────────────────────────────────
 *
 * El kernel acumula la ruta llamada a llamada y corta en el primer byte cero.
 * Si la ruta mide un múltiplo de ocho, el último paquete va entero y hace falta
 * uno más con el cero: el bucle lo hace solo porque `fin` sólo se pone cuando
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

/* ── `FILE` ───────────────────────────────────────────────────────────
 *
 * Tres campos y ninguno de adorno: el handle del archivo, el del BLOQUE donde
 * se puede leer, y dónde empieza ese bloque en memoria. El tercero hace falta
 * para traducir el puntero que da el usuario a un desplazamiento dentro del
 * bloque, que es lo único que el kernel acepta.
 */
struct BMO_FILE {
    unsigned long long cap;
    unsigned long long bloque;
    unsigned long long base;
};
typedef struct BMO_FILE FILE;

/* ★ El bloque que reparte `malloc`, y su handle.
 *
 * Las escribe EL COMPILADOR: la emisión de `malloc` guarda aquí el handle que
 * le devolvió el kernel y la base del bloque, justo antes de devolver el
 * puntero. Antes ese handle se tiraba —se usaba para pedir la base y adiós— y
 * sin él `fread` no puede existir, porque el kernel sólo acepta escribir en un
 * bloque si le dices CUÁL.
 *
 * Van declaradas aquí y no en el compilador a propósito: si un programa no
 * incluye esta cabecera, estas globales no existen y `malloc` no emite ni un
 * byte para publicarlas. Quien no lee ficheros no paga por los que sí.
 *
 * Valen 0 hasta el primer `malloc`, y eso es lo correcto: antes de pedir
 * memoria no hay bloque del que hablar. */
unsigned long long __bmo_bloque_cap;
unsigned long long __bmo_bloque_base;

FILE *fopen(char *ruta, char *modo) {
    FILE *f;
    unsigned long long cap;
    /* El modo se ignora a propósito: hoy sólo se puede leer, y aceptar "w"
     * para luego no escribir sería la clase de promesa que aquí no se hace. */
    (void)modo;
    cap = bmo_abrir(ruta);
    if (cap == 0) return 0;
    f = (FILE *)malloc(24);
    if (f == 0) return 0;
    f->cap = cap;
    f->bloque = __bmo_bloque_cap;
    f->base = __bmo_bloque_base;
    return f;
}

/* Devuelve ELEMENTOS leídos, como `fread` de verdad — no bytes. */
unsigned long long fread(void *dst, unsigned long long tam,
                        unsigned long long n, FILE *f) {
    unsigned long long desde;
    unsigned long long leidos;
    if (f == 0 || tam == 0) return 0;
    /* El desplazamiento dentro del bloque. Si `dst` no está dentro, esto sale
     * un número enorme y el kernel lo rechaza por la comprobación de rango —
     * que es exactamente lo que tiene que pasar. */
    desde = (unsigned long long)dst - f->base;
    leidos = bmo_valor(f->cap, BMO_ARCH_LEER_EN, f->bloque, desde, tam * n);
    return leidos / tam;
}

int fseek(FILE *f, unsigned long long pos, int desde) {
    if (f == 0) return -1;
    (void)desde; /* sólo SEEK_SET por ahora, y se dice */
    bmo_valor(f->cap, BMO_ARCH_SALTAR, pos, 0, 0);
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

#endif /* BMO_ARCHIVO_H */
