/* hola_C.bex — el programa que BMO-X ejecuta en el Ryzen 5 5600X real.
 *
 * No es un "hola mundo" de adorno: cada linea prueba algo que estaba ROTO
 * hasta esta semana y que solo se puede confirmar en metal.
 *
 *   - el bucle    : ningun `for` de C daba mas de una vuelta
 *   - el %d       : printf llamaba a un simbolo que nadie resuelve
 *   - la resta    : `42 - 100` daba +58
 *   - el switch   : entraba siempre por el primer caso
 *   - el %s       : las cadenas viven en otra seccion, y el cargador la
 *                   pone en la pagina siguiente (por eso el codegen rellena)
 *
 * Instrucciones x86-64 base. Nada de BMI, LZCNT ni TSX: AMD Zen 3 lo corre
 * igual que un Intel, que es justo lo que se quiere de la esencia de C.
 *
 * Compilar:
 *   cargo run -p bmo-c-front -- toolchain/lang/c/examples/hola_C.c \
 *       -o Ultra_kernel_x86-64/kernel/src/ring0/hola_C.bex
 */
enum Fase { ARRANQUE, CALCULO, FIN };

int suma_hasta(int n) {
    int total = 0;
    for (int i = 1; i <= n; i = i + 1) {
        total = total + i;
    }
    return total;
}

int main() {
    printf("hola desde C en el Ryzen\n");

    /* Bucle + funcion + recursion de vuelta al kernel log. */
    printf("suma 1..10 = %d\n", suma_hasta(10));

    /* Resta y division con signo. */
    printf("42-100=%d  100/7=%d  100%%7=%d\n", 42 - 100, 100 / 7, 100 % 7);

    /* switch con default alcanzable. */
    int fase = CALCULO;
    switch (fase) {
        case ARRANQUE: printf("fase: arranque\n"); break;
        case CALCULO:  printf("fase: calculo\n"); break;
        default:       printf("fase: otra\n"); break;
    }

    /* Cadena en otra seccion: prueba el relleno a pagina en hardware. */
    printf("cadena=%s hex=%x\n", "viva", 48879);

    printf("C termino ok\n");
    return 0;
}
