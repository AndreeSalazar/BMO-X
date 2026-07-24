/* El primer "hola mundo" en C que BMO-X ejecuta de verdad.
 *
 * No hay libc, no hay POSIX, no hay Win32. `printf` con literal baja a la
 * puerta genérica (bmo-lower) y de ahí al único syscall que existe:
 *
 *     INVOKE(CURRENT_TASK, CONSOLE_WRITE, 8 bytes por valor)
 *
 * Compilar:
 *     cargo run -p bmo-c-front -- toolchain/lang/c/examples/hola.c
 */
int main() {
    printf("BMO-X: hola mundo desde C\n");
    printf("C -> puerta L1 -> INVOKE -> Ring 0\n");
    return 0;
}
