/* caja_C.c -- EL PROGRAMA LEE SU PROPIA CAJA.
 *
 * == Que se esta probando, y por que hace falta la maquina ==
 *
 * Este `.bex` viaja con datos DENTRO -- la seccion `Resources` (`0x0B`) que le
 * mete `bmo-pack` despues de compilarlo. El programa los saca de ahi **sin
 * escribir ninguna ruta**: le pide al kernel su propia imagen con
 * `TASK_OP_MI_PAQUETE` y lee por offset.
 *
 * O sea que en el disco hay UN fichero, no un binario y sus datos al lado. Y
 * los datos no se cargan con el programa: el cargador mapea Code/RoData/Data/
 * Bss y **salta la seccion de recursos**, que se lee cuando se pide y no antes.
 *
 * == Por que no basta el emulador ==
 *
 * En el anfitrion esto ya pasa, pero ahi el "disco" es un `HashMap` y la
 * operacion nueva la modela el propio banco. Lo que **solo** se puede ver aqui:
 *
 *   - que el kernel se acuerde de verdad de por donde entro este proceso
 *     (`ring0/task/paquete.rs`), y que la ruta sobreviva al lanzamiento;
 *   - que `archivo::open` acepte esa ruta y entregue un handle de lectura;
 *   - que `ARCH_OP_LEER_EN` traiga los bytes desde FAT32 o ESTRATOS de verdad,
 *     y no desde una tabla del banco de pruebas.
 *
 * == Que tiene que salir ==
 *
 *   caja: 2 recursos
 *   [hola desde dentro de la caja] 28
 *   numeros: 1 2 3 4 5 6 7 8
 *   CAJA: las cuatro pruebas pasan
 *
 * Si sale `caja: no es un paquete`, hay tres causas y se distinguen en F11:
 *   - el kernel no recordo la ruta  -> `[paquete] la ruta no cabe` o nada
 *   - la ruta no se pudo abrir      -> el motivo de `archivo::open`
 *   - se abrio pero no hay seccion  -> alguien desplego el `.bex` SIN empaquetar
 */
#include <bmo/paquete.h>

int main() {
    PAQUETE *p;
    char *b;
    unsigned long long n;
    int pasan;
    int i;

    pasan = 0;

    /* 1 - La caja se abre SIN decir donde esta. */
    p = paquete_mio();
    if (p == 0) {
        printf("caja: no es un paquete\n");
        return 1;
    }
    printf("caja: %d recursos\n", (int)paquete_cuantos(p));
    if (paquete_cuantos(p) == 2) { pasan = pasan + 1; }

    /* 2 - Un recurso de texto, entero. */
    b = (char *)malloc(256);
    if (b == 0) { printf("caja: sin memoria\n"); return 1; }
    n = paquete_leer(p, "saludo.txt", b, 256);
    b[n] = 0;
    printf("[%s] %d\n", b, (int)n);
    if (n > 0) { pasan = pasan + 1; }

    /* 3 - Uno binario, y se mira el CONTENIDO. Un recurso que llega con el
     * tamano correcto y los bytes de otro sitio se ve igual desde fuera. */
    n = paquete_leer(p, "cuenta.bin", b, 256);
    printf("numeros:");
    i = 0;
    while (i < (int)n) {
        printf(" %d", (int)b[i] & 0xFF);
        i = i + 1;
    }
    printf("\n");
    if (n == 8 && (b[0] & 0xFF) == 1 && (b[7] & 0xFF) == 8) { pasan = pasan + 1; }

    /* 4 - Y lo que NO esta contesta cero, no basura. */
    n = paquete_leer(p, "no-existe", b, 256);
    if (n == 0) { pasan = pasan + 1; }

    if (pasan == 4) {
        printf("CAJA: las cuatro pruebas pasan\n");
    } else {
        printf("CAJA: pasan %d de 4\n", pasan);
    }
    return 0;
}
