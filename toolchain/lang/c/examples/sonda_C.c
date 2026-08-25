/* sonda_C.bex -- EL PROGRAMA QUE ATACA A SU PROPIO KERNEL, a proposito.
 *
 * == Que es esto y por que existe ==
 *
 * Todos los demas ejemplos usan la superficie BIEN: piden memoria y la
 * escriben, abren un archivo y lo leen. Esta sonda hace lo contrario -- le pasa
 * al kernel **lo que un programa honesto nunca le pasaria**: punteros que no son
 * suyos, longitudes gigantes, handles inventados, indices fuera de rango, la
 * quinta peticion de memoria cuando el tope son cuatro.
 *
 * La pregunta que contesta no es "funciona el sistema" sino **"que hace el
 * sistema cuando alguien lo empuja"**. Y la respuesta correcta a cada empujon es
 * SIEMPRE la misma: el kernel dice que no, con un codigo, y sigue vivo. Un
 * kernel que se cuelga, que reinicia, o que devuelve un valor donde deberia
 * negar, ha FALLADO la prueba -- aunque no se caiga en el acto.
 *
 * == Por que esto NO es un ataque de verdad ==
 *
 * No hay red, no hay privilegios que escalar, no hay otro proceso al que robar.
 * Un `.bex` de Ring 3 solo puede pasarle basura a la puerta y ver que contesta.
 * Eso es exactamente el modelo de amenaza REAL de BMO-X en uso: el disco te lo
 * rompe quien ya tiene la maquina; los syscalls te los rompe un programa que se
 * lanzo, y este es ese programa -- domesticado, contandolo.
 *
 * == Como se lee el resultado ==
 *
 * Cada prueba imprime `[ok]` si el kernel se defendio como debia, o `[FALLO]`
 * si dejo pasar algo. Al final, un recuento. Un `[FALLO]` no es que la sonda
 * este rota: es un agujero, y hay que mirarlo.
 *
 * ** Y la prueba de que el kernel SIGUE VIVO es que este programa llega hasta
 * el final e imprime su recuento. Si se cuelga en la prueba 3, la 3 es el
 * agujero -- y el sintoma es que las lineas de la 4 en adelante no salen.
 *
 * == [!] ESTO NO SE PUEDE CORRER EN EL EMULADOR ==
 *
 * El emulador del toolchain modela los syscalls que los ejemplos USAN BIEN, y
 * lo que no modela cae en su rama por defecto y **sale por el camino de exito
 * con valor cero**. O sea que a un handle inventado le contestaria igual que a
 * uno bueno.
 *
 * Para esta sonda eso no es una limitacion: es una MENTIRA. Diria "cero
 * agujeros" precisamente porque no esta comprobando nada. Esta sonda es un
 * instrumento de METAL -- se lanza en el Ryzen y se lee ahi, o no se lee.
 *
 * Compilar:
 *   cargo run -p bmo-c-front -- toolchain/lang/c/examples/sonda_C.c -o sonda.bex
 *
 * Lanzar:  c/sonda.bex   desde la caja Ejecutar del escritorio.
 */

#include <bmo/bmo.h>

/* * SE DECLARAN AQUI, y no llegan de ninguna cabecera.
 *
 * El codegen publica el handle y la base del bloque en estos dos nombres
 * **solo si el programa los declara**: mira su tabla de globales y, si no
 * estan, no emite el guardado. Es la misma decision que el resto del sistema --
 * lo que no pides no aparece-- y aqui hace falta un handle PROPIO contra el que
 * medir el rechazo de uno inventado. */
unsigned long long __bmo_bloque_cap;
unsigned long long __bmo_bloque_base;

int pasadas;
int fallos;

/* Un empujon que DEBE ser rechazado. `codigo != 0` es la defensa correcta.
 * Si el kernel contesta 0 --exito-- a algo que tenia que negar, es un agujero. */
void debe_negar(char *nombre, unsigned long long codigo) {
    if (codigo != 0) {
        printf("  [ok]    %s -> negado (codigo %d)\n", nombre, codigo);
        pasadas = pasadas + 1;
    } else {
        printf("  [FALLO] %s -> el kernel DEJO PASAR (codigo 0)\n", nombre);
        fallos = fallos + 1;
    }
}

/* Un empujon que puede contestar lo que sea MIENTRAS no reviente. Aqui no se
 * juzga el valor: se juzga que la maquina siga leyendo la linea siguiente. */
void debe_sobrevivir(char *nombre, unsigned long long valor) {
    printf("  [ok]    %s -> contesto 0x%x y seguimos vivos\n", nombre, valor);
    pasadas = pasadas + 1;
}

