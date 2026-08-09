/* sonido.h -- el SONIDO, en C.
 *
 * == Lo que esta capability entrega, y lo que NO ==
 *
 * Entrega el DERECHO a hacer ruido. No entrega un motor de audio: no hay
 * mezclador, no hay canales, no hay formato de muestras. El driver de HD Audio
 * --codec, DMA, anillo de buffers-- es la casilla 5.1 de `docs/PLAN_DOOM.md` y
 * todavia no existe.
 *
 * El contrato va ANTES que el driver a proposito. Escribir el motor primero y
 * preguntarse despues quien tiene derecho a usarlo es como se acaba con un
 * sistema donde cualquier programa pita encima de cualquier otro. La pantalla
 * ya aprendio esto: `KIND_FRAMEBUFFER` existia antes que el compositor.
 *
 * == Reclamarlo es EXCLUSIVO ==
 *
 * Un solo proceso lo tiene a la vez, igual que la pantalla y por el mismo
 * motivo: dos duenos escribiendo en el mismo aparato no es mezclar, es ruido.
 * Mezclar es un trabajo con nombre y le toca a Ring 3.
 *
 * Si otro lo tiene, `bmo_sonido_reclamar()` devuelve 0. Hay que comprobarlo:
 * un programa que da por hecho que lo tiene pita al vacio y parece un altavoz
 * roto.
 *
 * == Lo que suena hoy, dicho sin adornos ==
 *
 * El altavoz del PC, y puede que ni eso. El puerto que lo controla existe en
 * todo x86; el zumbador fisico, no -- muchas placas modernas traen el cabezal
 * SPKR sin nada conectado, y desde el kernel no hay forma de saberlo. Por eso
 * `bmo_sonido_aparatos()` dice que hay CAMINO, no que se vaya a oir algo.
 *
 * == Y PITAR BLOQUEA ==
 *
 * Mientras dura el tono, el nucleo no hace otra cosa: el altavoz del PC no
 * tiene interrupcion que avise de que acabo. El kernel recorta a
 * BMO_SONIDO_MAX_MS, asi que pedir mas no cuelga la maquina -- pero tampoco
 * suena mas. Es una propiedad del altavoz, no del contrato: con HDA se llena un
 * anillo y el DMA lo consume solo.
 */
#ifndef BMO_SONIDO_H
#define BMO_SONIDO_H

#include <bmo/bmo.h>

/* -- Operaciones sobre el handle de sonido ----------------------------- */
/* Que aparatos hay. Mapa de bits, ver BMO_APARATO_*. */
#define BMO_SONIDO_APARATO 0x01
/* Pitar: a0 = Hz, a1 = ms. Devuelve los ms que de verdad sonaron. */
#define BMO_SONIDO_PITAR 0x02
/* Volumen 0..100. En el altavoz del PC son DOS escalones, no cien: el volumen
 * se consigue cambiando el modo del temporizador y no hay mas modos. */
#define BMO_SONIDO_VOLUMEN 0x03
/* Callar ahora mismo. */
#define BMO_SONIDO_CALLAR 0x04

/* -- Lo que puede contestar BMO_SONIDO_APARATO ------------------------- */
#define BMO_APARATO_ALTAVOZ 1
#define BMO_APARATO_HDA 2

/* Tope de duracion de un pitido, en ms. El kernel recorta igual; esto solo
 * evita la sorpresa de pedir 5000 y recibir 250. */
#define BMO_SONIDO_MAX_MS 250

/* -- La capability ----------------------------------------------------- */

/* Reclamar el sonido. Devuelve el handle, o 0 si ya lo tiene otro proceso. */
unsigned long long bmo_sonido_reclamar() {
    return bmo_valor(BMO_TAREA_ACTUAL, BMO_OP_SONIDO_RECLAMAR, 0, 0, 0);
}

/* Soltarlo y seguir vivo. Devuelve 0 si se solto.
 *
 * Existe desde el primer dia por lo que costo que faltara en la pantalla: alli
 * la unica forma de dejar de ser dueno era morir, y el escritorio no podia
 * prestarla ni queriendo. */
unsigned long long bmo_sonido_soltar() {
    return bmo_codigo(BMO_TAREA_ACTUAL, BMO_OP_SONIDO_SOLTAR, 0, 0, 0);
}

/* Que aparatos hay. Un bit puesto dice que hay camino, no que se oiga. */
unsigned long long bmo_sonido_aparatos(unsigned long long cap) {
    return bmo_valor(cap, BMO_SONIDO_APARATO, 0, 0, 0);
}

/* Pitar. Devuelve los ms que de verdad sonaron (recortados a MAX_MS). */
unsigned long long bmo_sonido_pitar(unsigned long long cap,
                                    unsigned long long hz,
                                    unsigned long long ms) {
    return bmo_valor(cap, BMO_SONIDO_PITAR, hz, ms, 0);
}

/* Volumen 0..100. Devuelve el que quedo puesto. */
unsigned long long bmo_sonido_volumen(unsigned long long cap,
                                      unsigned long long v) {
    return bmo_valor(cap, BMO_SONIDO_VOLUMEN, v, 0, 0);
}

/* Callar. */
void bmo_sonido_callar(unsigned long long cap) {
    bmo_codigo(cap, BMO_SONIDO_CALLAR, 0, 0, 0);
}

#endif /* BMO_SONIDO_H */
