/* ciclos -- DONDE se van los ticks de una puerta, y si se pueden REPARTIR.
 *
 * == Por que existe, habiendo ya DOS medidores ==
 *
 * `c/coste.bex` y `sys/precio.bex` contestan *"cuanto cuesta una puerta"*, y
 * llevan meses contestandolo bien. Este contesta otra pregunta, y es la que
 * hacia falta para la meta de 300 ticks de `presupuesto.rs`:
 *
 *     de lo que cuesta una puerta, CUANTO se paga por CRUZAR
 *     y cuanto por el TRABAJO que se pidio
 *
 * Y eso no es curiosidad: **decide si la meta se alcanza optimizando o
 * repartiendo**, que son dos proyectos distintos.
 *
 * ```text
 *    si el trabajo es la mayoria   -> hay que hacer el trabajo mas barato
 *    si el CRUCE es la mayoria     -> el trabajo esta bien y lo que sobra es
 *                                     pagar el cruce una vez por operacion
 * ```
 *
 * == *** COMO SE SEPARA UNA COSA DE LA OTRA SIN TOCAR EL KERNEL ==
 *
 * Con una puerta que **se rechaza**. `INVOKE` sobre `BMO_TAREA_ACTUAL` con una
 * operacion que no existe cae en `_ => unsupported()` --un `BmoStatus::err` y
 * se vuelve-- asi que recorre la maquina ENTERA y no hace ningun trabajo:
 *
 * ```text
 *    cruzar `syscall`      si
 *    guardar los 15 GPR    si
 *    entrar en `dispatch`  si
 *    el trabajo pedido     NO, no existe
 *    devolver y `sysretq`  si
 * ```
 *
 * ** O sea que una puerta rechazada ES el coste fijo, medido y no estimado. Y
 * la casa ya lo habia visto sin buscarlo: `medida/coste` lleva un comentario
 * que cuenta que una sonda paso el campo `0` a `INFO` por error, cayo en el
 * brazo por defecto, y **salio 784 contra los 870 de una operacion de verdad**.
 * Se leyo como *"la sonda estaba mal"*, se arreglo, y el 784 se tiro.
 *
 * > Era el numero mas valioso de los dos. Un rechazo no es una medida
 * > estropeada: es la unica forma de medir el cruce sin instrumentar el stub.
 *
 * [!] Y se toman DOS caminos de rechazo distintos --una operacion inexistente
 * y un campo inexistente de `INFO`-- a proposito. Si los dos dan lo mismo, el
 * fijo esta medido. Si difieren, la diferencia es lo que cuesta llegar hasta
 * cada rechazo, y entonces el mas barato es la cota buena. Un solo camino no
 * puede decir cual de las dos cosas esta pasando.
 *
 * == ** LA PROYECCION DEL LOTE, que es el motivo de todo esto ==
 *
 * Si una puerta cuesta `FIJO + TRABAJO` y una sola puerta pudiera llevar N
 * operaciones, el coste POR OPERACION seria:
 *
 *     FIJO/N + TRABAJO
 *
 * El fijo **se divide**; el trabajo no. Esa division es lo unico que puede
 * bajar de 300 sin quitarle nada al trabajo, y este programa imprime la tabla
 * con la marca de donde cruza la meta.
 *
 * [!] Es una PROYECCION y lo dice en pantalla. Un lote de verdad tiene que
 * escribir sus N respuestas en memoria del usuario, y eso cuesta. La tabla es
 * el TECHO de lo que se puede ganar, no lo que se va a ganar. Ver
 * `docs/plan/PLAN_LA_PUERTA_SE_PARTE.md`.
 *
 * == [!] Lo que este programa NO puede decir ==
 *
 * ```text
 *    [ ] no parte el FIJO en `syscall` / prologo / epilogo. Eso pide sellos
 *        DENTRO del stub, se hizo el 16-08, costo el 17% de cada puerta y se
 *        retiro. Aqui el fijo es UN numero
 *    [ ] no mide un lote, porque no existe: proyecta
 *    [ ] y `rdtsc` cuenta TICKS del TSC (3700 MHz), no ciclos del nucleo (que
 *        sube a 4600). Un tick son ~1,24 ciclos. TODO lo de aqui va en ticks,
 *        igual que los presupuestos
 * ```
 *
 * ** Y es una SONDA: cruza la puerta con literales que REX no publica --una
 * operacion y un campo que no existen, a proposito-- asi que el contrato va a
 * sacar su nota `[i]` de R14 por cada uno. Esa nota es correcta y la respuesta
 * es esta linea.
 *
 * == Como se lanza ==
 *
 *     run c/ciclos.bex        desde el shell de Ring 0, o desde la caja del
 *                             escritorio
 */