int main() {
    unsigned long long yo;
    unsigned long long base;
    unsigned long long mi_cap;
    int i;

    pasadas = 0;
    fallos = 0;

    printf("SONDA: empujando la superficie a ver que aguanta\n");
    printf("(que este programa llegue al final YA es media prueba)\n\n");

    /* Se pide un bloque legitimo primero: publica `__bmo_bloque_cap` y da un
     * handle de verdad contra el que medir los ataques de handle. */
    base = (unsigned long long)malloc(64);
    mi_cap = __bmo_bloque_cap;
    yo = bmo_pid();
    printf("pid=%d  bloque propio: cap=0x%x base=0x%x\n\n", yo, mi_cap, base);

    /* == 1. OPERACIONES QUE NO EXISTEN ==
     * Un numero de operacion que el kernel no conoce tiene que contestar
     * "no soportado", no ejecutar la de al lado ni caerse. */
    printf("1. operaciones inexistentes\n");
    debe_negar("op 0x7777 sobre TAREA_ACTUAL",
               bmo_codigo(BMO_TAREA_ACTUAL, 0x7777, 0, 0, 0));
    debe_negar("op 0xFFFFFFFF sobre TAREA_ACTUAL",
               bmo_codigo(BMO_TAREA_ACTUAL, 0xFFFFFFFF, 0, 0, 0));

    /* == 2. HANDLES INVENTADOS ==
     * Un handle que nadie concedio no se puede resolver. Es la clausula
     * central del sistema: "un handle que nadie te dio no se puede inventar".
     * Se prueba con varios patrones porque un handle se codifica como
     * (generacion, indice) y cada trozo malo tiene que fallar. */
    printf("\n2. handles que nadie concedio\n");
    debe_negar("handle 0x1",
               bmo_codigo(0x1, BMO_OP_INFO, 0, 0, 0));
    debe_negar("handle 0xDEADBEEF",
               bmo_codigo(0xDEADBEEF, BMO_OP_INFO, 0, 0, 0));
    debe_negar("handle 0xFFFFFFFFFFFFFF00",
               bmo_codigo(0xFFFFFFFFFFFFFF00, BMO_OP_INFO, 0, 0, 0));
    /* El handle propio CON el indice cambiado: misma generacion, otra ranura.
     * Tiene que fallar aunque el numero se parezca al bueno. */
    debe_negar("mi cap con el indice +1",
               bmo_codigo(mi_cap + 1, 0x02, 0, 0, 0));

    /* == 3. EL RENGLON DE RUTA, DESBORDADO ==
     * `OP_RUTA` acumula la ruta a trozos de 8 bytes en un buffer de 128. Se le
     * meten MUCHOS mas trozos de los que caben: el kernel tiene que recortar en
     * silencio (su tope), nunca escribir mas alla del buffer. Si esto
     * corrompiera algo, se notaria en las pruebas siguientes, no aqui. */
    printf("\n3. el renglon de ruta, inundado\n");
    for (i = 0; i < 400; i = i + 1) {
        bmo_codigo(BMO_TAREA_ACTUAL, BMO_OP_RUTA, 0x4141414141414141, 0, 0);
    }
    debe_sobrevivir("400 trozos de ruta (buffer de 128)", 0);
    /* Y ejecutar lo que quedo: una ruta de pura 'A' no existe, asi que
     * EJECUTAR tiene que fallar limpio -- no cargar basura. */
    debe_negar("EJECUTAR la ruta inundada",
               bmo_codigo(BMO_TAREA_ACTUAL, BMO_OP_EJECUTAR, 0, 0, 0));

    /* == 4. EL TOPE DE MEMORIA, FORZADO ==
     * El tope son cuatro peticiones por proceso. Ya se gasto una (el bloque
     * propio de arriba). Se piden muchas mas: las que pasen del tope tienen que
     * devolver 0, NUNCA memoria de nadie. Aqui se mira que el kernel diga que
     * no; que no reviente al insistir es la otra mitad. */
    printf("\n4. el tope de memoria, empujado\n");
    {
        unsigned long long p;
        int ceros;
        int dados;
        ceros = 0;
        dados = 0;
        for (i = 0; i < 10; i = i + 1) {
            p = (unsigned long long)malloc(64);
            if (p == 0) {
                ceros = ceros + 1;
            } else {
                dados = dados + 1;
            }
        }
        printf("  10 peticiones tras el tope: %d dieron, %d dijeron 0\n",
               dados, ceros);
        /* Tienen que haber salido ceros: si las diez dieran memoria, el tope
         * no existe y cada peticion es una fuga. */
        if (ceros > 0) {
            printf("  [ok]    el tope corta: hubo %d negativas\n", ceros);
            pasadas = pasadas + 1;
        } else {
            printf("  [FALLO] diez peticiones y NINGUNA negada: no hay tope\n");
            fallos = fallos + 1;
        }
    }

    /* == 5. PEDIR MEMORIA ABSURDA ==
     * Un tamano gigantesco --mas RAM de la que existe-- tiene que contestar 0,
     * no intentar mapear medio universo ni desbordar el calculo de paginas. */
    printf("\n5. peticiones de tamano imposible\n");
    debe_negar("malloc(0xFFFFFFFFFFFFFFFF)",
               (unsigned long long)malloc(0xFFFFFFFFFFFFFFFF) == 0 ? 1 : 0);
    debe_negar("malloc(0xFFFFFFFF00000000)",
               (unsigned long long)malloc(0xFFFFFFFF00000000) == 0 ? 1 : 0);

    /* == 6. TOMAR LO QUE NADIE OFRECIO ==
     * `OP_TOMAR` (0x1C) recoge un prestamo de memoria. Sin que nadie haya
     * ofrecido nada a este proceso, tiene que devolver 0 -- no un handle a
     * memoria ajena. */
    printf("\n6. tomar un prestamo que no existe\n");
    debe_negar("TOMAR sin oferta",
               bmo_valor(BMO_TAREA_ACTUAL, 0x1C, 0, 0, 0) == 0 ? 1 : 0);

    /* == 7. RECLAMAR DOS VECES ==
     * La pantalla es exclusiva. Reclamarla una vez puede salir bien; la SEGUNDA
     * vez, sin haberla soltado, tiene que fallar -- o dos duenos pintarian el
     * mismo framebuffer. (Si la primera falla porque el compositor la tiene, con
     * mas razon: nunca se concede dos veces.) */
    printf("\n7. reclamar la pantalla dos veces\n");
    {
        unsigned long long a;
        unsigned long long b;
        a = bmo_codigo(BMO_TAREA_ACTUAL, BMO_OP_PANTALLA_RECLAMAR, 0, 0, 0);
        b = bmo_codigo(BMO_TAREA_ACTUAL, BMO_OP_PANTALLA_RECLAMAR, 0, 0, 0);
        /* Al menos una de las dos tiene que ser un no. Que las dos digan que si
         * es el agujero: la pantalla tendria dos duenos. */
        if (a != 0 || b != 0) {
            printf("  [ok]    la pantalla no se concede dos veces\n");
            pasadas = pasadas + 1;
        } else {
            printf("  [FALLO] DOS reclamos de pantalla dijeron que si\n");
            fallos = fallos + 1;
        }
    }

    /* == 8. LANZAR OTRO PROGRAMA SIN AUTORIDAD ==
     * C4 de PLAN_SEGURIDAD, cerrada el 2026-08-25. Hasta ese dia CUALQUIER
     * .bex podia lanzar otro y reiniciar la maquina: no hacia falta un fallo,
     * bastaba con pedirlo.
     *
     * Esta sonda la lanzo el escritorio, y el escritorio NO PUEDE PASARLE su
     * autoridad -- no es una capability, es un atributo que fija Ring 0 al
     * nacer. Asi que aqui tiene que salir un no. */
    printf("\n8. lanzar otro programa sin tener autoridad\n");
    debe_negar("ejecutar desde un hijo",
               bmo_codigo(BMO_TAREA_ACTUAL, BMO_OP_EJECUTAR, 0, 0, 0));

    /* == 9. REINICIAR SIN AUTORIDAD -- Y VA LA ULTIMA A PROPOSITO ==
     * Es el mismo gate que la 8 y el empujon mas peligroso de toda la sonda:
     * **si el kernel NO se defiende, la maquina se reinicia aqui mismo** y
     * todo lo que esta prueba iba a decir se pierde con ella.
     *
     * Por eso va detras de las otras ocho y no antes. Es la regla de las hojas
     * de metal aplicada dentro de un programa: lo que no toca nada va primero,
     * y lo que no se deshace va al final.
     *
     * ** Y si se reinicia, ESO ES EL RESULTADO: una sonda que no llega a su
     * recuento ya dijo lo que habia que saber. */
    printf("\n9. reiniciar la maquina sin tener autoridad\n");
    printf("   (si esto reinicia, el agujero es este)\n");
    debe_negar("reiniciar desde un hijo",
               bmo_codigo(BMO_TAREA_ACTUAL, BMO_OP_REINICIAR, 0, 0, 0));

    /* == EL RECUENTO ==
     * Que esta linea salga es la prueba de fondo: el kernel aguanto los nueve
     * bloques de empujones sin caerse. */
    printf("\n== SONDA COMPLETA ==\n");
    printf("defensas correctas: %d\n", pasadas);
    printf("agujeros:           %d\n", fallos);
    if (fallos == 0) {
        printf("el kernel nego TODO lo que tenia que negar, y sigue en pie.\n");
    } else {
        printf("HAY %d AGUJERO(S): mira los [FALLO] de arriba.\n", fallos);
    }
    return 0;
}
