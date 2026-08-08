/* scroll_C.bex -- ver y hacer scroll, escrito en BMO C.
 *
 * == Que prueba esto ==
 *
 * El compositor ya guarda 200 filas y las recorre con la rueda y con
 * RePag/AvPag. Eso esta en Rust. Este programa hace lo mismo en C, y no es una
 * copia por gusto: es la prueba de que la rueda y las teclas sin glifo son
 * parte del **contrato** y no de un privilegio del compositor. Si solo se
 * pudiera leer la rueda desde Rust, no seria un sistema operativo -- seria un
 * programa con un sistema alrededor.
 *
 * Cada pieza que toca estaba rota o no existia hasta hoy:
 *
 *   - `#include <bmo/...>`  las cabeceras tiraban sus `#define`, asi que una
 *                           constante de cabecera llegaba sin expandir y el
 *                           codegen la ponia a CERO en silencio
 *   - `0xFFFFFFFFFFFFFFFE`  no cabia en un i64 y valia cero: la capability de
 *                           uno mismo no se podia nombrar
 *   - `__syscall(...)`      no existia: C no podia cruzar a Ring 0 mas que por
 *                           `printf`
 *   - la rueda              el kernel la tenia y la tiraba (no habia lector)
 *
 * == Como se usa en la maquina ==
 *
 *   rueda arriba / RePag  ->  hacia el pasado
 *   rueda abajo  / AvPag  ->  hacia lo ultimo
 *   Inicio / Fin          ->  a los extremos de una vez
 *   ESC                   ->  salir
 *
 * * Si el compositor esta corriendo, la entrada es SUYA y esto dira que no la
 *   pudo reclamar. No es un fallo: es la cesion funcionando. Para probarlo,
 *   lanzalo desde el shell de Ring 0.
 *
 * Compilar:
 *   cargo run -p bmo-c-front -- toolchain/lang/c/examples/scroll_C.c \
 *       -o Ultra_kernel_x86-64/kernel/src/ring0/scroll_C.bex
 */
#include <bmo/scroll.h>

#define FILAS 60
#define COLS 24
#define VISIBLES 8
#define ESC 27

/* El historial. Vive aqui y no en un heap porque no hay heap: 60 filas de 24
 * columnas son 1440 bytes de datos estaticos, y el tamano se sabe al compilar.
 * Un `malloc` aqui seria pedirle al sistema algo que el programa ya tiene. */
char hist[1440];

/* Escribe "fila NNN" en la fila `f`, con el cero al final para que `%s` sepa
 * donde parar. El resto se rellena de ceros: una fila con basura detras se
 * imprimiria hasta el primer cero que hubiera por ahi. */
void poner(int f, int n) {
    int base;
    int i;
    base = f * COLS;
    hist[base] = 'f';
    hist[base + 1] = 'i';
    hist[base + 2] = 'l';
    hist[base + 3] = 'a';
    hist[base + 4] = ' ';
    hist[base + 5] = '0' + (n / 100) % 10;
    hist[base + 6] = '0' + (n / 10) % 10;
    hist[base + 7] = '0' + n % 10;
    i = 8;
    while (i < COLS) {
        hist[base + i] = 0;
        i = i + 1;
    }
}

/* Pinta la ventana: `VISIBLES` filas a partir de la que toque segun `vista`.
 *
 * El aviso de "historial" no es un adorno. Una ventana que ensena el pasado sin
 * decirlo se confunde con una que se ha colgado, y la reaccion normal a eso es
 * reiniciar la maquina -- que en un sistema que arranca desde un USB cuesta un
 * minuto y la sesion entera. */
void pintar(int vista) {
    int primera;
    int i;
    char *p;
    primera = bmo_scroll_primera(vista, FILAS, VISIBLES);
    printf("---- filas %d..%d ", primera, primera + VISIBLES - 1);
    if (bmo_scroll_en_historial(vista)) {
        printf("[historial] ----\n");
    } else {
        printf("[al dia] ----\n");
    }
    i = 0;
    while (i < VISIBLES) {
        p = hist + (primera + i) * COLS;
        printf("  %s\n", p);
        i = i + 1;
    }
}

int main() {
    unsigned long long ent;
    int vista;
    int giro;
    int tecla;
    int nueva;
    int i;

    i = 0;
    while (i < FILAS) {
        poner(i, i);
        i = i + 1;
    }

    ent = bmo_entrada_reclamar();
    if (ent == 0) {
        /* El caso NORMAL cuando el compositor esta vivo. Decirlo y salir es
         * mejor que quedarse en un bucle leyendo ceros: eso se ve igual que un
         * raton roto, y manda a depurar el USB sin motivo. */
        printf("la entrada es de otro proceso: no hay scroll que hacer.\n");
        return 0;
    }

    vista = 0;
    pintar(vista);

    for (;;) {
        nueva = vista;

        /* La rueda primero. Consume: lo que se lee aqui ya no vuelve, asi que
         * no hace falta guardar el valor anterior y restar -- que es donde se
         * cuela el scroll que se mueve solo. */
        giro = bmo_entrada_rueda(ent);
        if (giro != 0) {
            nueva = bmo_scroll_rueda(nueva, giro, FILAS, VISIBLES);
        }

        /* Las teclas se drenan hasta vaciar, no una por vuelta: pulsando
         * rapido llegan varias entre fotograma y fotograma, y quedarse con una
         * seria perder pulsaciones de forma que pareceria un teclado malo. */
        for (;;) {
            tecla = bmo_entrada_tecla(ent);
            if (tecla < 0) {
                break;
            }
            if (tecla == ESC) {
                printf("hasta luego.\n");
                return 0;
            }
            nueva = bmo_scroll_tecla(nueva, tecla, FILAS, VISIBLES);
        }

        /* Repintar solo cuando algo se movio. Repintar por fotograma llenaria
         * la consola de copias de lo mismo y haria ilegible justo lo que este
         * programa existe para dejar leer. */
        if (nueva != vista) {
            vista = nueva;
            pintar(vista);
        }

        /* Ceder es obligatorio, no cortesia: nada de lo de arriba bloquea, asi
         * que sin esto el bucle se come el quantum entero girando en vacio. */
        bmo_ceder();
    }
}