#include <bmo/bmo.h>
#include <stdio.h>

/* Llamadas dentro de UN bloque cronometrado. Grande para que los dos `rdtsc`
 * sean ruido, pequeno para que una expropiacion no toque a la mayoria. Los
 * mismos numeros que `coste_C.c`, para que las dos medidas se puedan comparar
 * -- cambiarlos aqui sin cambiarlos alli seria comparar dos cosas distintas. */
#define LOTE 4096
#define VUELTAS 16

/* ** LAS DOS PUERTAS QUE SE RECHAZAN, y por que estos dos numeros.
 *
 * `0x7E` no es ninguna de las operaciones de `BMO_TAREA_ACTUAL` --van de 0x01
 * a 0x25-- asi que cae en `_ => unsupported()` de `invoke_current_task`.
 *
 * `0` no es ningun campo de `OP_INFO` --empiezan en 0x01-- asi que cae en el
 * `_ => 0` de `report.rs`. Es el camino que dio 784 por accidente.
 *
 * [!] Los dos van a seguir siendo invalidos por construccion: el primero esta
 * POR ENCIMA de todo el espacio de operaciones y el segundo POR DEBAJO de
 * todos los campos. Si algun dia uno de los dos se USA, esta sonda deja de
 * medir un rechazo -- y lo dira, porque el numero se acercara a la fila de al
 * lado en vez de quedarse debajo.
 *
 * == ** Y VAN EN LINEA Y NO EN UN `#define`, Y ES R14 ==
 *
 * La primera version los puso en dos `#define` y **el contrato la rechazo**:
 *
 * ```text
 *    [X] R14: `ciclos_C.c` pasa `OP_QUE_NO_EXISTE` como operacion, y lo define
 *        el mismo fichero. Un numero del kernel copiado en una app es una copia
 *        que nadie compara con el original: usa el nombre de REX
 * ```
 *
 * Y el guardian esta en lo cierto sobre la forma aunque no sobre el caso: no
 * puede distinguir *"me he copiado una operacion"* de *"he elegido algo que NO
 * es una operacion"*. Lo que si distingue es el nombre: un `#define` **da a
 * entender que existe**, y estos existen justo por no existir.
 *
 * Asi que van donde se usan, con su porque al lado, y R14 saca su nota `[i]`
 * -- que es el camino que la regla tiene escrito para las sondas y el mismo por
 * el que pasa `sonda_C.c`. La nota es correcta y la respuesta es esta. */

/* La meta declarada en `ring0/syscall/presupuesto.rs`, fila `puerta`. Esta
 * escrita aqui para que la tabla de abajo pueda decir DONDE se cruza, y si
 * alli se cambia y aqui no, la tabla lo dira poniendo la marca en otro sitio.
 * No es un trinquete: es la linea que se esta persiguiendo. */
#define META 300

/* Con lo que se compara una puerta. Trivial a proposito: lo que se mide es la
 * LLAMADA, no el trabajo. */
unsigned long long llamada_normal(unsigned long long x) {
    return x + 1;
}

/* El minimo y la media de un escalon, en ticks por operacion.
 *
 * ** EL MINIMO ES LA RESPUESTA y la media se imprime igual. El planificador es
 * expropiativo y solo cambia de tarea en una frontera de trap, o sea que cada
 * puerta es una oportunidad de que se la lleven. El minimo es el unico valor
 * que eso no puede inflar; la distancia entre los dos ES la expropiacion, y es
 * un segundo dato util. */
static unsigned long long g_min;
static unsigned long long g_media;

static void anotar(unsigned long long mejor, unsigned long long total) {
    g_min = mejor / LOTE;
    g_media = total / (LOTE * VUELTAS);
}

static void fila(const char *nombre) {
    printf("  %s  min %5llu   media %6llu\n", nombre, g_min, g_media);
}

