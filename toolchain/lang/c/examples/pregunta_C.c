/* pregunta_C.bex -- el primer programa en C que PREGUNTA.
 *
 * Hasta hoy un programa en C de BMO solo podia hablar. `printf` bajaba a la
 * puerta de consola y ahi se acababa la conversacion: no habia forma de leer
 * una tecla, asi que ningun programa podia pedir un dato. COBOL tenia `ACCEPT`
 * desde hacia semanas; C no tenia nada.
 *
 * == Como se lanza ==
 *
 * Desde la caja del compositor:  c/pregc.bex
 *
 * Y entonces se escribe EN LA CAJA y se pulsa Enter: lo que se teclea llega a
 * este programa por su consola. Ese es el circuito entero --el terminal escribe
 * en la consola del hijo, el hijo la lee-- y es la primera vez que se recorre
 * desde C.
 *
 * * Ojo con `getchar`: en BMO **nunca devuelve EOF**. Una consola no se acaba,
 *   se queda esperando. Un `while ((c = getchar()) != EOF)` copiado de un libro
 *   gira para siempre. Aqui se corta con el salto de linea, que es lo que hay.
 *
 * Compilar:
 *   cargo run -p bmo-c-front -- toolchain/lang/c/examples/pregunta_C.c \
 *       -o Ultra_kernel_x86-64/kernel/src/ring0/pregunta_C.bex
 */

int main() {
    int edad;
    int c;
    int letras;
    char nombre[32];

    printf("como te llamas? ");
    scanf("%s", nombre);
    printf("hola, %s\n", nombre);

    printf("cuantos anos tienes? ");
    scanf("%d", &edad);
    /* Aritmetica sobre lo leido: si el parseo devolviera basura, esto lo
     * ensena. Un eco solo no distingue "lo lei" de "lo copie". */
    printf("en 10 anos tendras %d\n", edad + 10);

    /* Y byte a byte, que es el otro camino. Se cuentan las letras en vez de
     * repetirlas: contar prueba que llegaron TODAS, y ahi es donde se veria si
     * el buffer perdiera las seis que sobran de cada paquete. */
    printf("escribe algo y cuento sus letras: ");
    letras = 0;
    for (;;) {
        c = getchar();
        if (c == 10) {
            break;
        }
        letras = letras + 1;
    }
    printf("%d letras\n", letras);

    printf("listo.\n");
    return 0;
}
