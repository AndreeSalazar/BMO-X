/* pregunta_C.bex — el primer programa en C que PREGUNTA.
 *
 * Hasta hoy un programa en C de BMO sólo podía hablar. `printf` bajaba a la
 * puerta de consola y ahí se acababa la conversación: no había forma de leer
 * una tecla, así que ningún programa podía pedir un dato. COBOL tenía `ACCEPT`
 * desde hacía semanas; C no tenía nada.
 *
 * ══ Cómo se lanza ══
 *
 * Desde la caja del compositor:  apps/pregc.bex
 *
 * Y entonces se escribe EN LA CAJA y se pulsa Enter: lo que se teclea llega a
 * este programa por su consola. Ése es el circuito entero —el terminal escribe
 * en la consola del hijo, el hijo la lee— y es la primera vez que se recorre
 * desde C.
 *
 * ★ Ojo con `getchar`: en BMO **nunca devuelve EOF**. Una consola no se acaba,
 *   se queda esperando. Un `while ((c = getchar()) != EOF)` copiado de un libro
 *   gira para siempre. Aquí se corta con el salto de línea, que es lo que hay.
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
    /* Aritmética sobre lo leído: si el parseo devolviera basura, esto lo
     * enseña. Un eco solo no distingue "lo lei" de "lo copie". */
    printf("en 10 anos tendras %d\n", edad + 10);

    /* Y byte a byte, que es el otro camino. Se cuentan las letras en vez de
     * repetirlas: contar prueba que llegaron TODAS, y ahí es donde se vería si
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