int main(void) {
    unsigned long long inicio;
    unsigned long long transcurrido;
    unsigned long long mejor;
    unsigned long long total;
    unsigned long long sumidero;
    unsigned long long termometro;
    unsigned long long fijo_op;
    unsigned long long fijo_campo;
    unsigned long long fijo;
    unsigned long long pid;
    unsigned long long info;
    unsigned long long trabajo;
    unsigned long long por_op;
    unsigned long long n;
    int vuelta;
    int i;

    sumidero = 0;

    printf("ciclos -- de que esta hecha una puerta de BMO-X\n");
    printf("lote %d, vueltas %d, todo en TICKS del TSC\n\n", LOTE, VUELTAS);

    /* -- 0. el bucle, para poder restarlo ----------------------------- */
    mejor = 0; total = 0;
    for (vuelta = 0; vuelta < VUELTAS; vuelta++) {
        inicio = __rdtsc();
        for (i = 0; i < LOTE; i++) {
            sumidero = sumidero + 1;
        }
        transcurrido = __rdtsc() - inicio;
        if (mejor == 0 || transcurrido < mejor) { mejor = transcurrido; }
        total = total + transcurrido;
    }
    anotar(mejor, total);
    fila("0. bucle vacio       ");

    /* -- 1. una llamada normal ---------------------------------------- */
    mejor = 0; total = 0;
    for (vuelta = 0; vuelta < VUELTAS; vuelta++) {
        inicio = __rdtsc();
        for (i = 0; i < LOTE; i++) {
            sumidero = llamada_normal(sumidero);
        }
        transcurrido = __rdtsc() - inicio;
        if (mejor == 0 || transcurrido < mejor) { mejor = transcurrido; }
        total = total + transcurrido;
    }
    anotar(mejor, total);
    fila("1. llamada normal    ");

    /* -- 2. EL TERMOMETRO: un `rdtsc` suelto --------------------------
     *
     * ** Se mide antes de usarlo para juzgar a nadie. El instrumento es del
     * tamano del enfermo: la casa ya midio que un `rdtsc` cuesta 69 ticks en un
     * bucle largo y 107 en uno corto, porque el CPU es fuera de orden y un
     * bucle largo lo solapa. Sin este escalon, "la puerta cuesta X" lleva
     * dentro un instrumento del que no se sabe el tamano. */
    mejor = 0; total = 0;
    for (vuelta = 0; vuelta < VUELTAS; vuelta++) {
        inicio = __rdtsc();
        for (i = 0; i < LOTE; i++) {
            sumidero = sumidero + __rdtsc();
        }
        transcurrido = __rdtsc() - inicio;
        if (mejor == 0 || transcurrido < mejor) { mejor = transcurrido; }
        total = total + transcurrido;
    }
    anotar(mejor, total);
    fila("2. rdtsc suelto      ");
    termometro = g_min;

    /* -- 3. *** PUERTA RECHAZADA (operacion inexistente) = EL FIJO ---- */
    mejor = 0; total = 0;
    for (vuelta = 0; vuelta < VUELTAS; vuelta++) {
        inicio = __rdtsc();
        for (i = 0; i < LOTE; i++) {
            sumidero = sumidero
                /* `0x7E` esta POR ENCIMA de todas las operaciones de
                 * `BMO_TAREA_ACTUAL` --van de 0x01 a 0x25-- asi que cae en el
                 * `_ => unsupported()` de `invoke_current_task`. */
                + bmo_valor(BMO_TAREA_ACTUAL, 0x7E, 0, 0, 0);
        }
        transcurrido = __rdtsc() - inicio;
        if (mejor == 0 || transcurrido < mejor) { mejor = transcurrido; }
        total = total + transcurrido;
    }
    anotar(mejor, total);
    fila("3. RECHAZO (op)      ");
    fijo_op = g_min;

    /* -- 4. PUERTA RECHAZADA (campo inexistente de INFO) -------------- */
    mejor = 0; total = 0;
    for (vuelta = 0; vuelta < VUELTAS; vuelta++) {
        inicio = __rdtsc();
        for (i = 0; i < LOTE; i++) {
            sumidero = sumidero
                /* El campo `0` esta POR DEBAJO de todos los de `OP_INFO`
                 * --empiezan en 0x01-- asi que cae en su `_ => 0`. Es el camino
                 * que dio 784 por accidente. */
                + bmo_valor(BMO_TAREA_ACTUAL, BMO_OP_INFO, 0, 0, 0);
        }
        transcurrido = __rdtsc() - inicio;
        if (mejor == 0 || transcurrido < mejor) { mejor = transcurrido; }
        total = total + transcurrido;
    }
    anotar(mejor, total);
    fila("4. RECHAZO (campo)   ");
    fijo_campo = g_min;

    /* -- 5. la puerta mas barata que SI hace algo --------------------- */
    mejor = 0; total = 0;
    for (vuelta = 0; vuelta < VUELTAS; vuelta++) {
        inicio = __rdtsc();
        for (i = 0; i < LOTE; i++) {
            sumidero = sumidero + bmo_valor(BMO_TAREA_ACTUAL, BMO_OP_PID, 0, 0, 0);
        }
        transcurrido = __rdtsc() - inicio;
        if (mejor == 0 || transcurrido < mejor) { mejor = transcurrido; }
        total = total + transcurrido;
    }
    anotar(mejor, total);
    fila("5. PID (la barata)   ");
    pid = g_min;

    /* -- 6. una mas gorda, todavia sin handle ------------------------- */
    mejor = 0; total = 0;
    for (vuelta = 0; vuelta < VUELTAS; vuelta++) {
        inicio = __rdtsc();
        for (i = 0; i < LOTE; i++) {
            sumidero = sumidero
                + bmo_valor(BMO_TAREA_ACTUAL, BMO_OP_INFO, BMO_INFO_TICKS, 0, 0);
        }
        transcurrido = __rdtsc() - inicio;
        if (mejor == 0 || transcurrido < mejor) { mejor = transcurrido; }
        total = total + transcurrido;
    }
    anotar(mejor, total);
    fila("6. INFO ticks        ");
    info = g_min;

    /* == EL REPARTO ================================================== */

    printf("\n");
    /* El fijo es el MENOR de los dos rechazos: el que menos camino recorrio
     * antes de que le dijeran que no. El otro trae ese camino de mas. */
    fijo = fijo_op;
    if (fijo_campo < fijo) { fijo = fijo_campo; }
    printf("EL REPARTO DE UNA PUERTA\n");
    printf("  fijo (cruzar y volver)      %5llu ticks\n", fijo);
    if (fijo_op > fijo_campo) {
        printf("    [!] los dos rechazos difieren en %llu: el de la operacion\n",
               fijo_op - fijo_campo);
        printf("        recorre mas antes del NO. El fijo es el menor.\n");
    } else if (fijo_campo > fijo_op) {
        printf("    [!] los dos rechazos difieren en %llu: el del campo\n",
               fijo_campo - fijo_op);
        printf("        recorre mas antes del NO. El fijo es el menor.\n");
    } else {
        printf("    los dos rechazos dan LO MISMO: el fijo esta medido.\n");
    }
    if (pid > fijo) {
        printf("  trabajo de PID              %5llu ticks\n", pid - fijo);
    } else {
        printf("  [!] PID sale <= que un rechazo (%llu vs %llu): algo no cuadra,\n",
               pid, fijo);
        printf("      y hasta saber que, el reparto de abajo no vale.\n");
    }
    if (info > fijo) {
        printf("  trabajo de INFO             %5llu ticks\n", info - fijo);
    }
    printf("  el termometro (rdtsc)       %5llu ticks, dentro de cada fila\n",
           termometro);

    /* == ** LA PROYECCION: repartir el fijo entre N operaciones ======= */

    trabajo = 0;
    if (pid > fijo) { trabajo = pid - fijo; }
    printf("\n");
    printf("SI UNA PUERTA LLEVARA N OPERACIONES (proyeccion, meta %d)\n", META);
    printf("  el fijo se DIVIDE; el trabajo NO.\n");
    printf("     N    ticks/op\n");
    n = 1;
    while (n <= 64) {
        por_op = fijo / n + trabajo;
        printf("   %3llu     %5llu", n, por_op);
        if (por_op <= META) {
            printf("   <== bajo la meta");
        }
        printf("\n");
        n = n * 2;
    }
    printf("\n");
    printf("  [!] Es un TECHO, no una promesa: un lote de verdad tiene que\n");
    printf("      escribir sus N respuestas, y eso cuesta. Lo que esta tabla\n");
    printf("      dice es CUANTO hay para ganar. Ver PLAN_LA_PUERTA_SE_PARTE.\n");
    if (trabajo > 0 && fijo / 64 + trabajo > META) {
        printf("\n");
        printf("  *** Y SI NI CON 64 SE LLEGA, la respuesta NO es el lote:\n");
        printf("      el trabajo por operacion (%llu) ya pasa de la meta solo,\n",
               trabajo);
        printf("      y entonces lo que hay que abaratar es el trabajo.\n");
    }
    return 0;
}
