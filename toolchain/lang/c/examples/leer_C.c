/* leer_C.c -- abrir un fichero del disco y leer un BLOQUE de golpe.
 *
 * == La pregunta que contesta ==
 *
 * Funciona la cadena entera --`fopen` -> `fread` -> `fseek` -> `fclose`-- en el
 * metal? Hasta hoy no se podia ni preguntar: leer un fichero desde C daba
 * SIETE BYTES por llamada al sistema y no habia forma de moverse por el.
 *
 * Y no es un programa de adorno: es exactamente lo que DOOM hace lo primero de
 * todo. Abre su WAD, lee un bloque, y salta al final a buscar el directorio de
 * lumps. Si esto sale, esa parte de DOOM esta resuelta; si no sale, sabemos
 * donde mirar antes de portar treinta y cinco mil lineas.
 *
 * == Que mira, en orden ==
 *
 *   1. Abre?                    `fopen` de un fichero que existe
 *   2. Cuanto dice que mide?    y que no sea cero
 *   3. Trae los bytes?          `fread` de un bloque entero
 *   4. Son los de verdad?       se ensenan, en crudo
 *   5. El cursor obedece?       `fseek` al principio y releer
 *   6. Coinciden las dos?       * ESTA es la prueba
 *
 * El paso 6 es el que no se puede fingir. Un `fread` que devolviera basura
 * tambien devolveria basura la segunda vez -- pero **basura distinta**. Que las
 * dos lecturas del mismo sitio den lo mismo es lo que distingue "leyo" de
 * "escribio algo en mi buffer".
 *
 * [!] SIN TILDES NI GUIONES LARGOS DENTRO DE LAS CADENAS, y no es estilo:
 *
 * Este programa pesaba **1.101.952 bytes** y no arrancaba --`MAX_BEX` es 1 MiB,
 * asi que `run` contestaba "no esta" con el fichero delante--. La causa eran
 * CUATRO caracteres no-ASCII dentro de literales: cada uno hace que BMO C emita
 * ~500 KB de basura. Quitandolos, el mismo programa ocupa **20.608**.
 *
 * O sea que el `.bex` de 1 MB nunca fue un problema de relleno ni de
 * alineacion: era codigo emitido de mas, y lo medido lo dijo entero -- la
 * seccion `code` salia con 696 KB y `rodata` con 397 KB para cien lineas de C.
 *
 * El bug del compilador esta ANOTADO Y SIN ARREGLAR. Mientras tanto, en los
 * literales solo ASCII. En los comentarios da igual: el preprocesador los quita
 * antes de que el lexer los vea.
 *
 * == Por que el destino sale de `malloc` y no de la pila ==
 *
 * Porque el kernel solo acepta escribir dentro de un bloque QUE EL CONCEDIO, y
 * el de `malloc` es ese. Un `char buf[64]` de la pila no vale, y el kernel lo
 * rechaza en vez de escribir donde no debe -- que es la respuesta correcta y no
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

    /* 1 - Abrir. La ruta es del volumen de datos, y `datos/salida.txt` existe
     * desde que el escritorio guarda ahi lo que sale de `Ejecutar`. */
    f = fopen("datos/salida.txt", "r");
    if (f == 0) {
        /* [!] La causa casi siempre es la misma y conviene decirla, no dejar que
         * el lector la adivine: `datos/salida.txt` NO lo crea correr un
         * programa --cada uno vuelca en `datos/c<nombre>.txt`-- sino el comando
         * `guarda` del escritorio. Un mensaje que dice "no existe" y no dice
         * como hacerlo existir manda a mirar la ruta, que estaba bien. */
        printf("1. NO ABRE datos/salida.txt\n");
        printf("   escribe 'guarda' en la caja y vuelve a correr esto\n");
        return 1;
    }
    printf("1. abre: si\n");

    /* 2 - Cuanto queda por leer. Cero aqui significa fichero vacio, y entonces
     * lo demas no prueba nada -- se dice y se sale. */
    quedan = bmo_quedan(f);
    printf("2. mide %d bytes\n", (int)quedan);
    if (quedan == 0) {
        printf("   vacio: nada que leer. Corre algo desde Ejecutar y repite.\n");
        fclose(f);
        return 1;
    }

    /* 3 - El destino, del bloque de `malloc`. Ver la cabecera de arriba. */
    mem = (char *)malloc(4096);
    if (mem == 0) {
        printf("3. sin memoria\n");
        fclose(f);
        return 1;
    }
    printf("3. bloque en %x\n", (int)__bmo_bloque_base);

    /* 4 - Leer de golpe, y ensenar lo que llego. */
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
        /* Los saltos de linea se ensenan como un punto: si no, el volcado se
         * parte y no se ve lo que se esta comparando. */
        if (mem[i] == 10 || mem[i] == 13) {
            printf(".");
        } else {
            printf("%c", mem[i]);
        }
        i = i + 1;
    }
    printf("]\n");

    /* 5 - Volver al principio y releer en la segunda mitad del bloque. */
    fseek(f, 0, 0);
    n2 = fread(mem + 2048, 1, CUANTOS, f);
    printf("5. tras fseek trajo %d bytes\n", (int)n2);

    /* 6 - * La prueba: mismo sitio, mismos bytes. */
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
        printf("6. LAS DOS LECTURAS COINCIDEN: fopen/fread/fseek funcionan\n");
    } else {
        printf("6. NO coinciden: el cursor o la copia estan mal\n");
    }

    fclose(f);
    return 0;
}
