/* prestado_C.c -- memoria que viaja SIN COPIARSE, las dos direcciones.
 *
 * == Lo que ensena ==
 *
 * `<bmo/prestado.h>`, que llego el 2026-09-02 y tapo la mitad que faltaba del
 * zero copy: REX publicaba `MEM_OP_OFRECER` --dentro de `<bmo/superficie.h>`--
 * y no publicaba `TASK_OP_TOMAR` ni `PRESTADO_OP_*`. Una app de C podia
 * PRESTAR memoria y no podia RECIBIRLA.
 *
 * == ** POR QUE ESTE EJEMPLO NO PUEDE ENSENAR EL CICLO ENTERO ==
 *
 * Porque el kernel **prohibe prestarse a uno mismo**, y con razon:
 *
 *     if destino == owner { return false; }      obj/loan.rs
 *
 * Un prestamo a uno mismo mapearia las mismas paginas fisicas dos veces en el
 * mismo espacio de direcciones. No es util y es una forma barata de tener dos
 * punteros vivos a lo mismo sin que nadie lo sepa.
 *
 * Asi que un solo `.bex` no puede ser las dos puntas. Este programa hace las
 * dos cosas que SI se pueden hacer solo, y dice honestamente lo que ve:
 *
 *   1. **prestar a quien me lanzo** -- es el caso real: una app le presta sus
 *      pixeles al DIRECTOR. Si no hay padre, se dice y no se finge.
 *   2. **tomar lo que haya para mi** -- y contar cuanto y de quien.
 *
 * == [!] Y lo que este ejemplo destapa, que no estaba en el plan ==
 *
 * Para prestarle a un HIJO hace falta su TID, y el unico TID que un programa de
 * C puede conseguir hoy es el de su PADRE (`BMO_OP_MI_PADRE`). `TAREA_OP_TID`
 * existe en el ABI y **no esta en REX**.
 *
 * O sea que el zero copy sigue teniendo una punta corta, y no es la de tomar:
 * es la de **saber a quien prestar**. Eso es `<bmo/tarea.h>`, que el plan habia
 * bajado de prioridad por otra razon.
 *
 * == Compilar ==
 *
 *     bmo-c-front toolchain/lang/c/examples/prestado_C.c -o prestado.bex
 */
#include <bmo/bmo.h>
#include <bmo/prestado.h>

#define CAJA 4096

int main() {
    BMO_PRESTADO p;
    unsigned long long padre;
    unsigned long long r;
    char *caja;
    int i;
    int dueno;

    printf("prestado_C: el zero copy, las dos direcciones\n");

    /* -- 1. PRESTAR ---------------------------------------------------- */

    caja = (char *)malloc(CAJA);
    if (caja == 0) {
        printf("prestado_C: sin monton, no hay nada que prestar\n");
        return 1;
    }
    /* Se marca para que, si alguien la toma, se vea que son ESTOS bytes y no
     * una copia: quien la reciba lee lo mismo sin que nadie haya copiado. */
    i = 0;
    while (i < 16) {
        caja[i] = (char)(0x40 + i);
        i = i + 1;
    }

    padre = bmo_mi_padre();
    if (padre == 0) {
        /* No es un fallo: es haberse lanzado desde el shell de Ring 0, que no
         * compone nada. Se dice y se sigue. */
        printf("prestado_C: no hay padre a quien prestar (lanzado desde el shell)\n");
    } else {
        r = bmo_prestar_del_monton(caja, CAJA, padre);
        if (r == 0) {
            printf("prestado_C: ofrecidos %d bytes al TID %llu\n", CAJA, padre);
        } else {
            printf("prestado_C: el prestamo NO se acepto (codigo %llu)\n", r);
        }
    }

    /* -- 2. TOMAR ------------------------------------------------------ */

    if (bmo_prestado_tomar(&p) == 0) {
        /* ** Y ESTO NO ES UN FALLO. `TOMAR` contesta 0 cuando no hay nada para
         * ti, que es lo normal si nadie te ofrecio nada. Lo que seria un fallo
         * es no comprobarlo: `p.bytes_ptr` vale 0 y escribir ahi es un #PF que
         * no se parece en nada a "no habia nada". */
        printf("prestado_C: nadie me ha ofrecido nada. Correcto si nadie presto.\n");
        printf("prestado_C: hecho.\n");
        return 0;
    }

    dueno = bmo_prestado_dueno(&p);
    printf("prestado_C: recibidos %llu bytes del TID %d\n", p.bytes, dueno);

    /* Se lee con la comprobacion puesta: el tamano lo eligio OTRO proceso, asi
     * que suponerlo es peor que suponer el propio. */
    if (bmo_prestado_cabe(&p, 0)) {
        printf("prestado_C: primer byte 0x%02x\n", (int)p.bytes_ptr[0]);
    }

    /* ** Se PREGUNTA otra vez, no se reutiliza el `dueno` de arriba: entre las
     * dos lineas el otro proceso puede haberse muerto, y eso es exactamente lo
     * que esta pregunta existe para ver. */
    if (bmo_prestado_vive(&p) == 0) {
        printf("prestado_C: el que me lo presto YA NO VIVE. Suelto y salgo.\n");
    }

    /* -- 3. Y DEVOLVERLO, que no es opcional --------------------------- */

    if (bmo_prestado_soltar(&p) == 0) {
        printf("prestado_C: NO se pudo soltar\n");
        return 1;
    }
    printf("prestado_C: devuelto. Sigo vivo.\n");
    return 0;
}
