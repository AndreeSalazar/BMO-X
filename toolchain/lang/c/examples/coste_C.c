/* coste_C.bex -- HOW MANY CYCLES DOES A DOOR COST.
 *
 * Written 2026-08-16 to settle an argument with a number instead of an
 * estimate, following the rule already written in `PLAN_BANCA.md`: a hard NO is
 * earned by MEASURING, not by reasoning.
 *
 * == ** EL RESULTADO DE LA v4, en el Ryzen, 2026-08-16 (PIEZA 1 PUESTA) ==
 *
 *     1. bucle vacio   min   44 ciclos/op, media   45
 *     2. llamada       min   64 ciclos/op, media   65
 *     3. puerta pelada min 1625 ciclos/op, media 4169
 *        reparto: dentro de dispatch 311, en el stub 1314
 *        el stub por dentro: guardar 30, devolver 30, resto 1254
 *     4. puerta+handle min 1967 ciclos/op, media 5435
 *        reparto: dentro de dispatch 396, en el stub 1571
 *        el stub por dentro: guardar 30, devolver 30, resto 1511
 *     5. rdtsc suelto  min  113 ciclos/op, media 1358
 *
 * **LA PUERTA PASO DE 2618 A 1625** -- y descontando los cuatro sellos (fila 5
 * menos fila 1 = 69 ciclos cada uno, ~276 en total) a **~1350: la mitad**. Las
 * dos casillas que la pieza 1 atacaba se desplomaron a 30 cada una y `dispatch`
 * se quedo en 311 contra 318/319: el control aguanta.
 *
 * ** DOS COSAS QUE ESTA TANDA ENSENO Y QUE NO ESTABAN EN EL PLAN:
 *
 *   1. **El instrumento cuesta el DOBLE de lo estimado.** Un `rdtsc` suelto son
 *      69 ciclos, no ~25. La fila 5 existia para que esto fuera una resta y no
 *      una suposicion, y menos mal.
 *   2. [!] **UNA ANOMALIA NUEVA Y SIN EXPLICAR.** Resolver un handle costaba
 *      +14 en el stub (ruido, como debe ser: el stub no sabe que operacion se
 *      pidio). Ahora cuesta **+257**. `dispatch` sube +85, que es correcto y es
 *      la capability. Los otros 257 no deberian existir. `guardar` y `devolver`
 *      salen identicos en las dos filas, asi que no son cambios de contexto.
 *      **No hay explicacion y no se inventa una**: queda anotado.
 *
 * == El resultado de la v3 (`xsaveopt64`, antes de la pieza 1) ==
 *
 *     TSC 3700000000 Hz, lote 4096, vueltas 16
 *     1. bucle vacio   min   43 ciclos/op, media   44
 *     2. llamada       min   63 ciclos/op, media   64
 *     3. puerta pelada min 2618 ciclos/op, media 6291
 *        reparto: dentro de dispatch  319, en el stub 2299
 *     4. puerta+handle min 2707 ciclos/op, media 6855
 *        reparto: dentro de dispatch  394, en el stub 2313
 *
 * ** LA v2 (con `xsave64`) dio 2663/318/2345 y 2746/394/2352. Cambiar a
 * `xsaveopt64` movio la puerta 45 ciclos -- **el 2%** -- y dejo `dispatch`
 * clavado en 318 -> 319. La mitad que no se toco no se movio: eso es lo que
 * hace creible que la otra si. **El sospechoso principal era inocente.**
 *
 * ** Y LA MEDIA, que tambien contesta algo. Las filas 1 y 2 tienen media 1,02x
 * su minimo; la fila 3 tiene **2,4x**. Un lote son 4096 x 2618 = 10,7 M ciclos
 * = 2,9 ms, y la media son 7 ms. Con `tareas listas 2` en el informe, el lote
 * minimo es el que cogio un quantum entero y la media es el que lo compartio.
 * **Esa diferencia es el planificador, no ruido por puerta** -- que es
 * exactamente el motivo por el que la respuesta es el minimo.
 *
 * A bare door costs 2663 - 43 = 2620 cycles net, about 708 ns. A call costs
 * 63 - 43 = 20. The ratio is 131x. That is the number the whole "can this be a
 * capability operation?" argument needed, and it did not exist in the tree.
 *
 * ** And the two numbers nobody had ever seen:
 *
 *   - RESOLVING A HANDLE COSTS 83 CYCLES (row 4 - row 3), and 76 of those
 *     appear INSIDE `dispatch` while 7 land in the stub -- noise, 0.3%. The
 *     capability model costs 76 cycles; the door it arrives through costs 2345.
 *     The stub does not know which operation was asked for, which is exactly
 *     what the design says should happen.
 *   - 88% OF A DOOR IS THE ASSEMBLY STUB. The Rust half -- resolve, dispatch,
 *     answer -- is 318 cycles. [!] `dispatch` is read as a MEAN and the total
 *     as a MINIMUM, so 2345 is a FLOOR: the real stub is >= that. The
 *     conclusion comes out reinforced, not weakened.
 *
 * == ** Y LA PREGUNTA QUE ABRE LA v3: EL STUB POR DENTRO ==
 *
 * Sumando a mano lo que hay en ese camino --`syscall` ~100, `swapgs` ~20, 20
 * pushes ~25, la cabecera ~10, el `xsaveopt64` ~150, el `xrstor64` ~200, 15
 * pops ~15, el `iretq` ~300-- salen **unos 700 de 2299**. Faltan 1.600 ciclos
 * que **no estan en la lista de sospechosos**: el modelo del camino esta
 * incompleto, no solo mal ordenado. Van dos veces que razonar sobre este stub
 * pierde contra el metro, asi que la v3 no trae una tercera hipotesis: trae
 * cuatro `rdtsc` mas DENTRO del stub y parte esos 2.299 en cuatro:
 *
 *     guardar    la cabecera a cero + el `xsaveopt64`
 *     dispatch   (ya se sabia: 319)
 *     devolver   las comprobaciones del sello + el `xrstor64`
 *     resto      el `syscall`, los pushes, los pops y el `iretq`
 *
 * ** `resto` ES LA CASILLA QUE DECIDE. Si es pequena, el coste esta en codigo
 * que se puede reescribir. Si se lleva los 1.600 que no cuadran, esta en las
 * DOS TRANSICIONES DE PRIVILEGIO -- y entonces afinar el stub no lo va a mover.
 * Lo que se mueve es `sysretq` en vez de `iretq` para el camino normal, o
 * agrupar llamadas, que es justo la pregunta de `docs/PYTHON_MAESTRO.md`.
 *
 * La fila 5 existe para poder restar lo que cobra ese instrumento.
 *
 * ** The v1 of this file, the same afternoon, gave 43 / 65 / 2615 and never
 * measured row 4. The two runs agree inside the noise, and the 48-cycle gap on
 * row 3 is exactly what `meter.rs` declares it costs (two `rdtsc`). The
 * thermometer charged what it said it would charge.
 *
 * ** AND A HYPOTHESIS OF MINE THAT THE v2 KILLED: I blamed the 43-cycle empty
 * loop on the `if` chain being inside it (defect 1 below). The chain is gone
 * and the loop still costs 43. So it was never the chain: with no optimizer
 * every `i++`, every compare and every `sink + 1` is a trip to memory -- about
 * six per iteration. 43 cycles is what a BMO C loop costs, and now it is
 * measured instead of assumed.
 *
 * ** TWO DEFECTS OF THAT FIRST VERSION, FIXED HERE. Both were mine:
 *
 *   1. The `if (which == ...)` chain was INSIDE the timed loop, so every case
 *      paid a different number of comparisons and the baseline was not the same
 *      for the four rows. It cost the door number nothing (43 is noise against
 *      2615) but it made the CALL number soft, and the call is the thing the
 *      door is compared against -- and indeed the call moved, 65 -> 63. Now
 *      each case has its own tight loop and there is nothing to subtract but
 *      the loop. [!] What did NOT move is the empty loop itself: see above,
 *      the 43 was never the chain.
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
                  unsigned long long guarda0, unsigned long long restaura0,
                  unsigned long long total_per_op) {
    unsigned long long doors;
    unsigned long long inside;
    unsigned long long guarda;
    unsigned long long restaura;
    unsigned long long contado;

    doors = bmo_info(BMO_INFO_SYSCALL_CUENTA) - doors0;

    if (doors == 0) {
        printf("   REPARTO: el kernel no conto ni una puerta\n");
        return;
    }
    inside = (bmo_info(BMO_INFO_SYSCALL_CICLOS) - cycles0) / doors;
    guarda = (bmo_info(BMO_INFO_SYSCALL_CICLOS_GUARDA) - guarda0) / doors;
    restaura = (bmo_info(BMO_INFO_SYSCALL_CICLOS_RESTAURA) - restaura0) / doors;
    contado = inside + guarda + restaura;

    printf("   reparto: dentro de dispatch %llu, en el stub %llu\n",
           inside,
           total_per_op - inside);

    /* ** Y el stub por dentro -- CUANDO SE ESTA MIDIENDO.
     *
     * Los cuatro sellos `rdtsc` que llenaban estas dos casillas se retiraron
     * del stub el 16-08, en cuanto dieron su numero: costaban 69 ciclos cada
     * uno --~276 sobre 1625, un 17%-- y un instrumento que ya contesto y sigue
     * cobrando es un peaje. Con los sellos fuera, los dos contadores no se
     * escriben y el delta sale CERO.
     *
     * Cero no se imprime como un reparto de ceros, porque eso seria una
     * medida falsa en vez de una medida ausente. Se dice que no se midio. */
    if (guarda == 0 && restaura == 0) {
        printf("   el stub por dentro: NO MEDIDO (sellos fuera del stub)\n");
        return;
    }
    if (contado > total_per_op) {
        /* El instrumento se contradice: las etapas no pueden sumar mas que el
         * total. Se dice en vez de imprimir una resta que daria la vuelta. */
        printf("   AVISO: las etapas suman %llu > total %llu -- NO LEER\n",
               contado, total_per_op);
        printf("   etapas: guarda %llu, dispatch %llu, restaura %llu\n",
               guarda, inside, restaura);
        return;
    }
    printf("   el stub por dentro: guardar %llu, devolver %llu, resto %llu\n",
           guarda, restaura, total_per_op - contado);
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
    unsigned long long guarda0;
    unsigned long long restaura0;
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
    guarda0 = bmo_info(BMO_INFO_SYSCALL_CICLOS_GUARDA);
    restaura0 = bmo_info(BMO_INFO_SYSCALL_CICLOS_RESTAURA);
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
    report_split(doors0, cycles0, guarda0, restaura0, best / BATCH);

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
        guarda0 = bmo_info(BMO_INFO_SYSCALL_CICLOS_GUARDA);
        restaura0 = bmo_info(BMO_INFO_SYSCALL_CICLOS_RESTAURA);
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
        report_split(doors0, cycles0, guarda0, restaura0, best / BATCH);
        bmo_codigo(handle, BMO_ARCH_CERRAR, 0, 0, 0);
    }

    /* -- 5. lo que cobra el propio metro ------------------------------ *
     *
     * ** ESTA FILA ES LA FACTURA DEL INSTRUMENTO, y va la ultima para no mover
     * de sitio las cuatro que ya tienen historia.
     *
     * El reparto de la fila 3 lo escriben cuatro `rdtsc` metidos DENTRO del
     * stub. Cada uno cuesta, y ese coste no se reparte a partes iguales: tres
     * de los cuatro caen enteros en la casilla `resto`, que es justo la que se
     * espera grande. O sea que el instrumento empuja hacia su propia
     * conclusion.
     *
     * Midiendo aqui lo que vale un `rdtsc` suelto, esos ~90 ciclos dejan de ser
     * una estimacion y pasan a ser una resta. Si el `resto` sale en miles no
     * cambia nada; si sale en cientos, esta fila es la que manda. */
    best = 0; total = 0;
    for (round = 0; round < ROUNDS; round++) {
        start = __rdtsc();
        for (i = 0; i < BATCH; i++) {
            sink = sink + __rdtsc();
        }
        elapsed = __rdtsc() - start;
        if (best == 0 || elapsed < best) { best = elapsed; }
        total = total + elapsed;
    }
    report("5. rdtsc suelto ", best, total);
    printf("   (menos la fila 1 = lo que cuesta UN sello del reparto)\n");

    /* Printed so nothing here can ever be considered dead, and so a wrong
     * value is visible instead of silent. */
    if (sink == 0) {
        printf("AVISO: el acumulador quedo en cero\n");
    }

    printf("COSTE: la fila 3 menos la 1 = la puerta desnuda\n");
    printf("COSTE: la 4 menos la 3 = lo que cuesta resolver un handle\n");
    return 0;
}
