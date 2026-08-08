/* memoria_C.bex -- el programa que ESTRENA `KIND_MEMORIA`.
 *
 * La capability de memoria se cableo de punta a punta el 2026-08-02 --kernel,
 * userland y el `malloc` de C-- y **ningun programa la ha llamado nunca en el
 * Ryzen**. Esto es lo que la llama.
 *
 * == Por que no basta con "malloc devolvio algo distinto de cero" ==
 *
 * Un puntero no nulo solo prueba que el kernel contesto. Que las paginas esten
 * de verdad mapeadas, y que sean paginas DISTINTAS, no se sabe hasta que
 * alguien las escribe y vuelve a leerlas. Un mapeo que repitiera el mismo
 * marco fisico dieciseis veces devolveria un puntero perfecto y perderia
 * quince dieciseisavos de lo escrito, y eso pasa desapercibido si el programa
 * solo mira el primer byte.
 *
 * Por eso hay cuatro pruebas y cada una puede fallar sola:
 *
 *   1. hay bloque         -- `malloc(1024)` != 0, y se ensena su direccion.
 *   2. se escribe y se lee -- los 1024 bytes, con un valor distinto en cada uno.
 *   3. son paginas de verdad -- 64 KiB = 16 paginas, una marca en cada una y
 *                              releidas despues. Si dos paginas fueran el
 *                              mismo marco, las marcas se pisarian.
 *   4. el tope se cumple  -- el kernel acepta CUATRO peticiones por proceso; la
 *                           quinta tiene que devolver 0, no memoria de nadie.
 *
 * == Lo que se espera ver ==
 *
 *   base = 0xe0000000        (`vmm::MEMORIA_VA_BASE`; la primera peticion cae
 *                             siempre ahi, y las siguientes mas arriba)
 *   1024 bytes verificados, 0 malos
 *   16 paginas verificadas, 0 malas
 *   la 5a peticion devolvio 0: el tope se cumple
 *
 * Si el tope NO se cumpliera, seria peor que un fallo: sin forma de devolver
 * memoria, el numero de peticiones ES el numero de fugas posibles.
 *
 * == Como se lanza ==
 *
 *   c/memc.bex     desde la caja Ejecutar del escritorio.
 *
 * * La linea de CABINA `mem: bloque entregado a Ring 3` **no se va a ver** si
 *   se lanza desde el escritorio: mientras el compositor tiene la pantalla, el
 *   panel del kernel no se pinta. El contador vive igual, y se mira con `info`
 *   (fila "memoria a R3"). Esa fila subiendo es la confirmacion desde el otro
 *   lado: la que da el kernel, no la que da el programa.
 *
 * Compilar:
 *   cargo run -p bmo-c-front -- toolchain/lang/c/examples/memoria_C.c \
 *       -o memc.bex
 */

int main() {
    char *a;
    char *b;
    char *c;
    char *d;
    char *e;
    int i;
    int malos;
    int esperado;

    printf("KIND_MEMORIA - la primera vez que un programa PIDE\n");

    /* -- 1. Hay bloque ---------------------------------------------- */
    a = malloc(1024);
    printf("malloc(1024) = 0x%x\n", a);
    if (a == 0) {
        printf("FALLO: el kernel no entrego bloque\n");
        return 1;
    }

    /* -- 2. Se escribe y se lee ------------------------------------- */
    /* Un valor distinto por byte, y todos por debajo de 128: un `char` de C
     * es de signo indefinido, y comparar 200 contra -56 seria un fallo del
     * programa, no de la memoria. */
    for (i = 0; i < 1024; i = i + 1) {
        a[i] = (i * 7) % 127;
    }
    malos = 0;
    for (i = 0; i < 1024; i = i + 1) {
        esperado = (i * 7) % 127;
        if (a[i] != esperado) {
            malos = malos + 1;
        }
    }
    printf("1024 bytes verificados, %d malos\n", malos);

    /* -- 3. Son paginas de verdad ----------------------------------- */
    /* 64 KiB = dieciseis paginas de 4 KiB. Se marca la primera posicion de
     * cada una y se releen TODAS despues de escribirlas todas -- releer una
     * por una no distinguiria dieciseis paginas de una sola repetida. */
    b = malloc(65536);
    printf("malloc(65536) = 0x%x\n", b);
    if (b == 0) {
        printf("FALLO: la segunda peticion no dio bloque\n");
        return 1;
    }
    for (i = 0; i < 16; i = i + 1) {
        b[i * 4096] = 100 + i;
    }
    malos = 0;
    for (i = 0; i < 16; i = i + 1) {
        if (b[i * 4096] != 100 + i) {
            malos = malos + 1;
        }
    }
    printf("16 paginas verificadas, %d malas\n", malos);

    /* El ultimo byte del bloque, que es el que se sale si el redondeo a
     * paginas se hiciera hacia abajo en vez de hacia arriba. */
    b[65535] = 42;
    printf("ultimo byte del bloque = %d\n", b[65535]);

    /* -- 4. El tope se cumple --------------------------------------- */
    /* Van dos peticiones gastadas. La tercera y la cuarta tienen que darse; la
     * quinta, no. */
    c = malloc(4096);
    d = malloc(4096);
    printf("peticion 3 = 0x%x   peticion 4 = 0x%x\n", c, d);
    if (c == 0) {
        printf("FALLO: la 3a peticion se rechazo y el tope son 4\n");
        return 1;
    }
    if (d == 0) {
        printf("FALLO: la 4a peticion se rechazo y el tope son 4\n");
        return 1;
    }

    e = malloc(16);
    if (e == 0) {
        printf("la 5a peticion devolvio 0: el tope se cumple\n");
    } else {
        printf("FALLO: la 5a peticion dio 0x%x y el tope son 4\n", e);
        return 1;
    }

    /* Los cuatro bloques tienen que ser rangos distintos y crecientes: el
     * cursor del kernel avanza, asi que una peticion nunca puede caer sobre la
     * anterior. Si dos coincidieran, el segundo `malloc` habria entregado
     * memoria que ya era de alguien -- del mismo proceso, pero de alguien. */
    if (b <= a) {
        printf("FALLO: el 2o bloque no esta por encima del 1o\n");
        return 1;
    }
    if (c <= b) {
        printf("FALLO: el 3er bloque no esta por encima del 2o\n");
        return 1;
    }
    if (d <= c) {
        printf("FALLO: el 4o bloque no esta por encima del 3o\n");
        return 1;
    }

    if (malos != 0) {
        printf("MEMORIA ENTREGADA, PERO CON BYTES MALOS\n");
        return 1;
    }
    printf("MEMORIA: las cuatro pruebas pasan\n");
    return 0;
}
