/* musica.h -- notas, figuras y compas, encima de <bmo/sonido.h>.
 *
 * == Por que hay una capa aqui y no se llama al syscall directamente ==
 *
 * `bmo_sonido_pitar(cap, 440, 120)` dice "440 hercios durante 120
 * milisegundos". Eso no es una nota: es un dato fisico. Una nota son DOS
 * decisiones que el programa no deberia repetir cada vez -- **que altura** y
 * **cuanto dura respecto al pulso**. Aqui se dicen una sola vez:
 *
 *     bmo_musica_tempo(120);
 *     bmo_nota(cap, LA4, NEGRA);
 *
 * y la conversion a hercios y milisegundos la hace la cabecera. Cambiar el
 * tempo cambia la pieza entera; con numeros a pelo habria que reescribirla.
 *
 * == LA ARTICULACION, que es lo que separa musica de sirena ==
 *
 * Dos notas seguidas de la misma altura, tocadas sin hueco, **suenan como una
 * sola nota larga**. Por eso `bmo_nota` no ocupa toda su figura: suena el 85% y
 * calla el 15%. Ese silencio no es una pausa musical -- es lo que hace que se
 * oigan DOS. Se puede cambiar con `bmo_musica_ligado`.
 *
 * == Lo que esta capa NO puede tapar, y se dice ==
 *
 * El altavoz del PC es **monofonico y de onda cuadrada**: una nota a la vez y
 * siempre el mismo timbre. No hay acordes, no hay volumen continuo (son dos
 * escalones) y no hay instrumentos.
 *
 * Y `AUDIO_OP_PITAR` **bloquea**, con tope de 250 ms. Una blanca a 100 pulsos
 * por minuto son 1200 ms, o sea cinco veces el tope, asi que `bmo_nota` la
 * parte en trozos y los encadena. El corte entre trozos es de microsegundos
 * --el tiempo de volver del syscall y reprogramar el temporizador-- pero
 * **existe**: en una nota muy larga se puede notar un chasquido. Es una
 * propiedad del altavoz, no del contrato. El dia que haya un DAC de verdad,
 * estas mismas llamadas siguen valiendo y el troceo se borra de aqui.
 *
 * Esa es la razon de escribir esto ahora: **el lenguaje de la musica no cambia
 * cuando cambia el aparato.**
 *
 * -- EL SEMAFORO (L6g) y las dos preguntas de antes (L6e, L6f) --------
 *
 * Que cuesta que falle, por que falla ESTA pieza, y que arrastro si la
 * toco. La ley esta en `META-KERNEL_HARD.md`; el juez, en
 * `toolchain/tools/contrato/contrato.py`.
 *
 * [carril]  VERDE        una tabla de frecuencias y aritmetica de tempo. Ni
 *                        una puerta propia: todo baja por `<bmo/sonido.h>`
 * [cuesta]  NADA         se equivoca y suena mal. No hay nada detras que se
 *                        lleve por delante
 * [riesgo]  RELOJ        `PITAR` bloquea con tope de 250 ms, asi que `bmo_nota`
 *                        trocea las largas: lo que se OYE depende de cuando
 *                        vuelve cada syscall
 */
#ifndef BMO_MUSICA_H
#define BMO_MUSICA_H

#include <bmo/sonido.h>

/* -- Las alturas ------------------------------------------------------- */
/* Temperamento igual, redondeado al hercio. El altavoz del PC divide una
 * frecuencia base de 1.193.180 Hz por un entero, asi que la afinacion real ya
 * viene redondeada por el hardware: mas decimales aqui serian mentira. */
#define DO3 131
#define REB3 139
#define RE3 147
#define MIB3 156
#define MI3 165
#define FA3 175
#define SOLB3 185
#define SOL3 196
#define LAB3 208
#define LA3 220
#define SIB3 233
#define SI3 247

#define DO4 262
#define REB4 277
#define RE4 294
#define MIB4 311
#define MI4 330
#define FA4 349
#define SOLB4 370
#define SOL4 392
#define LAB4 415
#define LA4 440
#define SIB4 466
#define SI4 494

#define DO5 523
#define REB5 554
#define RE5 587
#define MIB5 622
#define MI5 659
#define FA5 698
#define SOLB5 740
#define SOL5 784
#define LAB5 831
#define LA5 880
#define SIB5 932
#define SI5 988

#define DO6 1047
#define RE6 1175
#define MI6 1319
#define FA6 1397
#define SOL6 1568
#define LA6 1760

/* Silencio: es una altura mas, y por eso vale 0 -- el kernel ya trata la
 * frecuencia 0 como "calla". Asi una frase puede llevar silencios sin que el
 * programa tenga que separar dos casos. */
#define SILENCIO 0

/* -- Las figuras, en dieciseisavos ------------------------------------- */
/* La unidad es el dieciseisavo y no la negra porque asi TODAS las figuras
 * corrientes son enteras: con la negra como unidad, una corchea seria 0.5 y
 * esto no tiene coma flotante. */
#define REDONDA 16
#define BLANCA 8
#define NEGRA_P 6 /* negra con puntillo: negra y media */
#define NEGRA 4
#define CORCHEA_P 3
#define CORCHEA 2
#define SEMICORCHEA 1

/* -- El pulso ---------------------------------------------------------- */

/* Pulsos por minuto. 120 es el de reposo, y es el que vale si nadie lo toca. */
int bmo_tempo_ppm = 120;
/* Cuanto de su figura SUENA una nota, en centesimas. 85 deja el hueco que hace
 * que dos notas iguales seguidas se oigan como dos. */
