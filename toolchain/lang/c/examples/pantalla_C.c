/* pantalla_C.c -- tomar la pantalla, usarla, y DEVOLVERLA.
 *
 * == Que ensena, y por que hacia falta ==
 *
 * Dos cosas que hasta el 2026-09-01 no se podian ensenar:
 *
 *   1. `<bmo/pantalla.h>`, que no existia. Los programas que tomaban el panel
 *      entero se copiaban los cuatro numeros del kernel a mano.
 *   2. `<bmo/entrada.h>` **con un ejemplo**. Era el unico hueco del indice de
 *      REX -- `README.md` lo decia desde el 19-08 con un aviso: *"ninguno"*.
 *
 * Y las dos van juntas a proposito, porque el caso real es uno solo: quien toma
 * la pantalla necesita el teclado para poder salir. El kernel lo aprendio a
 * golpes -- *"separarlas fue el bug: prestar la pantalla sin la entrada dejo a
 * `ray.bex` pintando sin poder leer su propio ESC, y a la maquina sin teclado"*.
 *
 * == ** LO QUE DE VERDAD DEMUESTRA: que se puede DEVOLVER ==
 *
 * Los otros ejemplos que toman la pantalla la sueltan de la unica forma que
 * habia: muriendose. Este sale por ESC, devuelve las dos capabilities, **sigue
 * vivo** y lo dice por consola. Si al terminar se ve el escritorio y el teclado
 * responde, el camino entero funciono.
 *
 * == Compilar ==
 *
 *     bmo-c-front toolchain/lang/c/examples/pantalla_C.c -o pantalla.bex
 *
 * con el `cwd` en la raiz del repo -- `Roots::find` sube desde ahi.
 */
#include <bmo/bmo.h>
#include <bmo/pantalla.h>
#include <bmo/entrada.h>

/* Un tablero de ajedrez, que es el dibujo mas barato que demuestra que el
 * `paso` se esta usando bien: si se indexara con `ancho` en un panel con
 * relleno, los cuadros saldrian inclinados en vez de rectos. */
#define LADO 32

/* Los colores, en el formato que hoy contesta esta maquina: 32 bits con el
 * byte alto sin usar. Ver la nota de `formato` en <bmo/pantalla.h>: el campo se
 * publica para poder MIRARLO, no porque haya un convertidor detras. */
#define CLARO  0x00203040
#define OSCURO 0x00101820
#define BARRA  0x0000E5FF

void tablero(BMO_PANTALLA *p) {
    int x;
    int y;
    unsigned int c;

    y = 0;
    while (y < p->alto) {
        x = 0;
        while (x < p->ancho) {
            if (((x / LADO) + (y / LADO)) & 1) {
                c = CLARO;
            } else {
                c = OSCURO;
            }
            /* El `paso` y no el `ancho`. Son distintos en cuanto el panel
             * tiene relleno al final de la fila. */
            p->pixeles[y * p->paso + x] = c;
            x = x + 1;
        }
        y = y + 1;
    }
}

/* Una barra que crece con cada tecla, para que se vea que la entrada llega.
 *
 * Se dibuja con `bmo_pantalla_cabe` en vez de con un `if` a mano: es la misma
 * comprobacion, hecha en el sitio donde esta escrita una vez. La fila 200 de
 * DOOM salio de tener ese `if` copiado en dos sitios y mal en los dos. */
void marca(BMO_PANTALLA *p, int n) {
    int i;
    int j;
    int x;

    i = 0;
    while (i < n && i < 64) {
        x = 16 + i * 6;
        j = 0;
        while (j < 24) {
            if (bmo_pantalla_cabe(p, x, 16 + j)) {
                p->pixeles[(16 + j) * p->paso + x] = BARRA;
            }
            j = j + 1;
        }
        i = i + 1;
    }
}

int main() {
    BMO_PANTALLA p;
    unsigned long long ent;
    int tecla;
    int teclas;

    printf("pantalla_C: pido la pantalla\n");

    /* ** SE COMPRUEBA, y no es formalidad. La pantalla tiene UN dueno: si el
     * DIRECTOR esta vivo, esto contesta 0 y quien no lo mire escribe en la
     * direccion 0 -- un fallo de pagina que no se parece en nada a "no te
     * toca". */
    if (bmo_pantalla_abrir(&p) == 0) {
        printf("pantalla_C: la tiene otro proceso. No hay donde dibujar.\n");
        return 1;
    }
    printf("pantalla_C: %d x %d, paso %d px, formato %d, %llu bytes\n",
           p.ancho, p.alto, p.paso, p.formato, p.bytes);

    /* ** Y SIN TECLADO NO SE ARRANCA, por la misma razon que `raycaster_C`: un
     * programa a pantalla completa sin forma de leer su propio ESC deja la
     * maquina sin salida. Se devuelve lo que ya se habia tomado antes de irse
     * -- que es justo lo que este ejemplo viene a demostrar. */
    ent = bmo_entrada_reclamar();
    if (ent == 0) {
        printf("pantalla_C: sin teclado no arranco (no podria salir)\n");
        bmo_pantalla_cerrar(&p);
        return 1;
    }

    tablero(&p);
    teclas = 0;

    for (;;) {
        tecla = bmo_entrada_tecla(ent);
        if (tecla >= 0) {
            if (tecla == BMO_SC_ESC) {
                break;
            }
            teclas = teclas + 1;
            marca(&p, teclas);
        }
        /* Ceder, que es la forma normal de esperar aqui: las lecturas de
         * entrada NO bloquean, asi que un bucle que no cede se come el turno
         * entero preguntando. */
        bmo_ceder();
    }

    /* ** EL FINAL, QUE ES EL EJEMPLO.
     *
     * Primero la entrada y luego la pantalla: si algo fallara al soltar la
     * pantalla, al menos el teclado ya volvio y la maquina se puede usar para
     * averiguar que paso. */
    if (bmo_entrada_soltar() != 0) {
        printf("pantalla_C: la entrada NO se solto\n");
    }
    if (bmo_pantalla_cerrar(&p) == 0) {
        printf("pantalla_C: la pantalla NO se solto\n");
        return 1;
    }

    /* Si esto se lee en la consola del escritorio, el camino entero funciono:
     * se tomo la pantalla, se pinto, se devolvio, y el proceso SIGUE VIVO. */
    printf("pantalla_C: devueltas las dos. %d teclas. Sigo vivo.\n", teclas);
    return 0;
}
