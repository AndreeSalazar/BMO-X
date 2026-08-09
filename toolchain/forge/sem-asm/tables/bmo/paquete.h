/* paquete.h -- leer los DATOS que viajan dentro del propio `.bex`.
 *
 * == La idea, dicha por el dueno ==
 *
 *   "es un bef pero ese bex es el mismo que abre la caja: no lo duplica, lo lee
 *    y punto. Es una app como Windows pero no lo copia, lo deja en el lugar
 *    correcto y lee directo."
 *
 * Eso es literalmente lo que hace este fichero. El paquete **no se carga**: se
 * abre, se mira su indice, y cada recurso se lee del disco por su offset cuando
 * hace falta. Un paquete de 500 MB arranca igual de rapido que uno de 500 KB,
 * porque lo que el cargador mete en memoria es el CODIGO -- los recursos ni los
 * mira: `bex::is_loadable` mapea Code/RoData/Data/Bss y salta el resto.
 *
 * == Como se lee un paquete, en tres saltos ==
 *
 *   1. la cabecera BEF (48 B) dice donde esta la tabla de secciones,
 *   2. la tabla dice donde esta la seccion `0x0B` (Resources),
 *   3. dentro de ella, el indice "BRES" dice donde esta cada recurso.
 *
 * Los tres son un `fseek` y un `fread`. No hace falta maquinaria nueva.
 *
 * == Como se usa ==
 *
 *     PAQUETE *p = paquete_abrir("apps/doom.bex");
 *     char *wad = (char *)malloc(4 * 1024 * 1024);
 *     unsigned long long n = paquete_leer(p, "doom1.wad", wad, 4*1024*1024);
 *
 * [!] **El destino tiene que salir de `malloc`.** Es el contrato de `fread`, y
 * no es un capricho de esta cabecera: el kernel solo acepta escribir dentro de
 * un bloque que EL concedio, porque comprobar es una resta contra lo que
 * entrego. Un puntero a la pila no vale y devuelve cero.
 *
 * [!] Y **la ruta se pasa a mano, de momento**. Es lo unico provisional que hay
 * aqui: lo correcto es que el lanzador le entregue al proceso un handle de
 * lectura sobre su propia imagen, como capability -- se tiene porque te la
 * dieron, no porque adivines un nombre. Cuando esa operacion exista, cambia
 * `paquete_abrir` y nada mas.
 */
#ifndef BMO_PAQUETE_H
#define BMO_PAQUETE_H

#include <bmo/archivo.h>

/* Los numeros del formato. Viven en `bmo_abi::bef` y aqui se repiten porque C
 * no puede importar de Rust; si algun dia dejan de coincidir, la fila de
 * pruebas que empaqueta con la herramienta y lee con esta cabecera es la que
 * lo dice. */
#define BMO_BEF_MAGIC        0x31464542  /* "BEF1" */
#define BMO_BEF_CABECERA     48
#define BMO_BEF_ENTRADA      48
#define BMO_SECCION_RECURSOS 0x0B
#define BMO_BRES_MAGIC       0x53455242  /* "BRES" */
#define BMO_BRES_CABECERA    16
#define BMO_BRES_ENTRADA     64
#define BMO_BRES_NOMBRE_MAX  47

struct BMO_PAQUETE {
    FILE *f;
    /* Offset EN FICHERO donde empieza la seccion de recursos. Los offsets del
     * indice son relativos a ESTO, no al fichero: por eso hay que sumarlo. */
    unsigned long long base;
    unsigned long long cuantos;
    /* Un trocito de bloque para leer cabeceras. Sale del mismo `malloc` que la
     * estructura -- son 4 peticiones por proceso y no se pueden gastar en
     * esto. */
    char *scratch;
};
typedef struct BMO_PAQUETE PAQUETE;

/* Los enteros del formato, leidos byte a byte. No se castea el buffer a
 * `unsigned int *`: nada garantiza que un offset del fichero caiga alineado, y
 * una lectura desalineada es de las que funcionan hasta que no. */
unsigned long long bmo_u32le(char *b, int i) {
    unsigned long long v;
    v = (unsigned long long)(b[i] & 0xFF);
    v = v | ((unsigned long long)(b[i + 1] & 0xFF) << 8);
    v = v | ((unsigned long long)(b[i + 2] & 0xFF) << 16);
    v = v | ((unsigned long long)(b[i + 3] & 0xFF) << 24);
    return v;
}

unsigned long long bmo_u64le(char *b, int i) {
    return bmo_u32le(b, i) | (bmo_u32le(b, i + 4) << 32);
}

/* Lee `n` bytes desde `pos` al scratch. Devuelve 1 si los trajo todos. */
int bmo_pq_leer(PAQUETE *p, unsigned long long pos, unsigned long long n) {
    fseek(p->f, pos, 0);
    if (fread(p->scratch, 1, n, p->f) != n) {
        return 0;
    }
    return 1;
}

