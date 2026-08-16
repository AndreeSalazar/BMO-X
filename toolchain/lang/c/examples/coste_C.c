/* coste_C.bex -- HOW MANY CYCLES DOES A DOOR COST.
 *
 * Written 2026-08-16 to settle an argument with a number instead of an
 * estimate, following the rule already written in `PLAN_BANCA.md`: a hard NO is
 * earned by MEASURING, not by reasoning.
 *
 * == The question this answers ==
 *
 * Every design decision that asks "can this go through INVOKE, or does it have
 * to be a library?" needs one number: what a door costs compared to a plain
 * call. Today that number does not exist anywhere in the tree -- it has been
 * assumed, never measured.
 *
 * It decides at least four open questions:
 *
 *   - Python: can the object model be capability operations, or must the
 *     runtime be linked code? (`docs/PYTHON_MAESTRO.md`)
 *   - Audio: how often can the bus be polled before the door itself is the
 *     cost. (`docs/AUDIO_MAESTRO.md`)
 *   - Disk: what the asynchronous path actually saves per chunk.
 *   - Net: whether the data path can go through the kernel at all.
 *
 * == What is measured, and why these four ==
 *
 *   1. empty loop      the loop itself, so its cost can be subtracted.
 *   2. plain call      a normal function call. The thing a door is compared to.
 *   3. bare door       `BMO_OP_PID` on `BMO_TAREA_ACTUAL`. This is the FLOOR:
 *                      the current task is a special-cased pseudo-capability,
 *                      so no handle table is walked. Nothing can be cheaper.
 *   4. door + handle   `BMO_ARCH_TAMANO` on a real open file. This is the
 *                      HONEST number: it pays handle resolution, the
 *                      generation check and the capability table lookup, which
 *                      is what any real operation pays.
 *
 * The gap between 3 and 4 is the price of a capability, and nobody has ever
 * seen it.
 *
 * == Why the MINIMUM and not the average ==
 *
 * The scheduler is preemptive: the LAPIC timer can land in the middle of a
 * measured block and charge another task's time to this one. The minimum over
 * many rounds is the only value that cannot be inflated that way.
 *
 * ** And the average is printed too, on purpose: if it is much larger than the
 * minimum, that gap IS the preemption, and that is a second useful number --
 * it says how often a Ring 3 loop gets interrupted.
 *
 * == What cannot distort this ==
 *
 * BMO C has no optimizer, so nothing here is folded away or hoisted out of a
 * loop. That is normally a cost; for a benchmark it is a guarantee.
 *
 * == How it is launched ==
 *
 *   run c/coste.bex        from the Ring 0 shell, or from the desktop box.
 *
 * The file handle needs `datos/salida.txt` to exist, same as `leer.bex`. If it
 * does not, measurement 4 is skipped and SAID -- a missing number is reported,
 * never guessed.
 */

#include <bmo/bmo.h>
#include <bmo/archivo.h>
#include <stdio.h>

/* Calls inside one timed block. Big enough that the two `__rdtsc()` reads and
 * any out-of-order skew around them are noise; small enough that a preemption
 * does not hit most blocks. */
#define BATCH 4096

/* Timed blocks per measurement. The minimum of these is the answer. */
#define ROUNDS 16

/* The thing a door is compared against. Deliberately trivial: what is being
 * measured is the CALL, not the work. */
unsigned long long plain_call(unsigned long long x) {
    return x + 1;
}

/* Runs one measurement `ROUNDS` times and reports minimum and average cycles
 * per single operation.
 *
 * `which` picks the body. A function pointer would be cleaner, but it would add
 * an indirect call to every case -- including the baseline, which exists
 * precisely to have nothing added to it.
 */
void measure(int which, char *label, unsigned long long handle) {
    unsigned long long best;
    unsigned long long total;
    unsigned long long start;
    unsigned long long stop;
    unsigned long long elapsed;
    unsigned long long sink;
    int round;
    int i;

    best = 0;
    total = 0;
    sink = 0;

    for (round = 0; round < ROUNDS; round++) {
        start = __rdtsc();
        for (i = 0; i < BATCH; i++) {
            if (which == 0) {
                /* empty: only the loop */
                sink = sink + 1;
            } else if (which == 1) {
                sink = plain_call(sink);
            } else if (which == 2) {
                sink = sink + bmo_valor(BMO_TAREA_ACTUAL, BMO_OP_PID, 0, 0, 0);
            } else {
                sink = sink + bmo_valor(handle, BMO_ARCH_TAMANO, 0, 0, 0);
            }
        }
        stop = __rdtsc();
        elapsed = stop - start;
        if (best == 0) {
            best = elapsed;
        }
        if (elapsed < best) {
            best = elapsed;
        }
        total = total + elapsed;
    }

    printf("%s min %llu ciclos/op, media %llu\n",
           label,
           best / BATCH,
           (total / ROUNDS) / BATCH);

    /* `sink` is printed so that nothing here can ever be considered dead, and
     * so a wrong value is visible instead of silent. */
    if (sink == 0) {
        printf("  AVISO: el acumulador quedo en cero\n");
    }
}

int main() {
    unsigned long long hz;
    unsigned long long handle;

    printf("COSTE: cuanto vale una puerta\n");

    hz = bmo_info(BMO_INFO_TSC_HZ);
    printf("TSC %llu Hz, lote %d, vueltas %d\n", hz, BATCH, ROUNDS);

    measure(0, "1. bucle vacio  ", 0);
    measure(1, "2. llamada      ", 0);
    measure(2, "3. puerta minima", 0);

    /* 4 -- the real capability. Needs a handle, and the only handle a C program
     * can hold today is a file. That is itself worth writing down: there is no
     * way to get a raw `KIND_MEMORIA` handle from C, only the pointer. */
    handle = bmo_abrir("datos/salida.txt");
    if (handle == 0) {
        printf("4. puerta con handle: NO SE MIDIO, no abre datos/salida.txt\n");
    } else {
        measure(3, "4. puerta+handle", handle);
        bmo_codigo(handle, BMO_ARCH_CERRAR, 0, 0, 0);
    }

    printf("COSTE: leer la fila 3 menos la 1 = la puerta desnuda\n");
    printf("COSTE: la 4 menos la 3 = lo que cuesta resolver un handle\n");
    return 0;
}
