/* superficie.h -- una app dibuja en SU memoria, y el DIRECTOR la compone.
 *
 * == El cambio de modelo, en una frase ==
 *
 * Hoy la pantalla es EXCLUSIVA: `prestar_pantalla` se la quita al compositor y
 * se la da al hijo entera. Por eso un programa no puede vivir en una caja --
 * no hay caja, hay relevo, y mientras el hijo corre el escritorio no existe.
 *
 * Una SUPERFICIE le da la vuelta: el programa pide memoria, **dibuja ahi**, y
 * se la OFRECE al DIRECTOR. El DIRECTOR la pega dentro de un marco con sus tres
 * botones. La pantalla no cambia de dueno ni una vez.
 *
 * == ** PANTALLA COMPLETA DEJA DE SER "TE DOY EL HARDWARE" ==
 *
 * Y esa es la parte que importa. En Windows, pantalla completa exclusiva
 * entrega el dispositivo: si el juego se cuelga, te llevas la maquina.
 *
 * Aqui, pantalla completa es **que el DIRECTOR no te dibuje el borde**. Sigue
 * componiendo, asi que Alt+Tab sigue, el conmutador sigue, y `Ctrl+Alt+ESC`
 * --que vive en Ring 0 y no depende de que nadie este vivo-- sigue. Un juego
 * colgado a pantalla completa se cierra con el teclado en vez de con el boton
 * de reset.
 *
 * == Por que la descripcion viaja DENTRO de la memoria ==
 *
 * El kernel presta BYTES: `loan::offer` mueve un rango y no sabe --ni tiene por
 * que saber-- que hay pixeles dentro. Meterle ancho y alto seria ensenarle al
 * kernel lo que es una imagen para que se lo repita a otro.
 *
 * Asi que la superficie **se describe a si misma**: los primeros 32 bytes del
 * bloque son la cabecera y los pixeles van detras. Es la misma decision que
 * `BRES` dentro de la seccion de recursos y que `BICO` dentro del paquete --
 * en este sistema, **el dato dice lo que es**, y quien lo transporta no
 * necesita entenderlo.
 *
 * Consecuencia practica: esto NO pide ni una operacion nueva del kernel.
 * `MEM_OP_OFRECER` ya existe y ya basta.
 *
 * == La disposicion ==
 *
 * ```text
 *    0..4    "BSUP"
 *    4..8    ancho  en pixeles (u32)
 *    8..12   alto   en pixeles (u32)
 *   12..16   stride en PIXELES, no en bytes (u32)
 *   16..20   formato: 0 = BGRA de 32 bits, que es el del framebuffer (u32)
 *   20..24   SECUENCIA: sube cada vez que el dibujo esta entero (u32)
 *   24..32   reservado, a cero
 *   32..     los pixeles
 * ```
 *
 * ** LA SECUENCIA ES EL UNICO CAMPO QUE NO ES OBVIO, y es el que hace que esto
 * funcione sin cerrojos.
 *
 * El DIRECTOR lee la superficie mientras la app la escribe: son dos procesos
 * sobre la misma memoria y no hay quien los pare. Sin nada mas, el compositor
 * pegaria medio fotograma viejo y medio nuevo -- el desgarro de toda la vida.
 *
 * La regla es de una linea: **la app sube `secuencia` cuando el dibujo esta
 * entero, y el DIRECTOR solo repinta cuando ve un numero distinto del que
 * pego la ultima vez.** Un fotograma a medias no cambia el numero, asi que no
 * se pinta; y el peor caso es ensenar el anterior un fotograma mas, que es
 * exactamente lo que uno quiere que pase.
 *
 * No es un cerrojo y no debe serlo: un cerrojo entre dos procesos deja al
 * compositor esperando a una app que se colgo -- y entonces una app rota se
 * lleva el escritorio, que es justo lo que este diseno existe para impedir.
 *
 * == Como se usa ==
 *
 *     BMO_SUPERFICIE *s = bmo_superficie_crear(640, 400);
 *     for (;;) {
 *         dibujar_en(bmo_superficie_pixeles(s), 640, 400);
 *         bmo_superficie_lista(s);     // el dibujo esta entero
 *         bmo_ceder();
 *     }
 */
#ifndef BMO_SUPERFICIE_H
#define BMO_SUPERFICIE_H

#include <bmo/bmo.h>
/* Los dos numeros con los que se nombra el bloque del monton. Se traen aqui
 * porque `MEM_OFRECER` presta `base + desde` y sin ellos no hay ni base ni
 * desde -- y no traerlos es lo que rompio el puerto de `ray.bex` a ventana. */
#include <bmo/bloque.h>

/* Ofrecer un trozo del bloque propio. Operacion sobre `KIND_MEMORIA`. */
#define BMO_MEM_OFRECER 0x03
/* Quien me lanzo. Hace falta para saber A QUIEN ofrecer: una superficie se le
 * da al DIRECTOR, y el programa no tiene otra forma de nombrarlo. */
