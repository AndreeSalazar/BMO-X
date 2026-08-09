/* sonido_C.bex -- el programa que ESTRENA `KIND_AUDIO`.
 *
 * La capability de sonido se cableo de punta a punta hoy --kernel, userland y
 * cabecera de C-- y como toda capability recien escrita, lo unico que se sabe
 * de ella es lo que el codigo DICE que hace. Esto es lo que la llama.
 *
 * == Por que no basta con "pito y lo oi" ==
 *
 * Porque puede que no se oiga nada y que todo este bien. El puerto del altavoz
 * existe en todo x86; el zumbador fisico, no -- muchas placas modernas traen el
 * cabezal SPKR sin nada conectado. Un programa que solo comprueba "se oye"
 * confunde una placa sin altavoz con un kernel roto.
 *
 * Asi que lo que se comprueba es el CONTRATO, que si tiene respuesta:
 *
 *   1. hay handle        -- reclamar devuelve algo distinto de 0.
 *   2. contesta que hay  -- `aparatos` trae el bit del altavoz.
 *   3. el tope se cumple -- se piden 5000 ms y tienen que sonar 250. Sin tope,
 *                           un programa se lleva el nucleo el tiempo que quiera.
 *   4. es EXCLUSIVO      -- reclamar dos veces sin soltar tiene que fallar.
 *   5. soltar REVOCA     -- y esta es la de verdad: despues de soltar, el mismo
 *                           handle **no puede pitar**. Si pudiera, "soltar"
 *                           seria una palabra bonita y el aparato seguiria
 *                           siendo de quien dijo devolverlo.
 *
 * La 5 es la que justifica todo lo demas. Un handle que sigue funcionando
 * despues de revocarlo no da error: da un programa que hace ruido cuando ya no
 * le toca, y eso se descubre semanas despues y lejos.
 *
 * == Y ademas, la escala ==
 *
 * Al final suena una escala de cinco notas. Si hay zumbador se oye; si no, no
 * pasa nada -- las cinco pruebas de arriba ya contestaron sin depender del
 * oido de nadie.
 *
 * == Lo que se espera ver ==
 *
 *   handle = 0x1000...      (distinto de 0)
 *   aparatos = 1            (altavoz si, HDA no: el driver es la casilla 5.1)
 *   pedi 5000 ms, sonaron 250: el tope se cumple
 *   la 2a reclamacion fallo: es exclusivo
 *   el handle soltado ya NO pita: la revocacion funciona
 *   SONIDO: las cinco pruebas pasan
 *
 * == Como se lanza ==
 *
 *   c/sonido.bex     desde la caja Ejecutar del escritorio.
 *
 * Compilar:
 *   cargo run -p bmo-c-front -- toolchain/lang/c/examples/sonido_C.c \
 *       -o sonido.bex
 */
#include <bmo/sonido.h>

int main() {
    unsigned long long cap;
    unsigned long long otra;
    unsigned long long aparatos;
    unsigned long long sonaron;
    unsigned long long codigo;
    int i;
    int notas[5];

    printf("KIND_AUDIO - la primera vez que un programa RECLAMA EL SONIDO\n");

    /* -- 1. Hay handle ---------------------------------------------- */
    cap = bmo_sonido_reclamar();
    printf("handle = 0x%x\n", cap);
    if (cap == 0) {
        printf("FALLO: no se pudo reclamar (ya lo tiene otro proceso?)\n");
        return 1;
    }

    /* -- 2. Contesta que aparatos hay ------------------------------- */
    /* Preguntar y no suponer: el dia que exista HDA, este mismo binario se
     * entera sin recompilarse. */
    aparatos = bmo_sonido_aparatos(cap);
    printf("aparatos = %d\n", aparatos);
    if (aparatos == 0) {
        printf("FALLO: el kernel no declara ni un aparato\n");
        return 1;
    }
    if (aparatos & BMO_APARATO_ALTAVOZ) {
        printf("  altavoz del PC: hay camino (que suene depende de la placa)\n");
    }
    if (aparatos & BMO_APARATO_HDA) {
        printf("  HD Audio: abierto\n");
    } else {
        printf("  HD Audio: no (es la casilla 5.1, el driver no existe)\n");
    }

    /* -- 3. El tope se cumple --------------------------------------- */
    /* Se piden cinco segundos. Tienen que sonar 250 ms y ni uno mas: mientras
     * pita, este nucleo no hace otra cosa. Un programa capaz de pedir diez
     * segundos es un programa capaz de parar el planificador a voluntad. */
    sonaron = bmo_sonido_pitar(cap, 440, 5000);
    printf("pedi 5000 ms, sonaron %d\n", sonaron);
    if (sonaron != BMO_SONIDO_MAX_MS) {
        printf("FALLO: el tope no se cumple\n");
        return 1;
    }
    printf("el tope se cumple\n");

    /* -- 4. Es exclusivo -------------------------------------------- */
    /* Sin soltar, otra reclamacion tiene que fallar. Si diera un segundo
     * handle, dos partes del mismo programa --o dos programas-- creerian ser
     * duenas del aparato, y la segunda que soltara dejaria a la primera
     * pitando sobre algo que ya no es suyo. */
    otra = bmo_sonido_reclamar();
    if (otra != 0) {
        printf("FALLO: la 2a reclamacion dio 0x%x y tenia que fallar\n", otra);
        return 1;
    }
    printf("la 2a reclamacion fallo: es exclusivo\n");

    /* -- La escala, mientras el handle vale ------------------------- */
    /* Do Re Mi Sol La, en la cuarta octava. Si hay zumbador, se oye. */
    notas[0] = 262;
    notas[1] = 294;
    notas[2] = 330;
    notas[3] = 392;
    notas[4] = 440;
    bmo_sonido_volumen(cap, 80);
    for (i = 0; i < 5; i = i + 1) {
        bmo_sonido_pitar(cap, notas[i], 120);
    }
    bmo_sonido_callar(cap);
    printf("escala de cinco notas enviada al aparato\n");

    /* -- 5. Soltar REVOCA de verdad --------------------------------- */
    codigo = bmo_sonido_soltar();
    if (codigo != 0) {
        printf("FALLO: soltar devolvio codigo %d\n", codigo);
        return 1;
    }
    /* Y aqui la prueba que importa: el MISMO handle, ya revocado. Tiene que
     * dar codigo distinto de cero. Un handle que sobrevive a su revocacion es
     * un uso-despues-de-liberar con otro nombre. */
    codigo = bmo_codigo(cap, BMO_SONIDO_PITAR, 880, 100, 0);
    if (codigo == 0) {
        printf("FALLO: el handle soltado SIGUE pitando\n");
        return 1;
    }
    printf("el handle soltado ya NO pita: la revocacion funciona\n");

    /* Y despues de soltarlo, se puede volver a reclamar: soltar deja el
     * aparato libre, no lo pierde para siempre. */
    cap = bmo_sonido_reclamar();
    if (cap == 0) {
        printf("FALLO: tras soltarlo no se pudo volver a reclamar\n");
        return 1;
    }
    bmo_sonido_soltar();

    printf("SONIDO: las cinco pruebas pasan\n");
    return 0;
}
