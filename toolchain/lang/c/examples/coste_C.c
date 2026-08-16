/* coste_C.bex -- HOW MANY CYCLES DOES A DOOR COST.
 *
 * Written 2026-08-16 to settle an argument with a number instead of an
 * estimate, following the rule already written in `PLAN_BANCA.md`: a hard NO is
 * earned by MEASURING, not by reasoning.
 *
 * == FIRST RESULT, on the Ryzen, 2026-08-16 ==
 *
 *     TSC 3700000000 Hz, lote 4096, vueltas 16
 *     1. bucle vacio   min 43 ciclos/op, media 45
 *     2. llamada       min 65 ciclos/op, media 67
 *     3. puerta minima min 2615 ciclos/op, media 6287
 *     4. puerta con handle: NO SE MIDIO
 *
 * A door costs ~2570 cycles net, about 707 ns. A call costs ~20. The ratio is
 * ~120x. That is the number the whole "can this be a capability operation?"
 * argument needed, and it did not exist anywhere in the tree before.
 *
 * ** TWO DEFECTS OF THAT FIRST VERSION, FIXED HERE. Both were mine:
 *
 *   1. The `if (which == ...)` chain was INSIDE the timed loop, so every case
 *      paid a different number of comparisons and the "empty loop" baseline of
 *      43 cycles was really the chain. It cost the door number nothing (43 is
 *      noise against 2615) but it made the CALL number soft, and the call is
 *      the thing the door is compared against. Now each case has its own tight
 *      loop and there is nothing to subtract but the loop.
 *   2. Row 4 opened `datos/salida.txt`, which **a program does not create** --
 *      the `guarda` command does, afterwards. On a fresh boot it is not there
 *      yet, so the honest number never got measured. Now it asks the kernel for
 *      THIS program's own image (`BMO_OP_MI_PAQUETE`): no path to get right,
 *      and it always exists for a `.bex` loaded from disk.
 *
 * == The question this answers ==
 *
 * Every design decision that asks "can this go through INVOKE, or does it have
 * to be linked code?" needs one number: what a door costs compared to a plain
 * call.
 *
 * It decides at least four open questions:
 *
 *   - Python: can the object model be capability operations, or must the
 *     runtime be linked code? (`docs/PYTHON_MAESTRO.md`)
 *   - Audio: how often the bus can be polled before the door is the cost.
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
 *   4. door + handle   `BMO_ARCH_TAMANO` on a real capability. This is the
 *                      HONEST number: it pays handle resolution, the
 *                      generation check and the capability table lookup, which
 *                      is what every real operation pays.
 *
 * The gap between 3 and 4 is the price of a capability, and nobody has ever
 * seen it.
 *
 * == Why the MINIMUM and not the average ==
 *
 * The scheduler is preemptive AND it only switches tasks at a trap boundary --
 * so every syscall is itself a scheduling opportunity. The minimum over many
 * rounds is the only value that cannot be inflated by that.
 *
 * ** And the average is printed too, on purpose. In the first run rows 1 and 2
 * were within 5% of their minimum and row 3 was 2.4x its own -- so the gap is
 * not noise, it is what happens to a block long enough to be interrupted. That
 * gap IS the preemption, and it is a second useful number.
 *
 * == What cannot distort this ==
 *
 * BMO C has no optimizer, so nothing here is folded away or hoisted out of a
 * loop. That is normally a cost; for a benchmark it is a guarantee.
 *
 * == How it is launched ==
 *
 *   run c/coste.bex        from the Ring 0 shell, or from the desktop box.
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

/* Shared reporting, so the four measurements cannot disagree on the
 * arithmetic. Takes totals, not loops -- the loops stay tight. */
void report(char *label, unsigned long long best, unsigned long long total) {
    printf("%s min %llu ciclos/op, media %llu\n",
           label,
           best / BATCH,
           (total / ROUNDS) / BATCH);
}

/* ** THE SPLIT: how much of a door is the kernel's Rust, and how much is the
 * assembly stub.
 *
 * The kernel counts, since boot, how many doors it has served and how many
 * cycles it spent inside `dispatch`. Read as a DELTA around a block, that gives
 * the Rust half; the total measured out here minus that is the stub half --
 * pushes, `xsave64`, `xrstor64` and `iretq`.
 *
 * That subtraction is the whole point. Without it, changing the stub would be
 * surgery on a guess, and the stub is the code that produced the `#GP` in
 * `xrstor`.
 *
 * [!] The two readings are themselves two doors, and the kernel counts them.
 * Over a block of tens of thousands that is under a tenth of a percent.
 */
