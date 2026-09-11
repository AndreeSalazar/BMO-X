/* superficie/amarilla.h -- decodificar lo que el DIRECTOR escribio: eventos y puntero
 *
 * Un CARRIL de `<bmo/superficie.h>` (L6g). La cabecera entera --que
 * explica por que existe esta pieza-- esta en la fachada; aqui va lo
 * que cambia de color.
 *
 * [carril]  AMARILLO     no reserva ni ofrece nada: descifra un formato. Se
 *                        puede tocar sin arrastrar memoria, pero el otro lado
 *                        tiene que decir lo mismo
 * [cuesta]  NADA         se equivoca y una app cree que se pulso una tecla que
 *                        nadie pulso
 * [riesgo]  ESPEJO SILENCIO
 *                        ESPEJO: el bit 63 lo enciende el DIRECTOR y lo lee
 *                        esto. SILENCIO: leer `e & 0xFF` sin preguntar por ese
 *                        bit hace creer que se pulso la tecla numero 1 en cada
 *                        clic -- el fichero ya lo avisa; y el byte 2 del
 *                        estado (VISTA) lo escribe el DIRECTOR y lo lee esto
 */
#ifndef BMO_SUPERFICIE_AMARILLA_H
#define BMO_SUPERFICIE_AMARILLA_H

#include <bmo/superficie/roja.h>

/* -- ** UNA RANURA PUEDE SER UNA TECLA O UN RATON, Y HAY QUE MIRARLO -----
 *
 * El bit 63 lo dice. Una TECLA es, bit a bit, lo mismo que devuelve
 * `bmo_entrada_evento` --el kernel nunca enciende el 63-- asi que el codigo que
 * ya sabia leer teclas sigue valiendo sin tocar una coma.
 *
 * [!] Y ESO ES JUSTO LO QUE LO HACE PELIGROSO SI NO SE MIRA. En un evento de
 * raton el byte bajo son los BOTONES, no un scancode: una app que lea
 * `e & 0xFF` sin preguntar por el bit 63 va a creer que se pulso la tecla
 * numero 1 cada vez que alguien haga clic. Por eso el bit tiene su propia
 * pregunta --`bmo_sup_es_raton`-- y no se deja al llamante recordarlo.
 *
 *    bit 63       1 = raton, 0 = tecla
 *    bit 8        HAY, en los dos
 *    bit 9        PULSADA: la tecla baja, o el boton baja
 *    bits 0..7    el scancode, o la mascara de BOTONES (1 izq, 2 der)
 *    bits 16..31  x dentro de la app, en pixeles suyos
 *    bits 32..47  y
 *
 * ** Las coordenadas son SUYAS, no de la pantalla: el DIRECTOR ya resto el
 * origen de la ventana antes de dejarlas ahi. Una app no sabe --ni tiene por
 * que saber-- donde la pusieron.
 *
 * ** VIAJAN LAS DOS CARAS: el boton bajando y subiendo, con `PULSADA` puesta o
 * no. Lo que NO hay es CAPTURA -- si sueltas fuera de la ventana, ese soltar no
 * llega, porque se entrega a quien esta debajo del puntero y ya no eres tu.
 * Para arrastrar algo hasta el borde hace falta captura, y no esta. Dicho aqui
 * y no escondido: media promesa contada entera es una limitacion; contada a
 * medias es un fallo. */
#define BMO_SUP_EV_RATON 0x8000000000000000ULL

int bmo_sup_es_raton(unsigned long long e) {
    if ((e & BMO_SUP_EV_RATON) != 0) {
        return 1;
    }
    return 0;
}

int bmo_sup_raton_x(unsigned long long e) {
    return (int)((e >> 16) & 0xFFFF);
}

int bmo_sup_raton_y(unsigned long long e) {
    return (int)((e >> 32) & 0xFFFF);
}

int bmo_sup_raton_botones(unsigned long long e) {
    return (int)(e & 0xFF);
}

/* Sacar un evento del buzon. **0 si no hay ninguno**, y no bloquea.
 *
 * Devuelve el evento CRUDO, el mismo `unsigned long long` de
 * `bmo_entrada_evento`: el bit 8 dice si hay, el 9 si es pulsada, y el byte
 * bajo es el scancode.
 *
 *     unsigned long long e = bmo_superficie_evento(s);
 *     if (e & BMO_EVENTO_HAY) {
 *         int sc = (int)(e & 0xFF);
 *     }
 *
 * ** POR QUE ESTO NO PUEDE LEER FUERA DEL BLOQUE, aunque la CABEZA venga de
 * otro proceso: el indice con el que se lee es la COLA, que es NUESTRA y se
 * enmascara aqui. La cabeza solo se usa para comparar --"hay algo?"--, asi que
 * una cabeza con basura dentro puede hacer que se lea una ranura vieja, nunca
 * que se lea fuera. Es la misma disciplina que el DIRECTOR aplica a nuestra
 * cabecera, en el otro sentido.
 */
