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
 *   24..28   BUZON: donde empieza, en bytes desde el principio. 0 = no hay
 *   28..32   BUZON: cuantas ranuras (potencia de 2)
 *   32..     los pixeles, y detras de ellos el buzon si se pidio
 * ```
 *
 * == EL BUZON: el camino de vuelta, y por que no cuesta un syscall ==
 *
 * Una superficie deja que una app ENSENE. El buzon deja que la TOQUES.
 *
 * Y va aqui dentro, en el mismo bloque, por el mismo motivo que la cabecera:
 * el kernel presta BYTES y no tiene por que saber que hay dentro. El bloque
 * que la app ofrece se mapea en el DIRECTOR con **derecho de escritura** --lo
 * concede la propia app al ofrecerlo, `RIGHT_READ | RIGHT_WRITE`-- asi que el
 * DIRECTOR puede dejar una tecla ahi con un `mov`, igual que ya lee un pixel
 * con un `mov`.
 *
 * ** CERO PUERTAS POR TECLA, Y NINGUNA OPERACION NUEVA DEL KERNEL. Una puerta
 * son 969 ciclos (`docs/componente/LA_PUERTA_POR_DENTRO.md`); para una
 * calculadora eso es gratis, pero para algo que siga al teclado a sesenta por
 * segundo es el precio equivocado.
 *
 * == Es OPCIONAL, y eso es lo que decide quien se queda las teclas ==
 *
 * `bmo_superficie_crear` no pide buzon: una app que solo ensena --un reloj, un
 * medidor-- no lo necesita, y el DIRECTOR **no le manda teclas**, o sea que el
 * escritorio las conserva. Pedirlo es decir *"yo se leer"*, y el foco solo se
 * le puede dar a quien lo dijo.
 *
 * == La forma del buzon ==
 *
 * ```text
 *    +0   CABEZA (u32)   la escribe el DIRECTOR
 *    +4   COLA   (u32)   la escribe la app
 *    +8   las ranuras, de 8 bytes cada una
 * ```
 *
 * Un escritor y un lector, cada uno con su indice: no hace falta cerrojo, por
 * la misma razon que no lo hace falta la secuencia. Vacio es `cabeza == cola`;
 * lleno es que la cabeza alcanzaria a la cola, y entonces **el DIRECTOR
 * descarta** en vez de esperar -- un compositor que espere a una app colgada es
 * una app rota llevandose el escritorio.
 *
 * ** Y una ranura es un evento CRUDO, el mismo `unsigned long long` que
 * devuelve `bmo_entrada_evento`. No hay formato nuevo que aprender: el codigo
 * que ya sabe leer una tecla de la pantalla exclusiva sirve tal cual dentro de
 * una ventana.
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
 * ** HOY SOLO VIAJA EL CLIC (el boton BAJANDO). Soltar no se publica todavia,
 * asi que dentro de una app no se puede arrastrar. Esta dicho aqui y no
 * escondido: media promesa contada entera es una limitacion; contada a medias
 * es un fallo. */
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

/* Lo que ocupa el buzon antes de la primera ranura: cabeza y cola. */
#define BMO_SUP_BUZON_CABECERA 8
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
        /* Cabeza y cola a cero: el buzon nace vacio. Se escriben ANTES de
         * ofrecer el bloque -- si se dejaran para despues, el DIRECTOR podria
         * tomar la superficie y leer una cola con basura dentro. */
        bmo_sup_poner(s->base, (int)(buzon / 4), 0);
        bmo_sup_poner(s->base, (int)(buzon / 4) + 1, 0);
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

#endif /* BMO_SUPERFICIE_H */
