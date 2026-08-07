/* leer_C.c — abrir un fichero del disco y leer un BLOQUE de golpe.
 *
 * ══ La pregunta que contesta ══
 *
 * ¿Funciona la cadena entera —`fopen` → `fread` → `fseek` → `fclose`— en el
 * metal? Hasta hoy no se podía ni preguntar: leer un fichero desde C daba
 * SIETE BYTES por llamada al sistema y no había forma de moverse por él.
 *
 * Y no es un programa de adorno: es exactamente lo que DOOM hace lo primero de
 * todo. Abre su WAD, lee un bloque, y salta al final a buscar el directorio de
 * lumps. Si esto sale, esa parte de DOOM está resuelta; si no sale, sabemos
 * dónde mirar antes de portar treinta y cinco mil líneas.
 *
 * ══ Qué mira, en orden ══
 *
 *   1. ¿Abre?                    `fopen` de un fichero que existe
 *   2. ¿Cuánto dice que mide?    y que no sea cero
 *   3. ¿Trae los bytes?          `fread` de un bloque entero
 *   4. ¿Son los de verdad?       se enseñan, en crudo
 *   5. ¿El cursor obedece?       `fseek` al principio y releer
 *   6. ¿Coinciden las dos?       ★ ÉSTA es la prueba
 *
 * El paso 6 es el que no se puede fingir. Un `fread` que devolviera basura
 * también devolvería basura la segunda vez — pero **basura distinta**. Que las
 * dos lecturas del mismo sitio den lo mismo es lo que distingue "leyó" de
 * "escribió algo en mi buffer".
 *
 * ══ Por qué el destino sale de `malloc` y no de la pila ══
 *
 * Porque el kernel sólo acepta escribir dentro de un bloque QUE ÉL CONCEDIÓ, y
 * el de `malloc` es ése. Un `char buf[64]` de la pila no vale, y el kernel lo
 * rechaza en vez de escribir donde no debe — que es la respuesta correcta y no
 * un fallo del programa.
 */

#include <bmo/archivo.h>

#define CUANTOS 32

int main() {
    FILE *f;
    char *mem;
    unsigned long long n1;
    unsigned long long n2;
    unsigned long long quedan;
    int i;
    int iguales;

    /* 1 · Abrir. La ruta es del volumen de datos, y `datos/salida.txt` existe
     * desde que el escritorio guarda ahí lo que sale de `Ejecutar`. */
    f = fopen("datos/salida.txt", "r");
    if (f == 0) {
        printf("1. NO ABRE — no existe la ruta o no hay handles libres\n");
        return 1;
    }
    printf("1. abre: si\n");

    /* 2 · Cuánto queda por leer. Cero aquí significa fichero vacío, y entonces
     * lo demás no prueba nada — se dice y se sale. */
    quedan = bmo_quedan(f);
    printf("2. mide %d bytes\n", (int)quedan);
    if (quedan == 0) {
        printf("   vacio: nada que leer. Corre algo desde Ejecutar y repite.\n");
        fclose(f);
        return 1;
    }

    /* 3 · El destino, del bloque de `malloc`. Ver la cabecera de arriba. */
    mem = (char *)malloc(4096);
    if (mem == 0) {
        printf("3. sin memoria\n");
        fclose(f);
        return 1;
    }
    printf("3. bloque en %x\n", (int)__bmo_bloque_base);

    /* 4 · Leer de golpe, y enseñar lo que llegó. */
    n1 = fread(mem, 1, CUANTOS, f);
    printf("4. fread trajo %d bytes\n", (int)n1);
    if (n1 == 0) {
        printf("   CERO: el kernel rechazo el destino o el archivo no da mas\n");
        fclose(f);
        return 1;
    }
    printf("   [");
    i = 0;
    while (i < (int)n1) {
        /* Los saltos de linea se enseñan como un punto: si no, el volcado se
         * parte y no se ve lo que se esta comparando. */
        if (mem[i] == 10 || mem[i] == 13) {
            printf(".");
        } else {
            printf("%c", mem[i]);
        }
        i = i + 1;
    }
    printf("]\n");

    /* 5 · Volver al principio y releer en la segunda mitad del bloque. */
    fseek(f, 0, 0);
    n2 = fread(mem + 2048, 1, CUANTOS, f);
    printf("5. tras fseek trajo %d bytes\n", (int)n2);

    /* 6 · ★ La prueba: mismo sitio, mismos bytes. */
    iguales = 1;
    if (n1 != n2) {
        iguales = 0;
    } else {
        i = 0;
        while (i < (int)n1) {
            if (mem[i] != mem[2048 + i]) {
                iguales = 0;
            }
            i = i + 1;
        }
    }
    if (iguales == 1) {
        printf("6. LAS DOS LECTURAS COINCIDEN — fopen/fread/fseek funcionan\n");
    } else {
        printf("6. NO coinciden: el cursor o la copia estan mal\n");
    }

    fclose(f);
    return 0;
}