/* Abre el paquete y localiza su indice. Devuelve 0 si no lo es.
 *
 * Que un `.bex` NO lleve recursos no es un fallo: es lo que tienen todos los
 * de hoy. Se contesta 0 y quien llama sigue su camino. */
PAQUETE *paquete_abrir(char *ruta) {
    PAQUETE *p;
    unsigned long long tabla;
    unsigned long long count;
    unsigned long long i;
    unsigned long long off;
    unsigned long long largo;
    char *s;

    p = (PAQUETE *)malloc(1024);
    if (p == 0) return 0;
    /* La estructura ocupa la cabeza del bloque y el scratch va detras: una
     * sola peticion para las dos cosas. */
    p->scratch = (char *)p + 128;
    p->base = 0;
    p->cuantos = 0;

    p->f = fopen(ruta, "r");
    if (p->f == 0) return 0;
    s = p->scratch;

    if (!bmo_pq_leer(p, 0, BMO_BEF_CABECERA)) return 0;
    if (bmo_u32le(s, 0) != BMO_BEF_MAGIC) return 0;
    tabla = bmo_u64le(s, 32);
    count = bmo_u32le(s, 40);

    /* La tabla, entrada a entrada. Se busca la seccion 0x0B. */
    off = 0;
    largo = 0;
    i = 0;
    while (i < count) {
        if (!bmo_pq_leer(p, tabla + i * BMO_BEF_ENTRADA, BMO_BEF_ENTRADA)) return 0;
        if ((s[0] & 0xFF) == BMO_SECCION_RECURSOS) {
            off = bmo_u64le(s, 8);
            largo = bmo_u64le(s, 16);
            i = count;
        } else {
            i = i + 1;
        }
    }
    if (largo == 0) return 0;

    /* Y su cabecera "BRES". */
    if (!bmo_pq_leer(p, off, BMO_BRES_CABECERA)) return 0;
    if (bmo_u32le(s, 0) != BMO_BRES_MAGIC) return 0;
    p->base = off;
    p->cuantos = bmo_u32le(s, 4);
    return p;
}

/* Cuantos recursos lleva. */
unsigned long long paquete_cuantos(PAQUETE *p) {
    if (p == 0) return 0;
    return p->cuantos;
}

/* Busca por nombre y deja donde y cuanto. Devuelve 1 si lo encontro.
 *
 * Recorrido lineal: un paquete tiene unidades de recursos, no miles, y una
 * tabla ordenada aqui seria mas formato que validar a cambio de nada. */
int paquete_buscar(PAQUETE *p, char *nombre, unsigned long long *pos,
                   unsigned long long *tam) {
    unsigned long long i;
    unsigned long long largo;
    unsigned long long k;
    int igual;
    char *s;

    if (p == 0 || p->base == 0) return 0;
    s = p->scratch;
    largo = 0;
    while (nombre[largo] != 0) { largo = largo + 1; }
    if (largo == 0 || largo > BMO_BRES_NOMBRE_MAX) return 0;

    i = 0;
    while (i < p->cuantos) {
        if (!bmo_pq_leer(p, p->base + BMO_BRES_CABECERA + i * BMO_BRES_ENTRADA,
                         BMO_BRES_ENTRADA)) {
            return 0;
        }
        if ((unsigned long long)(s[16] & 0xFF) == largo) {
            igual = 1;
            k = 0;
            while (k < largo) {
                if (s[17 + k] != nombre[k]) { igual = 0; k = largo; }
                else { k = k + 1; }
            }
            if (igual) {
                /* El offset del indice es RELATIVO a la seccion; el que quiere
                 * quien va a hacer `fseek` es del fichero. Sumar aqui, y no en
                 * quien llama, es lo que evita que cada usuario se acuerde. */
                *pos = p->base + bmo_u64le(s, 0);
                *tam = bmo_u64le(s, 8);
                return 1;
            }
        }
        i = i + 1;
    }
    return 0;
}

/* Lee un recurso entero a `dst`. Devuelve cuantos bytes trajo.
 *
 * `tope` es el tamano de `dst`: si el recurso no cabe **no se lee un trozo**,
 * se devuelve 0. Media textura o medio WAD se parecen demasiado a uno entero. */
unsigned long long paquete_leer(PAQUETE *p, char *nombre, void *dst,
                                unsigned long long tope) {
    unsigned long long pos;
    unsigned long long tam;
    if (!paquete_buscar(p, nombre, &pos, &tam)) return 0;
    if (tam > tope) return 0;
    fseek(p->f, pos, 0);
    return fread(dst, 1, tam, p->f);
}

#endif /* BMO_PAQUETE_H */
