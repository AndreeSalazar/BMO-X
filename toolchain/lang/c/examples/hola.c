/* El primer "hola mundo" en C que BMO-X ejecuta de verdad.
 *
 * No hay libc, no hay POSIX, no hay Win32. `printf` baja a la puerta
 * generica (bmo-lower) y de ahi al unico syscall que existe:
 *
 *     INVOKE(CURRENT_TASK, CONSOLE_WRITE, 8 bytes por valor)
 *
 * El formateo se emite EN LINEA: no hay runtime que enlazar ni simbolo que
 * resolver. Cada trozo literal viaja como inmediatos dentro de las
 * instrucciones; cada %d convierte el numero ahi mismo.
 *
 * Compilar:
 *     cargo run -p bmo-c-front -- toolchain/lang/c/examples/hola.c
 */
enum Estado { ARRANCANDO, LISTO, APAGANDO };

int main() {
    printf("BMO-X: hola mundo desde C\n");

    int cuenta = 3;
    int total = cuenta * 14;
    printf("cuenta=%d total=%d resto=%d\n", cuenta, total, total % 5);

    /* Antes esto imprimia el numero equivocado: la resta emitia b - a. */
    printf("42 - 100 = %d\n", 42 - 100);

    /* Las constantes de enum valen lo que dicen (antes valian nada). */
    printf("estado LISTO = %d de %d\n", LISTO, APAGANDO);

    printf("hex=%x char=%c texto=%s\n", 48879, 66, "cadena");

    if (total > cuenta) {
        printf("C -> puerta L1 -> INVOKE -> Ring 0\n");
    }
    return 0;
}
