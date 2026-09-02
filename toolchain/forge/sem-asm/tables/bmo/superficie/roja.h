/* superficie/roja.h -- pedir el bloque, escribir su cabecera y OFRECERLO al DIRECTOR
 *
 * Un CARRIL de `<bmo/superficie.h>` (L6g). La cabecera entera --que
 * explica por que existe esta pieza-- esta en la fachada; aqui va lo
 * que cambia de color.
 *
 * [carril]  ROJO         `MEM_OFRECER` presta un trozo de TU bloque a otro
 *                        proceso. Lo que se escriba en esa cabecera es lo que
 *                        el DIRECTOR va a creerse sobre cuanta memoria puede
 *                        leer
 * [cuesta]  DATO         un ancho o un alto mal puestos hacen que el
 *                        compositor lea fuera de lo que se le presto
 * [riesgo]  AJENO UNICO
 *                        AJENO: la cabecera la lee OTRO proceso, que no puede
 *                        comprobarla. UNICO: ofrecer se hace una vez y no se
 *                        deshace
 */
#ifndef BMO_SUPERFICIE_ROJA_H
#define BMO_SUPERFICIE_ROJA_H

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

/* Lo que ocupa el buzon antes de la primera ranura: cabeza, cola y el puntero. */
#define BMO_SUP_BUZON_CABECERA 16
/* Lo que mide una ranura: un evento crudo. */
#define BMO_SUP_BUZON_RANURA 8

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
BMO_SUPERFICIE *bmo_superficie_crear_con_buzon(int ancho, int alto, int ranuras) {
    BMO_SUPERFICIE *s;
    unsigned long long bytes;
    unsigned long long padre;
    unsigned long long buzon;

    if (ancho <= 0 || alto <= 0) {
        return 0;
    }
    /* Las ranuras tienen que ser potencia de dos, porque el indice avanza con
     * una mascara y no con un resto: `(i + 1) & (n - 1)`. Un numero que no lo
     * sea daria una mascara que salta ranuras, y el sintoma seria teclas que se
     * pierden de vez en cuando -- el peor fallo posible en una entrada.
     * Un valor malo se trata como "sin buzon", que es la respuesta segura. */
    if (ranuras < 2 || (ranuras & (ranuras - 1)) != 0) {
        ranuras = 0;
    }
    bytes = BMO_SUP_CABECERA + (unsigned long long)ancho * alto * 4;
    buzon = 0;
    if (ranuras > 0) {
        /* El buzon va DETRAS de los pixeles. Delante habria que mover el
         * origen de la imagen, y entonces `stride` dejaria de bastar para
         * describirla. */
        buzon = bytes;
        bytes = bytes + BMO_SUP_BUZON_CABECERA
              + (unsigned long long)ranuras * BMO_SUP_BUZON_RANURA;
    }

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
    bmo_sup_poner(s->base, 6, (unsigned int)buzon);
    bmo_sup_poner(s->base, 7, (unsigned int)ranuras);
    if (ranuras > 0) {
        /* Cabeza, cola y puntero a cero: el buzon nace vacio y sin raton
         * dentro. Se escriben ANTES de ofrecer el bloque -- si se dejaran para
         * despues, el DIRECTOR podria tomar la superficie y leer basura. */
        bmo_sup_poner(s->base, (int)(buzon / 4), 0);
        bmo_sup_poner(s->base, (int)(buzon / 4) + 1, 0);
        bmo_sup_poner(s->base, (int)(buzon / 4) + 2, 0);
        bmo_sup_poner(s->base, (int)(buzon / 4) + 3, 0);
    }

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

/* La superficie de siempre: sin buzon.
 *
 * Se queda con el nombre corto a proposito. Una app que solo ENSENA es el caso
 * normal --un reloj, un medidor, un visor--, y pedir entrada tiene que costar
 * escribirlo: quien no la lee no debe quitarle las teclas al escritorio.
 */
BMO_SUPERFICIE *bmo_superficie_crear(int ancho, int alto) {
    return bmo_superficie_crear_con_buzon(ancho, alto, 0);
}

#endif /* BMO_SUPERFICIE_ROJA_H */
