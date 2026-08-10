/* vivaldi_C.bex -- "La primavera", el ritornello de apertura.
 *
 * Antonio Vivaldi, *Le quattro stagioni*, concierto n.1 en mi mayor, Op. 8
 * (1725). **Dominio publico**: la obra tiene trescientos anos y la
 * transcripcion de estas dieciseis alturas es propia.
 *
 * == Por que una pieza de verdad y no cuatro pitidos ==
 *
 * `musica_C.c` ya prueba que la cadena funciona: reclamar el aparato, mandar
 * una frecuencia, respetar el tope. Lo que NO prueba es que la base sirva para
 * lo que la gente hace con un ordenador.
 *
 * Una melodia que alguien reconoce sin que se la anuncien es una prueba
 * distinta y mas dura: exige que el tempo sea estable, que dos notas iguales
 * seguidas se oigan como DOS, que los silencios midan, y que nada de eso se
 * descuadre al cabo de treinta segundos. Un pitido correcto no dice nada de
 * eso; ocho compases si.
 *
 * == ** EL ECO ESTA EN LA PARTITURA, no lo inventamos aqui ==
 *
 * Vivaldi escribio el ritornello **dos veces seguidas: forte y luego piano**.
 * Es una de las cosas por las que la pieza se reconoce, y resulta que BMO-X
 * tiene exactamente la operacion que eso pide -- `BMO_SONIDO_VOLUMEN`, cableada
 * el 2026-08-09 para el audifono USB.
 *
 * O sea que aqui no se esta adaptando la musica al sistema: **la musica pedia
 * algo y el sistema ya lo tenia**. Esa es toda la diferencia entre una demo y
 * una aplicacion.
 *
 * == Lo que este programa NO puede hacer, dicho antes de que suene ==
 *
 * 1. **Una sola voz.** El altavoz del PC es un cuadrado y una frecuencia; el
 *    concierto son cuerdas a cuatro voces y continuo. Esto es la LINEA del
 *    primer violin, no la pieza.
 * 2. **Bloquea.** `AUDIO_OP_BEEP` para el nucleo mientras suena --de ahi el
 *    tope de 250 ms por llamada, y por eso `bmo_sostener` parte las notas
 *    largas--. Mientras toca, la maquina esta tocando y nada mas.
 * 3. **Puede que no se oiga NADA y este todo bien.** El puerto 0x61 existe en
 *    todo x86; el zumbador fisico no. En la MSI A320M puede venir el cabezal
 *    SPKR sin conectar. Por eso la pieza **se dibuja mientras suena**: si la
 *    placa no trae zumbador, la pantalla es la unica prueba de que la cadena
 *    entera funciono.
 *
 * Las tres son del ALTAVOZ, no del diseno: `KIND_AUDIO` entrega un aparato y
 * el dia que haya un driver de HD Audio con su DMA, esta misma pieza sale por
 * ahi sin tocar una linea. Ver la fase 5 de `docs/PLAN_DOOM.md`.
 *
 * == Como se lanza ==
 *
 *   run c/vivaldi.bex
 */

#include <bmo/bmo.h>
#include <bmo/musica.h>

/* -- El ritornello, en dos filas paralelas -----------------------------
 *
 * Altura y figura separadas y no en pares dentro de un struct: asi la melodia
 * se lee como se lee una partitura --una linea de notas y debajo su ritmo-- y
 * corregir una duracion no obliga a tocar la altura de al lado.
 *
 * Mi mayor. El sostenido se escribe con su nombre bemol enarmonico porque es
 * como esta en `<bmo/musica.h>`: LAB5 es sol sostenido, y suenan igual.
 *
 *   compas 1-2 : mi mi mi   mi mi mi
 *   compas 3-4 : mi sol# si   si la sol#
 *   compas 5-6 : la la la   la la la
 *   compas 7-8 : la si la   sol# fa# mi
 */
#define NOTAS 24

int altura[NOTAS] = {
    MI5,  MI5,  MI5,     MI5,  MI5,  MI5,
    MI5,  LAB5, SI5,     SI5,  LA5,  LAB5,
    LA5,  LA5,  LA5,     LA5,  LA5,  LA5,
    LA5,  SI5,  LA5,     LAB5, SOLB5, MI5
};

int figura[NOTAS] = {
    NEGRA, CORCHEA, CORCHEA,   NEGRA, CORCHEA, CORCHEA,
    NEGRA, CORCHEA, CORCHEA,   NEGRA, CORCHEA, CORCHEA,
    NEGRA, CORCHEA, CORCHEA,   NEGRA, CORCHEA, CORCHEA,
    NEGRA, CORCHEA, CORCHEA,   NEGRA, CORCHEA, NEGRA
};

/* El nombre de cada nota, para pintarlo. Una tabla de punteros a cadena, que
 * es una relocation por elemento -- el mismo camino que `sprnames[]` de DOOM. */
char *nombre[NOTAS] = {
    "mi",  "mi",  "mi",   "mi",  "mi",  "mi",
    "mi",  "sol#", "si",  "si",  "la",  "sol#",
    "la",  "la",  "la",   "la",  "la",  "la",
    "la",  "si",  "la",   "sol#", "fa#", "mi"
};

/* Pinta una nota como una barra cuya longitud es su altura. Se lee la melodia
 * subiendo y bajando en la pantalla al mismo tiempo que se oye. */
void pintar(int i, int fuerte) {
    int barra;
    int k;

    printf("%s", fuerte ? "f " : "p ");
    printf("%-4s ", nombre[i]);
    /* La altura, escalada a algo que quepa en una linea. MI5 son 659 Hz y
     * LA5 880: dividir por 40 los separa sin salirse. */
    barra = altura[i] / 40;
    k = 0;
    while (k < barra) {
        printf("#");
        k = k + 1;
    }
    printf("\n");
}

/* Toca el ritornello entero a un volumen dado. */
void ritornello(unsigned long long cap, int volumen, int fuerte) {
    int i;

    bmo_sonido_volumen(cap, volumen);
    i = 0;
    while (i < NOTAS) {
        pintar(i, fuerte);
        bmo_nota(cap, altura[i], figura[i]);
        i = i + 1;
    }
}

int main() {
    unsigned long long cap;

    printf("VIVALDI - La primavera, ritornello (Op. 8 n.1, 1725)\n");

    cap = bmo_sonido_reclamar();
    if (cap == 0) {
        printf("no hay aparato de sonido: lo tiene otro proceso\n");
        return 1;
    }
    printf("aparatos = %d\n", (int)bmo_sonido_aparatos(cap));

    /* Allegro. 132 pulsos por minuto es el paso al que se suele tocar; a 120
     * --el de reposo-- se arrastra. */
    bmo_musica_tempo(132);
    /* Cuerda: las notas se separan poco. 90 en vez del 85 por defecto. */
    bmo_musica_ligado(90);

    /* ** Y AQUI ESTA LA PIEZA: forte, y el mismo ritornello en eco.
     *
     * No es una repeticion para rellenar. Esta en la partitura, y es lo que
     * hace que se reconozca: Vivaldi pinta la primavera contestandose a si
     * misma desde lejos. */
    ritornello(cap, 100, 1);
    bmo_silencio(cap, NEGRA);
    ritornello(cap, 35, 0);

    bmo_sonido_callar(cap);
    bmo_sonido_soltar();
    printf("VIVALDI: %d notas, dos veces\n", NOTAS * 2);
    return 0;
}
