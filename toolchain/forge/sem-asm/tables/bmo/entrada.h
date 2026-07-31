/* entrada.h — el raton y el teclado, en C.
 *
 * ══ Lo que esta capability entrega, y lo que NO ══
 *
 * El kernel lee el HID —transferencias xHCI, endpoints, reintentos— porque eso
 * es tocar hardware. Lo que entrega son coordenadas ya recortadas al panel, una
 * mascara de botones, los bytes que van saliendo del teclado y las muescas de
 * rueda.
 *
 * **El cursor no sale de aqui, y las letras tampoco.** Su forma, su color y su
 * contorno son decisiones de aspecto, y ninguna de esas tiene nada que hacer en
 * Ring 0. Quien reclama la entrada dibuja su propio puntero.
 *
 * ══ Reclamarla es EXCLUSIVO ══
 *
 * Mientras un proceso la tenga, el shell de Ring 0 deja de leer el teclado
 * fisico y ningun otro proceso puede reclamarla. No es un reparto —dos
 * lectores de la misma cola se robarian las letras— es una cesion.
 *
 * En la practica: si el compositor esta corriendo, `bmo_entrada_reclamar()`
 * devuelve 0 y hay que comprobarlo. Un programa que da por hecho que la tiene
 * lee ceros para siempre y parece un raton roto.
 *
 * ══ Nada de esto BLOQUEA ══
 *
 * `bmo_entrada_tecla` devuelve -1 cuando no hay nada, y `bmo_entrada_rueda`
 * devuelve 0. Es a proposito: quien lee entrada tiene un bucle de fotograma, y
 * dormirse en el teclado congelaria el puntero entre tecla y tecla — justo al
 * reves de lo que uno quiere. El bucle correcto llama a `bmo_ceder()`.
 */
#ifndef BMO_ENTRADA_H
#define BMO_ENTRADA_H

#include <bmo/bmo.h>

/* ── Operaciones sobre el handle de entrada ──────────────────────────── */
#define BMO_ENTRADA_PUNTERO 0x01
#define BMO_ENTRADA_EVENTOS 0x02
#define BMO_ENTRADA_TECLA 0x03
#define BMO_ENTRADA_MODIFICADORES 0x04
#define BMO_ENTRADA_RUEDA 0x05

/* ── Modificadores ───────────────────────────────────────────────────── */
#define BMO_MOD_SHIFT 0x01
#define BMO_MOD_CTRL 0x02
#define BMO_MOD_ALT 0x04
#define BMO_MOD_ALTGR 0x08
#define BMO_MOD_CAPS 0x10

/* ── Las teclas sin glifo ────────────────────────────────────────────── */
/*
 * Van por la MISMA cola que las letras, con bytes del rango C1 de Latin-1
 * (0x80..0x9F). Ese rango no tiene glifo ni significado imprimible, asi que un
 * programa las distingue de lo que se escribe sin necesitar un segundo canal
 * — y nunca se dibujan por error.
 *
 * Son los mismos bytes que `ring0::dev::keyboard::KEY_*`. Si divergen, un
 * programa lee flechas donde hay paginas.
 */
#define BMO_TECLA_ARRIBA 0x80
#define BMO_TECLA_ABAJO 0x81
#define BMO_TECLA_IZQUIERDA 0x82
#define BMO_TECLA_DERECHA 0x83
#define BMO_TECLA_INICIO 0x84
#define BMO_TECLA_FIN 0x85
#define BMO_TECLA_SUPR 0x86
#define BMO_TECLA_REPAG 0x87
#define BMO_TECLA_AVPAG 0x88

/* Las teclas de FUNCION, detras de la navegacion en el mismo rango C1.
 *
 * ★ Son el sitio correcto para un atajo del sistema porque **no producen
 *   caracter en ninguna distribucion**: no pueden chocar con escribir. Una
 *   combinacion con Ctrl+Alt si puede — en espanol Ctrl+Alt ES AltGr.
 *
 * F12 abre la consola de datos del compositor. */
