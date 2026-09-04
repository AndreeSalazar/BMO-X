/* LA MEDIDA DEL BLIT: cuanto cuesta escribir en el framebuffer, y por que.
 *
 * ============================================================================
 * POR QUE EXISTE
 * ============================================================================
 *
 * DOOM a 1600x1000 gasta 25,5 ms por fotograma SOLO en el blit, de 36 ms
 * totales. Son 6,4 MB por fotograma, o sea unos 250 MB/s -- y eso esta un
 * orden de magnitud por debajo de lo que da write-combining, que es como el
 * kernel mapea el framebuffer (`vmm::map_page_wc`).
 *
 * *** LA SOSPECHA, dicha antes de medir para que la medida pueda desmentirla:
 *
 * `memcpy` de BMO C es `rep movsb` (`bmo-lower::memoria::copiar`). Con ERMSB
 * --que el Zen 3 tiene-- `rep movsb` es la copia MAS RAPIDA que hay... a
 * memoria normal (WB). El camino rapido del microcodigo pide destino
 * CACHEABLE; con write-combining se cae a un camino lento.
 *
 * O sea que la misma instruccion que es la mejor para RAM podria ser de las
 * peores para la pantalla. Esta medida lo dice o lo desmiente.
 *
 * ============================================================================
 * QUE MIDE, y por que TRES filas y no dos
 * ============================================================================
 *
 *    1. memcpy a RAM normal          el techo de `rep movsb` en esta maquina
 *    2. memcpy al FRAMEBUFFER        lo mismo, pero a memoria WC
 *    3. bucle de 8 bytes al FB       la alternativa, sin `rep movsb`
 *
 * La fila 1 no sobra: sin ella, un numero malo en la 2 no se sabe si es de la
 * memoria WC o de que esta maquina copia despacio. **Una medida sin su testigo
 * no es una medida, es un numero.**
 *
 * Y la 3 es la que decide si hay algo que hacer: si iguala o mejora a la 2, el
 * arreglo esta nombrado. Si no, el limite es el bus y entonces si hace falta
 * una GPU.
 *
 * [!] NO DIBUJA NADA UTIL. Escribe negro en una banda de la pantalla, la mide,
 * y sale. Es un instrumento, no una demo.
 */

#include <bmo/bmo.h>
#include <bmo/pantalla.h>

/* Los bytes de cada pasada. 6400 = una fila de 1600 pixeles a 4 bytes, que es
 * exactamente lo que el blit de DOOM manda de una vez a escala 5. Medir con el
 * tamano REAL importa: `rep movsb` cambia de camino segun la longitud. */
#define TROZO 6400
/* Cuantas veces, para que el `rdtsc` no sea el que se mide. `coste_C.c` ya
 * enseno que un `rdtsc` suelto son ~113 ciclos: con 1000 pasadas de 6400 bytes
 * eso es ruido por debajo del 0,01%. */
#define PASADAS 1000

/* El origen, en RAM normal. Global y no local: 6400 bytes en la pila de una
 * tarea de Ring 3 es pedirle demasiado al marco. */
static unsigned char origen[TROZO];
/* El testigo: un destino en RAM normal, para la fila 1. */
static unsigned char destino_ram[TROZO];

int main(void)
{
    BMO_PANTALLA p;
    unsigned long long t0;
    unsigned long long ciclos_ram;
    unsigned long long ciclos_fb;
    unsigned long long ciclos_bucle;
    unsigned long long *d8;
    unsigned long long *o8;
    unsigned char *fb;
    int i;
    int k;
    int palabras;

    for (i = 0; i < TROZO; i = i + 1) {
        origen[i] = (unsigned char)(i & 0xFF);
    }

    if (bmo_pantalla_abrir(&p) == 0) {
        printf("blit: no tengo la pantalla (la tiene otro). Salgo.\n");
        return 1;
    }
    printf("blit: pantalla %d x %d, paso %d\n", p.ancho, p.alto, p.paso);

    /* La banda que se va a pisar: la fila 0, que en el peor caso es una linea
     * negra en lo alto. No se toca nada mas. */
    fb = (unsigned char *)p.pixeles;

    /* -- 1. A RAM NORMAL, que es el testigo ------------------------------- */
    t0 = __rdtsc();
    for (k = 0; k < PASADAS; k = k + 1) {
        memcpy(destino_ram, origen, TROZO);
    }
    ciclos_ram = __rdtsc() - t0;

    /* -- 2. AL FRAMEBUFFER, con el mismo memcpy --------------------------- */
    t0 = __rdtsc();
    for (k = 0; k < PASADAS; k = k + 1) {
        memcpy(fb, origen, TROZO);
    }
    ciclos_fb = __rdtsc() - t0;

    /* -- 3. AL FRAMEBUFFER, sin `rep movsb` ------------------------------- */
    /* Palabras de 8 bytes, que es lo que llena un bufer de write-combining sin
     * pedirle nada raro al compilador. */
    palabras = TROZO / 8;
    t0 = __rdtsc();
    for (k = 0; k < PASADAS; k = k + 1) {
        d8 = (unsigned long long *)fb;
        o8 = (unsigned long long *)origen;
        for (i = 0; i < palabras; i = i + 1) {
            d8[i] = o8[i];
        }
    }
    ciclos_bucle = __rdtsc() - t0;

    /* -- El informe ------------------------------------------------------- */
    /* Se dan CICLOS POR BYTE y no MB/s: los ciclos no dependen de a que reloj
     * fue el CPU en ese instante, y el reloj de un Ryzen se mueve solo. */
    printf("\n");
    printf("   bytes por pasada  %d      pasadas  %d\n", TROZO, PASADAS);
    printf("   1. memcpy a RAM         %d ciclos/KiB\n",
           (int)((ciclos_ram * 1024) / ((unsigned long long)TROZO * PASADAS)));
    printf("   2. memcpy al FB (WC)    %d ciclos/KiB\n",
           (int)((ciclos_fb * 1024) / ((unsigned long long)TROZO * PASADAS)));
    printf("   3. bucle de 8B al FB    %d ciclos/KiB\n",
           (int)((ciclos_bucle * 1024) / ((unsigned long long)TROZO * PASADAS)));
    printf("\n");
    printf("   si 2 es MUCHO peor que 1  -> `rep movsb` pierde con WC\n");
    printf("   si 3 gana a 2             -> el arreglo esta nombrado\n");
    printf("   si 3 no gana              -> el limite es el bus, no la copia\n");

    bmo_pantalla_cerrar(&p);
    return 0;
}
