/* musica_C.bex -- la libreria de musica, tocando y ENSENANDOSE.
 *
 * Es el programa que estrena `<bmo/musica.h>`: notas por nombre, figuras y
 * tempo encima de `KIND_AUDIO`. Y es tambien la respuesta a por que hace falta
 * una capa ahi en medio -- comparense las dos formas de escribir lo mismo:
 *
 *     bmo_sonido_pitar(cap, 440, 425);      dato fisico
 *     bmo_nota(cap, LA4, NEGRA);            musica
 *
 * La segunda sigue siendo correcta si manana cambia el tempo, el aparato o el
 * driver. La primera hay que reescribirla entera.
 *
 * == La interfaz ==
 *
 * Cada nota se dibuja mientras suena: su nombre, su figura y una barra cuya
 * longitud es la altura. Se lee la melodia subiendo y bajando **en la pantalla,
 * al mismo tiempo que se oye** -- y eso importa mas de lo que parece, porque
 * puede que no se oiga nada: el puerto del altavoz existe en todo x86 y el
 * zumbador fisico no. Si la placa no lo trae, la pantalla es la unica prueba de
 * que la cadena entera funciono.
 *
 * == Lo que ademas ejercita, sin querer ==
 *
 * Los arrays globales de `int` indexados en bucle: `melodia[i]` con `int` son
 * cuatro bytes por elemento, y el escalado de indices es justamente lo que
 * fallaba el 08-08 (`*p` leia ocho bytes). Si las alturas salen en desorden o
 * repetidas, mirar ahi antes que al sonido.
 *
 * == Como se lanza ==
 *
 *   c/musica.bex     desde la caja Ejecutar del escritorio.
 *
 * Compilar:
 *   cargo run -p bmo-c-front -- toolchain/lang/c/examples/musica_C.c \
 *       -o musica.bex
 */
#include <bmo/musica.h>

/* La frase, en La menor. Sube, se queda arriba y vuelve -- la misma forma que
 * el maullido que da nombre al proyecto: una curva, no una escala. */
int frase[12];
int figura[12];
char barra[70];

/* Dibuja la nota: nombre, figura y una barra proporcional a la altura.
 * La escala esta elegida para que DO3 ocupe poco y LA5 casi toda la linea. */
void pintar(char *nombre, int hz, int fig) {
    int largo;
    int i;

    largo = (hz - 100) / 16;
    if (largo < 1) {
        largo = 1;
    }
    if (largo > 60) {
        largo = 60;
    }
    for (i = 0; i < largo; i = i + 1) {
        barra[i] = '=';
    }
    barra[largo] = '>';
    barra[largo + 1] = 0;

    printf("  %s  %d Hz  x%d  %s\n", nombre, hz, fig, barra);
}

int main() {
    unsigned long long cap;
    int i;
    int hz;

    printf("+--------------------------------------------------+\n");
    printf("|  BMO-X  --  musica sobre KIND_AUDIO              |\n");
    printf("+--------------------------------------------------+\n");

    cap = bmo_sonido_reclamar();
    if (cap == 0) {
        printf("no se pudo reclamar el sonido: lo tiene otro proceso\n");
        return 1;
    }
    printf("sonido reclamado. aparatos = %d\n", bmo_sonido_aparatos(cap));
    printf("(si la placa no trae zumbador, esto se VE y no se oye)\n\n");

    bmo_sonido_volumen(cap, 80);

    /* -- 1. La voz del sistema ------------------------------------- */
    /* Cuatro sonidos y la regla que los separa: lo que sube va bien, lo que
     * baja va mal. No hay que aprenderla. */
    printf("-- la voz del sistema --\n");

    printf("  arranque   (cuarta que sube, sin cerrar)\n");
    bmo_son_arranque(cap);
    bmo_silencio(cap, NEGRA);

    printf("  hecho      (sube y cierra)\n");
    bmo_son_ok(cap);
    bmo_silencio(cap, NEGRA);

    printf("  error      (segunda menor que BAJA)\n");
    bmo_son_error(cap);
    bmo_silencio(cap, NEGRA);

    printf("  aviso      (la misma nota dos veces: ni bien ni mal)\n");
    bmo_son_aviso(cap);
    bmo_silencio(cap, BLANCA);

    /* -- 2. La escala, para oir la afinacion ----------------------- */
    printf("\n-- escala de DO, a 150 --\n");
    bmo_musica_tempo(150);
    frase[0] = DO4;
    frase[1] = RE4;
    frase[2] = MI4;
    frase[3] = FA4;
    frase[4] = SOL4;
    frase[5] = LA4;
    frase[6] = SI4;
    frase[7] = DO5;
    for (i = 0; i < 8; i = i + 1) {
        hz = frase[i];
        pintar("nota", hz, CORCHEA);
        bmo_nota(cap, hz, CORCHEA);
    }
    bmo_silencio(cap, NEGRA);

    /* -- 3. La frase de BMO ---------------------------------------- */
    /* La menor. Doce notas con sus figuras: aqui es donde se ve que separar
     * altura y duracion valia la pena -- cambiar el tempo de abajo cambia la
     * pieza entera sin tocar una sola nota. */
    printf("\n-- la frase, a 108 --\n");
    bmo_musica_tempo(108);
    bmo_musica_ligado(80);

    frase[0] = LA4;   figura[0] = CORCHEA;
    frase[1] = DO5;   figura[1] = CORCHEA;
    frase[2] = MI5;   figura[2] = NEGRA;
    frase[3] = RE5;   figura[3] = CORCHEA;
    frase[4] = DO5;   figura[4] = CORCHEA;
    frase[5] = MI5;   figura[5] = NEGRA_P;
    frase[6] = SILENCIO; figura[6] = CORCHEA;
    frase[7] = LA5;   figura[7] = CORCHEA;
    frase[8] = SOL5;  figura[8] = CORCHEA;
    frase[9] = MI5;   figura[9] = NEGRA;
    frase[10] = RE5;  figura[10] = CORCHEA;
    frase[11] = LA4;  figura[11] = BLANCA;

    for (i = 0; i < 12; i = i + 1) {
        hz = frase[i];
        if (hz == SILENCIO) {
            printf("  (silencio)\n");
        } else {
            pintar("nota", hz, figura[i]);
        }
        bmo_nota(cap, hz, figura[i]);
    }

    /* -- 4. Un barrido, que no es musica pero es util -------------- */
    printf("\n-- barrido 200 -> 1500 Hz --\n");
    bmo_barrido(cap, 200, 1500, 30, 400);
    bmo_silencio(cap, CORCHEA);
    printf("-- y de vuelta --\n");
    bmo_barrido(cap, 1500, 200, 30, 400);

    /* -- 5. Devolver el aparato ------------------------------------ */
    bmo_sonido_callar(cap);
    if (bmo_sonido_soltar() != 0) {
        printf("\nFALLO: no se pudo soltar el sonido\n");
        return 1;
    }
    printf("\nsonido devuelto. la autopsia no deberia contar fugas.\n");
    printf("MUSICA: la libreria toca sobre KIND_AUDIO\n");
    return 0;
}