int bmo_tempo_ligado = 85;

void bmo_musica_tempo(int ppm) {
    /* Un tempo de 0 dividiria por cero mas abajo, y un tempo absurdo daria
     * notas de horas. Se recorta aqui y no se avisa: es un ajuste, no una
     * operacion que pueda fallar. */
    if (ppm < 20) {
        ppm = 20;
    }
    if (ppm > 400) {
        ppm = 400;
    }
    bmo_tempo_ppm = ppm;
}

void bmo_musica_ligado(int centesimas) {
    if (centesimas < 10) {
        centesimas = 10;
    }
    if (centesimas > 100) {
        centesimas = 100;
    }
    bmo_tempo_ligado = centesimas;
}

/* Milisegundos que dura una figura al tempo actual.
 *
 * Un dieciseisavo es la cuarta parte de una negra, y una negra son
 * 60000/ppm milisegundos. Se multiplica ANTES de dividir para no perder la
 * parte entera: 15000*figura/ppm y no (60000/ppm)*figura/4. */
int bmo_musica_ms(int figura) {
    return (15000 * figura) / bmo_tempo_ppm;
}

/* -- Tocar ------------------------------------------------------------- */

/* Mantiene una frecuencia el tiempo pedido, partiendola en trozos que quepan
 * en el tope del kernel. Ver la nota de la cabecera sobre el chasquido. */
void bmo_sostener(unsigned long long cap, int hz, int ms) {
    int resto;
    int trozo;
    resto = ms;
    while (resto > 0) {
        trozo = resto;
        if (trozo > BMO_SONIDO_MAX_MS) {
            trozo = BMO_SONIDO_MAX_MS;
        }
        bmo_sonido_pitar(cap, hz, trozo);
        resto = resto - trozo;
    }
}

/* Una nota: su altura y su figura. Suena `ligado` por ciento y calla el resto.
 *
 * El silencio del final se hace con frecuencia 0, o sea con la misma operacion:
 * callar no es un caso aparte, es tocar la nota vacia. */
void bmo_nota(unsigned long long cap, int hz, int figura) {
    int total;
    int suena;
    total = bmo_musica_ms(figura);
    suena = (total * bmo_tempo_ligado) / 100;
    if (hz == SILENCIO) {
        bmo_sostener(cap, 0, total);
        return;
    }
    bmo_sostener(cap, hz, suena);
    bmo_sostener(cap, 0, total - suena);
}

/* Un silencio. Existe por legibilidad: `bmo_silencio(cap, NEGRA)` se lee mejor
 * que `bmo_nota(cap, SILENCIO, NEGRA)` en medio de una frase. */
void bmo_silencio(unsigned long long cap, int figura) {
    bmo_nota(cap, SILENCIO, figura);
}

/* -- Efectos ----------------------------------------------------------- */

/* Barrido de `hz0` a `hz1` en `pasos` escalones. Sube o baja segun el orden de
 * los extremos: no hay dos funciones porque la resta ya lleva el signo. */
void bmo_barrido(unsigned long long cap, int hz0, int hz1, int pasos, int ms) {
    int i;
    int hz;
    int cada;
    if (pasos < 1) {
        pasos = 1;
    }
    cada = ms / pasos;
    if (cada < 1) {
        cada = 1;
    }
    for (i = 0; i < pasos; i = i + 1) {
        hz = hz0 + ((hz1 - hz0) * i) / pasos;
        bmo_sonido_pitar(cap, hz, cada);
    }
}

/* -- La voz del sistema ------------------------------------------------ */
/*
 * Cuatro sonidos, y son POCOS a proposito. Un sistema que pita distinto en
 * cada sitio no ensena nada: lo que hace util un aviso sonoro es que el mismo
 * suceso suene siempre igual, y que se distingan entre si sin mirar.
 *
 * La regla que los separa es la direccion: **lo que sube va bien, lo que baja
 * va mal**. No hay que aprenderla, ya se sabe.
 */

/* Arranque: cuarta justa ascendente. Abierta, sin resolver -- "empieza algo". */
void bmo_son_arranque(unsigned long long cap) {
    bmo_musica_tempo(160);
    bmo_nota(cap, DO5, CORCHEA);
    bmo_nota(cap, SOL5, CORCHEA);
    bmo_nota(cap, DO6, NEGRA);
}

/* Hecho: dos notas que suben y cierran. Corto, para que no moleste al repetir. */
void bmo_son_ok(unsigned long long cap) {
    bmo_musica_tempo(200);
    bmo_nota(cap, SOL5, SEMICORCHEA);
    bmo_nota(cap, DO6, CORCHEA);
}

/* Error: segunda menor descendente, la unica disonancia de las cuatro. Suena
 * mal a proposito -- es lo que se pretende. */
void bmo_son_error(unsigned long long cap) {
    bmo_musica_tempo(140);
    bmo_nota(cap, SI3, CORCHEA);
    bmo_nota(cap, SIB3, NEGRA);
}

/* Aviso: la misma nota dos veces. Ni sube ni baja: no juzga, solo llama. */
void bmo_son_aviso(unsigned long long cap) {
    bmo_musica_tempo(180);
    bmo_nota(cap, LA5, SEMICORCHEA);
    bmo_silencio(cap, SEMICORCHEA);
    bmo_nota(cap, LA5, SEMICORCHEA);
}

#endif /* BMO_MUSICA_H */