void report_split(unsigned long long doors0, unsigned long long cycles0,
                  unsigned long long total_per_op) {
    unsigned long long doors;
    unsigned long long cycles;
    unsigned long long inside;

    doors = bmo_info(BMO_INFO_SYSCALL_CUENTA) - doors0;
    cycles = bmo_info(BMO_INFO_SYSCALL_CICLOS) - cycles0;

    if (doors == 0) {
        printf("   REPARTO: el kernel no conto ni una puerta\n");
        return;
    }
    inside = cycles / doors;
    printf("   reparto: dentro de dispatch %llu, en el stub %llu\n",
           inside,
           total_per_op - inside);
}

int main() {
    unsigned long long hz;
    unsigned long long handle;
    unsigned long long best;
    unsigned long long total;
    unsigned long long start;
    unsigned long long elapsed;
    unsigned long long sink;
    unsigned long long doors0;
    unsigned long long cycles0;
    int round;
    int i;

    printf("COSTE: cuanto vale una puerta\n");

    hz = bmo_info(BMO_INFO_TSC_HZ);
    printf("TSC %llu Hz, lote %d, vueltas %d\n", hz, BATCH, ROUNDS);

    /* -- 1. the loop itself ------------------------------------------- */
    best = 0; total = 0; sink = 0;
    for (round = 0; round < ROUNDS; round++) {
        start = __rdtsc();
        for (i = 0; i < BATCH; i++) {
            sink = sink + 1;
        }
        elapsed = __rdtsc() - start;
        if (best == 0 || elapsed < best) { best = elapsed; }
        total = total + elapsed;
    }
    report("1. bucle vacio  ", best, total);

    /* -- 2. a plain call ---------------------------------------------- */
    best = 0; total = 0;
    for (round = 0; round < ROUNDS; round++) {
        start = __rdtsc();
        for (i = 0; i < BATCH; i++) {
            sink = plain_call(sink);
        }
        elapsed = __rdtsc() - start;
        if (best == 0 || elapsed < best) { best = elapsed; }
        total = total + elapsed;
    }
    report("2. llamada      ", best, total);

    /* -- 3. the bare door: no handle to resolve ----------------------- */
    doors0 = bmo_info(BMO_INFO_SYSCALL_CUENTA);
    cycles0 = bmo_info(BMO_INFO_SYSCALL_CICLOS);
    best = 0; total = 0;
    for (round = 0; round < ROUNDS; round++) {
        start = __rdtsc();
        for (i = 0; i < BATCH; i++) {
            sink = sink + bmo_valor(BMO_TAREA_ACTUAL, BMO_OP_PID, 0, 0, 0);
        }
        elapsed = __rdtsc() - start;
        if (best == 0 || elapsed < best) { best = elapsed; }
        total = total + elapsed;
    }
    report("3. puerta pelada", best, total);
    report_split(doors0, cycles0, best / BATCH);

    /* -- 4. a real capability ----------------------------------------- *
     *
     * The handle is THIS program's own image. No path is written, so there is
     * no path to get wrong -- it is the difference between asking by NAME and
     * holding by RIGHT. Returns 0 for binaries the kernel embeds; a `.bex`
     * launched with `run` always has one.
     */
    handle = bmo_valor(BMO_TAREA_ACTUAL, BMO_OP_MI_PAQUETE, 0, 0, 0);
    if (handle == 0) {
        printf("4. puerta+handle: NO SE MIDIO, el kernel no recuerda mi imagen\n");
    } else {
        doors0 = bmo_info(BMO_INFO_SYSCALL_CUENTA);
        cycles0 = bmo_info(BMO_INFO_SYSCALL_CICLOS);
        best = 0; total = 0;
        for (round = 0; round < ROUNDS; round++) {
            start = __rdtsc();
            for (i = 0; i < BATCH; i++) {
                sink = sink + bmo_valor(handle, BMO_ARCH_TAMANO, 0, 0, 0);
            }
            elapsed = __rdtsc() - start;
            if (best == 0 || elapsed < best) { best = elapsed; }
            total = total + elapsed;
        }
        report("4. puerta+handle", best, total);
        report_split(doors0, cycles0, best / BATCH);
        bmo_codigo(handle, BMO_ARCH_CERRAR, 0, 0, 0);
    }

    /* Printed so nothing here can ever be considered dead, and so a wrong
     * value is visible instead of silent. */
    if (sink == 0) {
        printf("AVISO: el acumulador quedo en cero\n");
    }

    printf("COSTE: la fila 3 menos la 1 = la puerta desnuda\n");
    printf("COSTE: la 4 menos la 3 = lo que cuesta resolver un handle\n");
    return 0;
}