#define BMO_OP_MI_PADRE 0x26

#define BMO_SUP_MAGIC 0x50555342 /* "BSUP" en little-endian */
#define BMO_SUP_CABECERA 32
/* BGRA de 32 bits: el mismo que el framebuffer, asi que el DIRECTOR compone
 * copiando y no convirtiendo. Un formato distinto seria una conversion por
 * pixel y por fotograma en el proceso que menos puede permitirsela. */
#define BMO_SUP_BGRA32 0

struct BMO_SUPERFICIE {
    unsigned long long base;   /* donde empieza el bloque, en MI espacio */
    unsigned long long bloque; /* el handle, para poder ofrecerlo */
    int ancho;
    int alto;
};
typedef struct BMO_SUPERFICIE BMO_SUPERFICIE;

/* Los cuatro enteros de la cabecera viven en el bloque, no aqui: el DIRECTOR
 * lee ESOS. Escribirlos en el struct local y no en la memoria compartida seria
 * describir la superficie donde el unico que puede verlo es quien ya lo sabe. */
void bmo_sup_poner(unsigned long long base, int i, unsigned int v) {
    unsigned int *c;
    c = (unsigned int *)(base + i * 4);
    *c = v;
}

unsigned int bmo_sup_leer(unsigned long long base, int i) {
    unsigned int *c;
    c = (unsigned int *)(base + i * 4);
    return *c;
}

/* Los pixeles: detras de la cabecera. */
unsigned int *bmo_superficie_pixeles(BMO_SUPERFICIE *s) {
    if (s == 0) {
        return 0;
    }
    return (unsigned int *)(s->base + BMO_SUP_CABECERA);
}

/* ** EL DIBUJO ESTA ENTERO. Llamar a esto es lo unico que hace que el DIRECTOR
 * pinte -- ver la nota de la secuencia en la cabecera.
 *
 * Se sube DESPUES del ultimo pixel y nunca antes: subirla al empezar el
 * fotograma seria prometer un dibujo que todavia se esta haciendo. */
void bmo_superficie_lista(BMO_SUPERFICIE *s) {
    if (s == 0) {
        return;
    }
    bmo_sup_poner(s->base, 5, bmo_sup_leer(s->base, 5) + 1);
}

/* Pide la memoria, escribe la cabecera y **se la ofrece a quien nos lanzo**.
 *
 * Devuelve 0 si no hay memoria o si no hay a quien ofrecersela -- lo segundo
 * pasa cuando el programa se lanza desde el shell de Ring 0, que no compone
 * nada. Un programa que quiera funcionar en los dos sitios comprueba el 0 y se
 * cae al camino de la pantalla exclusiva. */
BMO_SUPERFICIE *bmo_superficie_crear(int ancho, int alto) {
    BMO_SUPERFICIE *s;
    unsigned long long bytes;
    unsigned long long padre;

    if (ancho <= 0 || alto <= 0) {
        return 0;
    }
    bytes = BMO_SUP_CABECERA + (unsigned long long)ancho * alto * 4;

    s = (BMO_SUPERFICIE *)malloc(48);
    if (s == 0) {
        return 0;
    }
    /* [!] El bloque tiene que salir del monton, y el monton pide UN bloque al
     * kernel: la superficie sale de ahi dentro. Por eso `<stdlib.h>` tiene que
     * estar incluido y con `BMO_MONTON_BYTES` bastante para la imagen. */
    s->base = (unsigned long long)malloc(bytes);
    if (s->base == 0) {
        return 0;
    }
    s->bloque = __bmo_bloque_cap;
    s->ancho = ancho;
    s->alto = alto;

    bmo_sup_poner(s->base, 0, BMO_SUP_MAGIC);
    bmo_sup_poner(s->base, 1, (unsigned int)ancho);
    bmo_sup_poner(s->base, 2, (unsigned int)alto);
    bmo_sup_poner(s->base, 3, (unsigned int)ancho); /* stride = ancho: sin relleno */
    bmo_sup_poner(s->base, 4, BMO_SUP_BGRA32);
    bmo_sup_poner(s->base, 5, 0); /* secuencia: nada que pintar todavia */
    bmo_sup_poner(s->base, 6, 0);
    bmo_sup_poner(s->base, 7, 0);

    padre = bmo_valor(BMO_TAREA_ACTUAL, BMO_OP_MI_PADRE, 0, 0, 0);
    if (padre == 0) {
        /* Nadie compone: no es un fallo, es haberse lanzado desde el shell. */
        return 0;
    }
    /* El desplazamiento va contra la base del BLOQUE del monton, que es lo que
     * el kernel conoce -- la misma resta que hace `fread`. */
    bmo_valor(s->bloque, BMO_MEM_OFRECER, s->base - __bmo_bloque_base, bytes, padre);
    return s;
}

#endif /* BMO_SUPERFICIE_H */