unsigned long long bmo_superficie_evento(BMO_SUPERFICIE *s) {
    unsigned long long buz;
    unsigned int ranuras;
    unsigned int cabeza;
    unsigned int cola;
    unsigned long long *ranura;
    unsigned long long e;
    int idx;

    if (s == 0) {
        return 0;
    }
    buz = (unsigned long long)bmo_sup_leer(s->base, 6);
    ranuras = bmo_sup_leer(s->base, 7);
    if (buz == 0 || ranuras == 0) {
        return 0;
    }
    idx = (int)(buz / 4);
    cabeza = bmo_sup_leer(s->base, idx);
    cola = bmo_sup_leer(s->base, idx + 1);
    if (cola == cabeza) {
        return 0; /* vacio */
    }
    ranura = (unsigned long long *)(s->base + buz + BMO_SUP_BUZON_CABECERA
              + (unsigned long long)(cola & (ranuras - 1)) * BMO_SUP_BUZON_RANURA);
    e = *ranura;
    cola = (cola + 1) & (ranuras - 1);
    bmo_sup_poner(s->base, idx + 1, cola);
    return e;
}

/* -- ** DONDE ESTA EL PUNTERO AHORA, que es un ESTADO y no un evento ----
 *
 * Se lee cuando hace falta y no se consume: dos lecturas seguidas sin que el
 * raton se haya movido contestan lo mismo. Ver el porque en la cabecera.
 *
 * `bmo_superficie_dentro` es el que hay que preguntar primero: cuando el raton
 * no esta encima, x e y conservan **la ultima posicion buena**, que es lo unico
 * util que se puede dejar ahi -- ponerlas a cero diria que el puntero esta en
 * la esquina, y eso es una posicion, no una ausencia. */
int bmo_superficie_dentro(BMO_SUPERFICIE *s) {
    unsigned long long buz;
    if (s == 0) {
        return 0;
    }
    buz = (unsigned long long)bmo_sup_leer(s->base, 6);
    if (buz == 0) {
        return 0;
    }
    return (int)((bmo_sup_leer(s->base, (int)(buz / 4) + 3) >> 8) & 0xFF);
}

int bmo_superficie_puntero_x(BMO_SUPERFICIE *s) {
    unsigned long long buz;
    if (s == 0) {
        return 0;
    }
    buz = (unsigned long long)bmo_sup_leer(s->base, 6);
    if (buz == 0) {
        return 0;
    }
    return (int)(bmo_sup_leer(s->base, (int)(buz / 4) + 2) & 0xFFFF);
}

int bmo_superficie_puntero_y(BMO_SUPERFICIE *s) {
    unsigned long long buz;
    if (s == 0) {
        return 0;
    }
    buz = (unsigned long long)bmo_sup_leer(s->base, 6);
    if (buz == 0) {
        return 0;
    }
    return (int)((bmo_sup_leer(s->base, (int)(buz / 4) + 2) >> 16) & 0xFFFF);
}

/* -- ** LO QUE NO SE VE, NO SE PINTA (R-APP8 de `META-APP_HARD.md`) ------
 *
 * El DIRECTOR deja en el byte 2 del estado del buzon si esta ventana se ve, y
 * por que no. Lo decide EL y no la app: una app no puede declararse vista. Lo
 * juzga `bmo_golpe::vista` en el anfitrion, con sus pruebas, y estos cuatro
 * numeros son un contrato con ese juez -- el banco los compara.
 *
 *    0  SE_VE        pinta. Es tambien lo que lee una app sin buzon, y lo que
 *                    escribia el DIRECTOR antes de que esto existiera
 *    1  MINIMIZADA   esta en la barra
 *    2  FUERA        arrastrada fuera del lienzo, o sin interior
 *    3  PRESTADA     el DIRECTOR presto la pantalla entera a otro programa
 *
 * ** Que hacer con el: si no se ve, **no dibujar y no llamar a
 * `bmo_superficie_lista`**. Todo lo demas sigue -- la logica, el sonido, la
 * entrada --, asi que al volver a verse el primer fotograma ya es el de ahora.
 * Una app que siga pintando oculta no se castiga: se ACUSA, por su tid.
 *
 *     if (bmo_superficie_se_ve(s)) {
 *         dibujar_en(bmo_superficie_pixeles(s), 640, 400);
 *         bmo_superficie_lista(s);
 *     }
 *
 * [!] Lo que el DIRECTOR todavia NO sabe decir es "tapada por otra ventana":
 * ante la duda contesta que se ve. Ahorra menos, y nunca congela lo que se
 * mira. */
#define BMO_SUP_VISTA_SE_VE 0
#define BMO_SUP_VISTA_MINIMIZADA 1
#define BMO_SUP_VISTA_FUERA 2
#define BMO_SUP_VISTA_PRESTADA 3

/* El motivo, tal cual. 0 si se ve o si no hay buzon donde leerlo. */
int bmo_superficie_vista(BMO_SUPERFICIE *s) {
    unsigned long long buz;
    if (s == 0) {
        return BMO_SUP_VISTA_SE_VE;
    }
    buz = (unsigned long long)bmo_sup_leer(s->base, 6);
    if (buz == 0) {
        return BMO_SUP_VISTA_SE_VE;
    }
    return (int)((bmo_sup_leer(s->base, (int)(buz / 4) + 3) >> 16) & 0xFF);
}

/* 1 si hay que pintar, 0 si nadie lo va a ver. */
int bmo_superficie_se_ve(BMO_SUPERFICIE *s) {
    if (bmo_superficie_vista(s) == BMO_SUP_VISTA_SE_VE) {
        return 1;
    }
    return 0;
}

#endif /* BMO_SUPERFICIE_AMARILLA_H */