#define BMO_TECLA_F1 0x89
#define BMO_TECLA_F2 0x8A
#define BMO_TECLA_F3 0x8B
#define BMO_TECLA_F4 0x8C
#define BMO_TECLA_F5 0x8D
#define BMO_TECLA_F6 0x8E
#define BMO_TECLA_F7 0x8F
#define BMO_TECLA_F8 0x90
#define BMO_TECLA_F9 0x91
#define BMO_TECLA_F10 0x92
#define BMO_TECLA_F11 0x93
#define BMO_TECLA_F12 0x94

/* Reclama raton + teclado. Devuelve el handle, o **0 si no se pudo**.
 *
 * Comprobar el 0 no es opcional: es el caso normal cuando el compositor esta
 * vivo. Ver la nota de exclusividad arriba. */
unsigned long long bmo_entrada_reclamar() {
    return bmo_valor(BMO_TAREA_ACTUAL, BMO_OP_ENTRADA_RECLAMAR, 0, 0, 0);
}

/* Los tres datos del puntero vienen empaquetados en una sola llamada:
 * `(x << 32) | (y << 16) | botones`. Una llamada por fotograma, no tres. */
unsigned long long bmo_entrada_puntero(unsigned long long ent) {
    return bmo_valor(ent, BMO_ENTRADA_PUNTERO, 0, 0, 0);
}

int bmo_entrada_x(unsigned long long ent) {
    return (int)(bmo_entrada_puntero(ent) >> 32);
}

int bmo_entrada_y(unsigned long long ent) {
    return (int)((bmo_entrada_puntero(ent) >> 16) & 0xFFFF);
}

int bmo_entrada_botones(unsigned long long ent) {
    return (int)(bmo_entrada_puntero(ent) & 0xFF);
}

/* Cuantos informes HID se han visto desde el arranque.
 *
 * Distingue "el raton no se mueve" de "el raton no llega": si esto no sube al
 * moverlo, el problema esta en el USB y no en el programa. Es el primer numero
 * que hay que mirar cuando el puntero esta quieto. */
unsigned long long bmo_entrada_eventos(unsigned long long ent) {
    return bmo_valor(ent, BMO_ENTRADA_EVENTOS, 0, 0, 0);
}

/* La siguiente tecla, o **-1 si no hay ninguna**. No bloquea.
 *
 * El byte es Latin-1 ya resuelto: la `n~` llega como 0xF1, que es justo el
 * indice que entiende la fuente. Sin decodificador de por medio.
 *
 * -1 y no 0 porque 0 es un byte como cualquier otro; el kernel lo marca con el
 * bit 8 y aqui se traduce al convenio de C, el mismo de `getchar`. */
int bmo_entrada_tecla(unsigned long long ent) {
    unsigned long long v;
    v = bmo_valor(ent, BMO_ENTRADA_TECLA, 0, 0, 0);
    if (v & 0x100) {
        return (int)(v & 0xFF);
    }
    return -1;
}

/* Que modificadores estan pulsados AHORA. No consume nada: es estado.
 *
 * ★ En la distribucion espanola `Ctrl+Alt` **es** `AltGr`: lo que produce `@`,
 *   `#`, `[`, `]`, `\`, `|` y `€`. Un atajo que dispare al PULSARLOS rompe
 *   escribir todo eso. Si se usa como atajo, hay que disparar al SOLTAR y solo
 *   si no llego ningun caracter mientras estaban pulsados. */
int bmo_entrada_modificadores(unsigned long long ent) {
    return (int)bmo_valor(ent, BMO_ENTRADA_MODIFICADORES, 0, 0, 0);
}

/* Las muescas de rueda desde la ultima vez. Positivo = hacia arriba.
 *
 * ★ **Consume**: dos llamadas seguidas sin girar dan cero la segunda. Asi el
 *   llamante no tiene que guardar el valor anterior y restar — que es donde se
 *   cuela el scroll que se mueve solo.
 *
 * El `(int)` no es decorativo: el valor viaja como un entero de 32 bits en
 * complemento a dos dentro de un `unsigned long long`, y sin el cast girar
 * hacia atras daria cuatro mil millones en vez de -1. */
int bmo_entrada_rueda(unsigned long long ent) {
    return (int)bmo_valor(ent, BMO_ENTRADA_RUEDA, 0, 0, 0);
}

#endif /* BMO_ENTRADA_H */
